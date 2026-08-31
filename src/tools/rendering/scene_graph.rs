// <ai-owned/>

//! Walks a parsed glTF document's node tree, composing world-space transforms (animated where an
//! `Animation` applies, static otherwise) and collecting every visible mesh primitive's geometry
//! already transformed into world space - the input the rasterizer core and bounds computation
//! both need. No pyo3, independently testable like the rest of `tools/`.
//!
//! **`include`/`exclude` semantics**: a name (matched against a node's own name or its mesh's
//! name) makes the whole subtree rooted at that node included/excluded. `exclude` always wins
//! over `include` within a subtree once applied, matching real-world "hide this part" intent.
//! With no `include` list, everything starts included; `exclude` alone still works as a deny-list.

use anyhow::{Context, Result};
use glam::{DVec3, Mat4, Quat, Vec3};

use super::super::glb::{decode_u32s, decode_vec2s, decode_vec3s};
use super::super::gltf;
use super::animation::{self, EvaluatedTrs};

/// One mesh primitive's geometry, already transformed into world space by its node's (possibly
/// animated) transform. `material` indexes `root.materials`.
#[derive(Debug)]
pub struct WorldPrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material: u32,
}

/// Collects every visible mesh primitive in `root`'s default scene, in world space, at `time`
/// (only meaningful when `animation` is `Some`).
pub fn collect_world_primitives(
    root: &gltf::Root,
    bin: &[u8],
    animation: Option<&gltf::Animation>,
    time: f64,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<WorldPrimitive>> {
    let scene_index = root.scene.unwrap_or(0);
    let scene = root
        .scenes
        .get(scene_index as usize)
        .with_context(|| format!("glTF document has no scene at index {scene_index}"))?;

    let mut primitives = Vec::new();
    let included_by_default = include.is_empty();
    for &node_index in &scene.nodes {
        traverse(
            root,
            bin,
            animation,
            time,
            node_index,
            Mat4::IDENTITY,
            included_by_default,
            false,
            include,
            exclude,
            &mut primitives,
        )?;
    }
    Ok(primitives)
}

#[allow(clippy::too_many_arguments)]
fn traverse(
    root: &gltf::Root,
    bin: &[u8],
    animation: Option<&gltf::Animation>,
    time: f64,
    node_index: u32,
    parent_matrix: Mat4,
    parent_included: bool,
    parent_excluded: bool,
    include: &[String],
    exclude: &[String],
    out: &mut Vec<WorldPrimitive>,
) -> Result<()> {
    let node = root
        .nodes
        .get(node_index as usize)
        .with_context(|| format!("node index {node_index} is out of range"))?;
    let mesh_name = node
        .mesh
        .and_then(|m| root.meshes.get(m as usize))
        .and_then(|mesh| mesh.name.as_deref());

    let included = parent_included || name_matches(node.name.as_deref(), include) || name_matches(mesh_name, include);
    let excluded = parent_excluded || name_matches(node.name.as_deref(), exclude) || name_matches(mesh_name, exclude);
    let visible = included && !excluded;

    let evaluated: Option<EvaluatedTrs> = match animation {
        Some(anim) => Some(animation::evaluate_node_trs(root, bin, anim, node_index, time)?),
        None => None,
    };
    let world_matrix = parent_matrix * node_local_matrix(node, evaluated.as_ref());

    if visible && let Some(mesh_index) = node.mesh {
        let mesh = root
            .meshes
            .get(mesh_index as usize)
            .with_context(|| format!("mesh index {mesh_index} is out of range"))?;
        for primitive in &mesh.primitives {
            out.push(world_primitive(root, bin, primitive, world_matrix)?);
        }
    }

    for &child_index in &node.children {
        traverse(
            root,
            bin,
            animation,
            time,
            child_index,
            world_matrix,
            included,
            excluded,
            include,
            exclude,
            out,
        )?;
    }
    Ok(())
}

fn name_matches(name: Option<&str>, list: &[String]) -> bool {
    name.is_some_and(|n| list.iter().any(|candidate| candidate == n))
}

/// A node's local TRS: the evaluated animation value for each channel that applies, falling back
/// to the node's own static value, falling back to the glTF spec's identity default for any
/// component neither provides - the same three-way fallback `gltf::Node`'s fields already imply.
fn node_local_matrix(node: &gltf::Node, animated: Option<&EvaluatedTrs>) -> Mat4 {
    let translation = animated
        .and_then(|a| a.translation)
        .or(node.translation)
        .unwrap_or([0.0, 0.0, 0.0]);
    let rotation = animated
        .and_then(|a| a.rotation)
        .or(node.rotation)
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let scale = animated.and_then(|a| a.scale).or(node.scale).unwrap_or([1.0, 1.0, 1.0]);
    Mat4::from_scale_rotation_translation(Vec3::from(scale), Quat::from_array(rotation), Vec3::from(translation))
}

fn world_primitive(
    root: &gltf::Root,
    bin: &[u8],
    primitive: &gltf::Primitive,
    world_matrix: Mat4,
) -> Result<WorldPrimitive> {
    let local_positions = decode_vec3s(root, bin, primitive.attributes.position)?;
    let local_normals = decode_vec3s(root, bin, primitive.attributes.normal)?;
    let uvs = decode_vec2s(root, bin, primitive.attributes.texcoord_0)?;
    let indices = decode_u32s(root, bin, primitive.indices)?;

    // The standard "normal matrix" (inverse-transpose of the upper 3x3) rather than the world
    // matrix directly: correct for non-uniform scale (e.g. an animated node scaled unevenly per
    // axis), which a plain rotation-only transform would get wrong.
    let normal_matrix = world_matrix.inverse().transpose();
    let positions = local_positions
        .iter()
        .map(|&p| world_matrix.transform_point3(Vec3::from(p)).to_array())
        .collect();
    let normals = local_normals
        .iter()
        .map(|&n| {
            normal_matrix
                .transform_vector3(Vec3::from(n))
                .normalize_or_zero()
                .to_array()
        })
        .collect();

    Ok(WorldPrimitive {
        positions,
        normals,
        uvs,
        indices,
        material: primitive.material,
    })
}

/// The world-space min/max corners spanning every vertex of every given primitive. Falls back to
/// a small box centered on the origin when nothing is visible, so a camera can still be framed
/// for an empty/fully-excluded scene rather than this returning `None` and pushing that case
/// onto every caller.
pub fn world_bounds(primitives: &[WorldPrimitive]) -> (DVec3, DVec3) {
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    let mut found = false;
    for primitive in primitives {
        for &p in &primitive.positions {
            found = true;
            let p = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
            min = min.min(p);
            max = max.max(p);
        }
    }
    if found {
        (min, max)
    } else {
        (DVec3::splat(-0.5), DVec3::splat(0.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal but complete glTF document (one accessor per attribute, backed by real
    /// bytes) for a single triangle, so tests exercise the real accessor-decode path rather than
    /// stubbing it out.
    fn triangle_root() -> (gltf::Root, Vec<u8>) {
        let positions = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let normals = [[0.0f32, 0.0, 1.0]; 3];
        let uvs = [[0.0f32, 0.0]; 3];
        let indices = [0u32, 1, 2];

        let mut bin = Vec::new();
        let position_view = push_floats(&mut bin, &positions.iter().flatten().copied().collect::<Vec<_>>());
        let normal_view = push_floats(&mut bin, &normals.iter().flatten().copied().collect::<Vec<_>>());
        let uv_view = push_floats(&mut bin, &uvs.iter().flatten().copied().collect::<Vec<_>>());
        let index_view = push_u32s(&mut bin, &indices);

        let mut root = gltf::Root::default();
        root.buffer_views = vec![position_view, normal_view, uv_view, index_view];
        root.accessors = vec![
            accessor(0, 3, "VEC3", gltf::COMPONENT_TYPE_FLOAT),
            accessor(1, 3, "VEC3", gltf::COMPONENT_TYPE_FLOAT),
            accessor(2, 3, "VEC2", gltf::COMPONENT_TYPE_FLOAT),
            accessor(3, 3, "SCALAR", gltf::COMPONENT_TYPE_UNSIGNED_INT),
        ];
        root.meshes.push(gltf::Mesh {
            name: Some("triangle_mesh".to_string()),
            primitives: vec![gltf::Primitive {
                attributes: gltf::Attributes {
                    position: 0,
                    normal: 1,
                    texcoord_0: 2,
                },
                indices: 3,
                material: 0,
            }],
            extras: None,
        });
        root.materials.push(gltf::Material::default());
        (root, bin)
    }

    fn push_floats(bin: &mut Vec<u8>, values: &[f32]) -> gltf::BufferView {
        let byte_offset = bin.len() as u32;
        for v in values {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        gltf::BufferView {
            buffer: 0,
            byte_offset,
            byte_length: bin.len() as u32 - byte_offset,
            target: None,
        }
    }

    fn push_u32s(bin: &mut Vec<u8>, values: &[u32]) -> gltf::BufferView {
        let byte_offset = bin.len() as u32;
        for v in values {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        gltf::BufferView {
            buffer: 0,
            byte_offset,
            byte_length: bin.len() as u32 - byte_offset,
            target: None,
        }
    }

    fn accessor(buffer_view: u32, count: u32, type_: &str, component_type: u32) -> gltf::Accessor {
        gltf::Accessor {
            buffer_view,
            component_type,
            count,
            type_: type_.to_string(),
            min: None,
            max: None,
        }
    }

    fn node(name: &str) -> gltf::Node {
        gltf::Node {
            name: Some(name.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn a_single_visible_node_produces_one_world_primitive_at_the_identity() {
        let (mut root, bin) = triangle_root();
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            ..node("solo")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);

        let primitives = collect_world_primitives(&root, &bin, None, 0.0, &[], &[]).unwrap();
        assert_eq!(primitives.len(), 1);
        assert_eq!(
            primitives[0].positions,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(primitives[0].indices, vec![0, 1, 2]);
    }

    #[test]
    fn parent_and_child_translations_compose() {
        let (mut root, bin) = triangle_root();
        root.nodes.push(gltf::Node {
            translation: Some([10.0, 0.0, 0.0]),
            children: vec![1],
            ..node("parent")
        });
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            translation: Some([0.0, 5.0, 0.0]),
            ..node("child")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);

        let primitives = collect_world_primitives(&root, &bin, None, 0.0, &[], &[]).unwrap();
        assert_eq!(primitives.len(), 1);
        // The triangle's [0,0,0] vertex should land at parent + child translation.
        assert_eq!(primitives[0].positions[0], [10.0, 5.0, 0.0]);
    }

    #[test]
    fn animation_overrides_the_static_translation() {
        let (mut root, bin) = triangle_root();
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            translation: Some([1.0, 1.0, 1.0]),
            ..node("animated")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);

        let mut anim_bin = Vec::new();
        let input_view = push_floats(&mut anim_bin, &[0.0]);
        let output_view = push_floats(&mut anim_bin, &[7.0, 8.0, 9.0]);
        root.buffer_views.push(input_view);
        let input_accessor_index = root.accessors.len() as u32;
        root.accessors
            .push(accessor(4, 1, "SCALAR", gltf::COMPONENT_TYPE_FLOAT));
        root.buffer_views.push(output_view);
        let output_accessor_index = root.accessors.len() as u32;
        root.accessors.push(accessor(5, 1, "VEC3", gltf::COMPONENT_TYPE_FLOAT));

        // Two separate binary buffers in this test (`bin` for the mesh, `anim_bin` for the
        // animation track) don't compose - append the animation bytes onto the mesh's buffer and
        // fix up the bufferView offsets accordingly, rather than pretending there are two buffers.
        let mesh_bin_len = bin.len() as u32;
        let mut combined = bin;
        combined.extend_from_slice(&anim_bin);
        root.buffer_views[input_accessor_index as usize].byte_offset += mesh_bin_len;
        root.buffer_views[output_accessor_index as usize].byte_offset += mesh_bin_len;

        let animation = gltf::Animation {
            name: None,
            channels: vec![gltf::AnimationChannel {
                sampler: 0,
                target: gltf::AnimationChannelTarget {
                    node: 0,
                    path: "translation".to_string(),
                },
            }],
            samplers: vec![gltf::AnimationSampler {
                input: input_accessor_index,
                output: output_accessor_index,
                interpolation: None,
            }],
            extras: None,
        };

        let primitives = collect_world_primitives(&root, &combined, Some(&animation), 0.0, &[], &[]).unwrap();
        assert_eq!(primitives[0].positions[0], [7.0, 8.0, 9.0]);
    }

    #[test]
    fn exclude_hides_a_whole_subtree() {
        let (mut root, bin) = triangle_root();
        root.nodes.push(gltf::Node {
            children: vec![1],
            ..node("parent")
        });
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            ..node("hidden_child")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);

        let primitives = collect_world_primitives(&root, &bin, None, 0.0, &[], &["hidden_child".to_string()]).unwrap();
        assert!(primitives.is_empty());
    }

    #[test]
    fn include_shows_only_the_named_subtree() {
        let (mut root, bin) = triangle_root();
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            ..node("shown")
        });
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            ..node("not_shown")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0, 1] });
        root.scene = Some(0);

        let primitives = collect_world_primitives(&root, &bin, None, 0.0, &["shown".to_string()], &[]).unwrap();
        assert_eq!(primitives.len(), 1);
    }

    #[test]
    fn include_matches_by_mesh_name_too() {
        let (mut root, bin) = triangle_root();
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            ..node("unnamed_use")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);

        // "triangle_mesh" is the mesh's name (see `triangle_root`), not the node's.
        let primitives = collect_world_primitives(&root, &bin, None, 0.0, &["triangle_mesh".to_string()], &[]).unwrap();
        assert_eq!(primitives.len(), 1);
    }

    #[test]
    fn exclude_wins_even_under_an_included_ancestor() {
        let (mut root, bin) = triangle_root();
        root.nodes.push(gltf::Node {
            children: vec![1],
            ..node("included_parent")
        });
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            ..node("excluded_child")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);

        let primitives = collect_world_primitives(
            &root,
            &bin,
            None,
            0.0,
            &["included_parent".to_string()],
            &["excluded_child".to_string()],
        )
        .unwrap();
        assert!(primitives.is_empty());
    }

    #[test]
    fn errors_on_missing_scene_rather_than_panicking() {
        let root = gltf::Root::default(); // no scenes at all
        let err = collect_world_primitives(&root, &[], None, 0.0, &[], &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("scene"));
    }

    #[test]
    fn world_bounds_folds_every_vertex_of_every_primitive() {
        let (mut root, bin) = triangle_root(); // vertices span [0,0,0]..[1,1,0]
        root.nodes.push(gltf::Node {
            mesh: Some(0),
            ..node("solo")
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);

        let primitives = collect_world_primitives(&root, &bin, None, 0.0, &[], &[]).unwrap();
        let (min, max) = world_bounds(&primitives);
        assert_eq!(min, DVec3::new(0.0, 0.0, 0.0));
        assert_eq!(max, DVec3::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn world_bounds_falls_back_to_a_small_box_when_nothing_is_visible() {
        let (min, max) = world_bounds(&[]);
        assert_eq!(min, DVec3::splat(-0.5));
        assert_eq!(max, DVec3::splat(0.5));
    }
}

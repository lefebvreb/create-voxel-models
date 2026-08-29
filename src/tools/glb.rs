// <ai-owned/>

use std::collections::{BTreeSet, HashMap};

use png::ColorType;
use pyo3::{Bound, Py, PyResult};

use super::gltf;
use super::meshing::{self, MaterialData, PaletteData};
use super::utils::encode_png;
use crate::anim::{Anim, Interpolation, Trs};
use crate::model::Model;
use crate::palette::Palette;
use crate::scene::{Mesh, Node, Scene};
use crate::utils::HashPy;

pub fn export_glb(scene: Bound<Scene>) -> PyResult<Vec<u8>> {
    let py = scene.py();
    let scene_ref = scene.borrow();

    let mut root = gltf::Root::default();
    let mut writer = BinWriter::default();
    let mut extensions_used = BTreeSet::new();
    let mut palette_cache: HashMap<HashPy<Palette>, Vec<u32>> = HashMap::new();
    let mut model_cache: HashMap<HashPy<Model>, Vec<gltf::Primitive>> = HashMap::new();

    let (mut gltf_nodes, node_index, roots) = build_nodes(py, scene_ref.nodes.values());

    for (node, meshes) in group_meshes_by_node(py, scene_ref.meshes.values()) {
        let mut primitives = Vec::new();
        for mesh in &meshes {
            let model = mesh.borrow(py).model.clone_ref(py);
            primitives.extend(get_or_build_model_primitives(
                py,
                &model,
                &mut model_cache,
                &mut palette_cache,
                &mut root,
                &mut writer,
                &mut extensions_used,
            )?);
        }
        if primitives.is_empty() {
            continue;
        }
        let first = meshes[0].borrow(py);
        let mesh_index = root.meshes.len() as u32;
        root.meshes.push(gltf::Mesh {
            name: Some(first.name.clone()),
            primitives,
            extras: first.extras.clone(),
        });
        let node_idx = node_index[&HashPy(node)] as usize;
        gltf_nodes[node_idx].mesh = Some(mesh_index);
    }

    root.nodes = gltf_nodes;
    root.scenes = vec![gltf::Scene { nodes: roots }];
    root.scene = Some(0);

    for anim in scene_ref.anims.values() {
        root.animations
            .push(build_animation(py, anim, &node_index, &mut writer));
    }

    root.accessors = writer.accessors;
    root.buffer_views = writer.buffer_views;
    if !writer.bytes.is_empty() {
        root.buffers.push(gltf::Buffer {
            byte_length: writer.bytes.len() as u32,
        });
    }
    root.extensions_used = extensions_used.into_iter().map(String::from).collect();

    let json_bytes =
        serde_json::to_vec(&root).expect("all exportable floats are validated finite at construction time");
    Ok(write_glb_container(&json_bytes, &writer.bytes))
}

// --- Node hierarchy: `Scene.nodes` is a flat Vec with parent back-links, not a tree ---

fn build_nodes<'a>(
    py: pyo3::Python,
    nodes: impl Iterator<Item = &'a Py<Node>> + Clone,
) -> (Vec<gltf::Node>, HashMap<HashPy<Node>, u32>, Vec<u32>) {
    let node_index: HashMap<HashPy<Node>, u32> = nodes
        .clone()
        .enumerate()
        .map(|(i, node)| (HashPy(node.clone_ref(py)), i as u32))
        .collect();

    let mut gltf_nodes: Vec<gltf::Node> = nodes
        .clone()
        .map(|node| {
            let node_ref = node.get();
            gltf::Node {
                name: Some(node_ref.name.clone()),
                translation: node_ref.translation.map(|v| v.inner.as_vec3().to_array()),
                rotation: node_ref.rotation.map(|q| q.inner.as_quat().to_array()),
                scale: node_ref.scale.map(|v| v.inner.as_vec3().to_array()),
                extras: node_ref.extras.clone(),
                ..Default::default()
            }
        })
        .collect();

    let mut roots = Vec::new();
    for node in nodes {
        let idx = node_index[&HashPy(node.clone_ref(py))];
        match &node.get().parent {
            Some(parent) => {
                let parent_idx = node_index[&HashPy(parent.clone_ref(py))] as usize;
                gltf_nodes[parent_idx].children.push(idx);
            }
            None => roots.push(idx),
        }
    }

    (gltf_nodes, node_index, roots)
}

fn group_meshes_by_node<'a>(
    py: pyo3::Python,
    meshes: impl Iterator<Item = &'a Py<Mesh>>,
) -> Vec<(Py<Node>, Vec<Py<Mesh>>)> {
    let mut order: Vec<HashPy<Node>> = Vec::new();
    let mut groups: HashMap<HashPy<Node>, Vec<Py<Mesh>>> = HashMap::new();
    for mesh in meshes {
        let parent = HashPy(mesh.get().parent.clone_ref(py));
        if !groups.contains_key(&parent) {
            order.push(HashPy(parent.0.clone_ref(py)));
        }
        groups.entry(parent).or_default().push(mesh.clone_ref(py));
    }
    order
        .into_iter()
        .map(|key| {
            let meshes = groups.remove(&key).expect("key was just inserted above");
            (key.0, meshes)
        })
        .collect()
}

// --- Model/Palette dedup, keyed by pointer identity via the existing `HashPy` helper ---

fn get_or_build_palette_materials(
    py: pyo3::Python,
    palette: &Py<Palette>,
    palette_cache: &mut HashMap<HashPy<Palette>, Vec<u32>>,
    root: &mut gltf::Root,
    writer: &mut BinWriter,
    extensions_used: &mut BTreeSet<&'static str>,
) -> PyResult<Vec<u32>> {
    let key = HashPy(palette.clone_ref(py));
    if let Some(indices) = palette_cache.get(&key) {
        return Ok(indices.clone());
    }
    let data: PaletteData = meshing::export_palette(palette.bind(py).clone());
    let mut indices = Vec::with_capacity(data.materials.len());
    for material in &data.materials {
        indices.push(build_material(material, root, writer, extensions_used)?);
    }
    palette_cache.insert(key, indices.clone());
    Ok(indices)
}

fn get_or_build_model_primitives(
    py: pyo3::Python,
    model: &Py<Model>,
    model_cache: &mut HashMap<HashPy<Model>, Vec<gltf::Primitive>>,
    palette_cache: &mut HashMap<HashPy<Palette>, Vec<u32>>,
    root: &mut gltf::Root,
    writer: &mut BinWriter,
    extensions_used: &mut BTreeSet<&'static str>,
) -> PyResult<Vec<gltf::Primitive>> {
    let key = HashPy(model.clone_ref(py));
    if let Some(primitives) = model_cache.get(&key) {
        return Ok(primitives.clone());
    }
    let mesh_data = meshing::export_model(model.bind(py).clone());
    // A hollow model (no voxels set) produces no primitives; resolving its palette into materials
    // anyway would leave genuinely unreferenced materials/textures in the output, so skip it.
    if mesh_data.primitives.is_empty() {
        model_cache.insert(key, Vec::new());
        return Ok(Vec::new());
    }
    let palette = model.borrow(py).palette.clone_ref(py);
    let material_indices = get_or_build_palette_materials(py, &palette, palette_cache, root, writer, extensions_used)?;

    let mut primitives = Vec::with_capacity(mesh_data.primitives.len());
    for primitive in &mesh_data.primitives {
        let position = writer.write_positions(&primitive.positions);
        let normal = writer.write_vec3s_no_bounds(&primitive.normals, gltf::TARGET_ARRAY_BUFFER);
        let texcoord_0 = writer.write_vec2s(&primitive.uvs);
        let indices = writer.write_indices(&primitive.indices);
        primitives.push(gltf::Primitive {
            attributes: gltf::Attributes {
                position,
                normal,
                texcoord_0,
            },
            indices,
            material: material_indices[primitive.material_index],
        });
    }

    model_cache.insert(key, primitives.clone());
    Ok(primitives)
}

// --- MaterialData -> glTF material/texture/image ---

fn build_material(
    material: &MaterialData,
    root: &mut gltf::Root,
    writer: &mut BinWriter,
    extensions_used: &mut BTreeSet<&'static str>,
) -> PyResult<u32> {
    let width = material.atlas_width;
    let height = material.atlas_height;

    let base_color_bytes: Vec<u8> = material.base_color.iter().flatten().copied().collect();
    let base_color_texture = push_texture(
        root,
        writer,
        encode_png(width, height, &base_color_bytes, ColorType::Rgb),
    );

    let metallic_roughness_bytes: Vec<u8> = material
        .metallic_roughness
        .iter()
        .flat_map(|&[roughness, metallic]| [0, roughness, metallic])
        .collect();
    let metallic_roughness_texture = push_texture(
        root,
        writer,
        encode_png(width, height, &metallic_roughness_bytes, ColorType::Rgb),
    );

    let mut extensions = gltf::MaterialExtensions::default();

    if material.ior != 1.5 {
        extensions.ior = Some(gltf::KhrMaterialsIor { ior: material.ior });
        extensions_used.insert("KHR_materials_ior");
    }

    if material.transmission.iter().any(|&t| t != 0) {
        let transmission_texture = push_texture(
            root,
            writer,
            encode_png(width, height, &material.transmission, ColorType::Grayscale),
        );
        extensions.transmission = Some(gltf::KhrMaterialsTransmission {
            transmission_factor: 1.0,
            transmission_texture: gltf::TextureInfo {
                index: transmission_texture,
            },
        });
        extensions_used.insert("KHR_materials_transmission");
    }

    let (emissive_factor, emissive_texture) = if material.emissive != 0.0 {
        extensions.emissive_strength = Some(gltf::KhrMaterialsEmissiveStrength {
            emissive_strength: material.emissive,
        });
        extensions_used.insert("KHR_materials_emissive_strength");
        (
            Some([1.0, 1.0, 1.0]),
            Some(gltf::TextureInfo {
                index: base_color_texture,
            }),
        )
    } else {
        (None, None)
    };

    if let Some(volume) = material.volume {
        extensions.volume = Some(gltf::KhrMaterialsVolume {
            thickness_factor: volume.thickness,
            attenuation_color: [
                volume.color.r as f32 / 255.0,
                volume.color.g as f32 / 255.0,
                volume.color.b as f32 / 255.0,
            ],
            attenuation_distance: volume.distance,
        });
        extensions_used.insert("KHR_materials_volume");
    }

    let index = root.materials.len() as u32;
    root.materials.push(gltf::Material {
        name: None,
        pbr_metallic_roughness: gltf::PbrMetallicRoughness {
            base_color_texture: Some(gltf::TextureInfo {
                index: base_color_texture,
            }),
            metallic_roughness_texture: Some(gltf::TextureInfo {
                index: metallic_roughness_texture,
            }),
        },
        emissive_factor,
        emissive_texture,
        extensions: if extensions.is_empty() { None } else { Some(extensions) },
    });
    Ok(index)
}

/// Pushes the atlas sampler lazily, on first use: a texture-less scene should not end up with an
/// unreferenced sampler in the output (the validator flags unused objects).
fn push_texture(root: &mut gltf::Root, writer: &mut BinWriter, png_bytes: Vec<u8>) -> u32 {
    if root.samplers.is_empty() {
        root.samplers.push(gltf::Sampler {
            mag_filter: gltf::FILTER_NEAREST,
            min_filter: gltf::FILTER_NEAREST,
            wrap_s: gltf::WRAP_CLAMP_TO_EDGE,
            wrap_t: gltf::WRAP_CLAMP_TO_EDGE,
        });
    }
    let buffer_view = writer.push_view(&png_bytes, None);
    let image_index = root.images.len() as u32;
    root.images.push(gltf::Image {
        mime_type: "image/png".to_string(),
        buffer_view,
    });
    let texture_index = root.textures.len() as u32;
    root.textures.push(gltf::Texture {
        sampler: 0,
        source: image_index,
    });
    texture_index
}

// --- Animations ---

fn build_animation(
    py: pyo3::Python,
    anim: &Py<Anim>,
    node_index: &HashMap<HashPy<Node>, u32>,
    writer: &mut BinWriter,
) -> gltf::Animation {
    let anim_ref = anim.borrow(py);
    let mut channels = Vec::new();
    let mut samplers = Vec::new();

    let mut nodes: Vec<(&HashPy<Node>, &Trs)> = anim_ref.nodes.iter().collect();
    nodes.sort_by_key(|(node, _)| node_index[node]);

    for (node, trs) in nodes {
        let node_idx = node_index[node];
        if let Some(channel) = &trs.translation {
            push_channel(
                &mut samplers,
                &mut channels,
                writer,
                node_idx,
                "translation",
                channel,
                |v| v.inner.as_vec3().to_array(),
            );
        }
        if let Some(channel) = &trs.rotation {
            push_channel(
                &mut samplers,
                &mut channels,
                writer,
                node_idx,
                "rotation",
                channel,
                |q| q.inner.as_quat().to_array(),
            );
        }
        if let Some(channel) = &trs.scale {
            push_channel(&mut samplers, &mut channels, writer, node_idx, "scale", channel, |v| {
                v.inner.as_vec3().to_array()
            });
        }
    }

    gltf::Animation {
        name: Some(anim_ref.name.clone()),
        channels,
        samplers,
        extras: anim_ref.extras.clone(),
    }
}

fn push_channel<T, const N: usize>(
    samplers: &mut Vec<gltf::AnimationSampler>,
    channels: &mut Vec<gltf::AnimationChannel>,
    writer: &mut BinWriter,
    node_idx: u32,
    path: &str,
    channel: &crate::anim::Channel<T>,
    to_array: impl Fn(&T) -> [f32; N],
) {
    // A channel with no keyframes would produce a zero-count accessor, which the spec forbids
    // (accessor.count has a minimum of 1) — skip it, mirroring how a mesh with zero primitives
    // is skipped entirely rather than emitted empty.
    if channel.input.is_empty() {
        return;
    }
    let input = writer.write_scalar_times(&channel.input);
    let values: Vec<[f32; N]> = channel.output.iter().map(to_array).collect();
    let output = writer.write_floatn(&values, None);

    let interpolation = match channel.interpolation {
        None | Some(Interpolation::Linear) => None,
        Some(Interpolation::Step) => Some("STEP".to_string()),
        Some(Interpolation::CubicSpline) => Some("CUBICSPLINE".to_string()),
    };

    let sampler_idx = samplers.len() as u32;
    samplers.push(gltf::AnimationSampler {
        input,
        output,
        interpolation,
    });
    channels.push(gltf::AnimationChannel {
        sampler: sampler_idx,
        target: gltf::AnimationChannelTarget {
            node: node_idx,
            path: path.to_string(),
        },
    });
}

// --- Binary buffer accumulator ---

#[derive(Default)]
struct BinWriter {
    bytes: Vec<u8>,
    buffer_views: Vec<gltf::BufferView>,
    accessors: Vec<gltf::Accessor>,
}

impl BinWriter {
    fn push_view(&mut self, data: &[u8], target: Option<u32>) -> u32 {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        let byte_offset = self.bytes.len() as u32;
        self.bytes.extend_from_slice(data);
        let index = self.buffer_views.len() as u32;
        self.buffer_views.push(gltf::BufferView {
            buffer: 0,
            byte_offset,
            byte_length: data.len() as u32,
            target,
        });
        index
    }

    fn push_accessor(
        &mut self,
        component_type: u32,
        count: u32,
        type_: &str,
        data: &[u8],
        target: Option<u32>,
        bounds: Option<(Vec<f32>, Vec<f32>)>,
    ) -> u32 {
        let buffer_view = self.push_view(data, target);
        let index = self.accessors.len() as u32;
        self.accessors.push(gltf::Accessor {
            buffer_view,
            component_type,
            count,
            type_: type_.to_string(),
            min: bounds.as_ref().map(|(min, _)| min.clone()),
            max: bounds.map(|(_, max)| max),
        });
        index
    }

    fn write_positions(&mut self, positions: &[[f32; 3]]) -> u32 {
        let (min, max) = position_min_max(positions);
        let data = flatten_le(positions);
        self.push_accessor(
            gltf::COMPONENT_TYPE_FLOAT,
            positions.len() as u32,
            "VEC3",
            &data,
            Some(gltf::TARGET_ARRAY_BUFFER),
            Some((min.to_vec(), max.to_vec())),
        )
    }

    fn write_vec3s_no_bounds(&mut self, data: &[[f32; 3]], target: u32) -> u32 {
        let bytes = flatten_le(data);
        self.push_accessor(
            gltf::COMPONENT_TYPE_FLOAT,
            data.len() as u32,
            "VEC3",
            &bytes,
            Some(target),
            None,
        )
    }

    fn write_vec2s(&mut self, data: &[[f32; 2]]) -> u32 {
        let bytes = flatten_le(data);
        self.push_accessor(
            gltf::COMPONENT_TYPE_FLOAT,
            data.len() as u32,
            "VEC2",
            &bytes,
            Some(gltf::TARGET_ARRAY_BUFFER),
            None,
        )
    }

    fn write_indices(&mut self, indices: &[u32]) -> u32 {
        let bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        self.push_accessor(
            gltf::COMPONENT_TYPE_UNSIGNED_INT,
            indices.len() as u32,
            "SCALAR",
            &bytes,
            Some(gltf::TARGET_ELEMENT_ARRAY_BUFFER),
            None,
        )
    }

    /// The spec requires `min`/`max` on the accessor referenced by `animation.sampler.input`
    /// (unlike other non-POSITION accessors), so those are always computed here.
    fn write_scalar_times(&mut self, times: &[f64]) -> u32 {
        let values: Vec<f32> = times.iter().map(|&t| t as f32).collect();
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let (min, max) = scalar_min_max(&values);
        self.push_accessor(
            gltf::COMPONENT_TYPE_FLOAT,
            values.len() as u32,
            "SCALAR",
            &bytes,
            None,
            Some((vec![min], vec![max])),
        )
    }

    /// Writes an animation sampler output accessor. `N` selects the glTF accessor type (3 =>
    /// VEC3, 4 => VEC4); no bufferView `target`, since accessors used only by animation samplers
    /// don't get one.
    fn write_floatn<const N: usize>(&mut self, data: &[[f32; N]], target: Option<u32>) -> u32 {
        let type_ = match N {
            3 => "VEC3",
            4 => "VEC4",
            _ => unreachable!("animation channels are only ever VEC3 (translation/scale) or VEC4 (rotation)"),
        };
        let bytes = flatten_le(data);
        self.push_accessor(
            gltf::COMPONENT_TYPE_FLOAT,
            data.len() as u32,
            type_,
            &bytes,
            target,
            None,
        )
    }
}

fn flatten_le<const N: usize>(data: &[[f32; N]]) -> Vec<u8> {
    data.iter().flatten().flat_map(|c: &f32| c.to_le_bytes()).collect()
}

fn position_min_max(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = positions[0];
    let mut max = positions[0];
    for p in &positions[1..] {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    (min, max)
}

/// Assumes `values` is non-empty: callers only reach this once `Channel::input` has already been
/// checked non-empty (see `push_channel`).
fn scalar_min_max(values: &[f32]) -> (f32, f32) {
    let mut min = values[0];
    let mut max = values[0];
    for &v in &values[1..] {
        min = min.min(v);
        max = max.max(v);
    }
    (min, max)
}

// --- GLB container framing (12-byte header + JSON chunk + optional BIN chunk) ---

fn write_glb_container(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let json_padded = pad(json, b' ');
    let bin_padded = pad(bin, 0);
    let has_bin = !bin_padded.is_empty();

    let total_len = 12 + 8 + json_padded.len() + if has_bin { 8 + bin_padded.len() } else { 0 };

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());

    out.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_padded);

    if has_bin {
        out.extend_from_slice(&(bin_padded.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin_padded);
    }

    out
}

fn pad(data: &[u8], fill: u8) -> Vec<u8> {
    let mut padded = data.to_vec();
    while !padded.len().is_multiple_of(4) {
        padded.push(fill);
    }
    padded
}

// --- GLB reading: the inverse of the writing above ---
//
// Plain `Result<_, String>`, not `PyResult`: this is a malformed-input case (a bad path, or a
// file that isn't valid GLB/glTF), which belongs to whoever exposes this to Python to turn into a
// `PyValueError` with the caller's context - not something this pyo3-free module should decide.

/// Parses a GLB container into its JSON chunk (still padded with trailing spaces, which
/// `serde_json` tolerates as whitespace) and its BIN chunk (still padded with trailing zero
/// bytes, harmless since accessors only ever read their own declared `byteOffset`/`byteLength`
/// range - never the padding beyond it).
fn read_glb_container(data: &[u8]) -> Result<(&[u8], &[u8]), String> {
    if data.len() < 12 {
        return Err("not a GLB file: shorter than the 12-byte header".to_string());
    }
    if &data[0..4] != b"glTF" {
        return Err("not a GLB file: missing the 'glTF' magic bytes".to_string());
    }
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!("unsupported glTF container version {version} (only version 2 is supported)"));
    }
    let declared_len = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    if declared_len != data.len() {
        return Err(format!(
            "GLB header declares a total length of {declared_len} bytes, but the file is {} bytes",
            data.len()
        ));
    }

    let (json, rest) = read_chunk(&data[12..], *b"JSON")?;
    let bin = if rest.is_empty() { &[][..] } else { read_chunk(rest, *b"BIN\0")?.0 };
    Ok((json, bin))
}

fn read_chunk(data: &[u8], expected_type: [u8; 4]) -> Result<(&[u8], &[u8]), String> {
    if data.len() < 8 {
        return Err("truncated GLB chunk header".to_string());
    }
    let chunk_len = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let chunk_type = &data[4..8];
    if chunk_type != expected_type {
        return Err(format!(
            "expected a {:?} chunk, found {:?}",
            String::from_utf8_lossy(&expected_type),
            String::from_utf8_lossy(chunk_type)
        ));
    }
    if data.len() < 8 + chunk_len {
        return Err("GLB chunk declares a length longer than the remaining file".to_string());
    }
    Ok((&data[8..8 + chunk_len], &data[8 + chunk_len..]))
}

/// Parses a GLB file's bytes into its glTF document and binary buffer. The inverse of
/// `export_glb` + `write_glb_container`, and the entry point for everything downstream (node
/// traversal, meshing, animation) that reads a `.glb` path rather than an in-memory `Scene`.
///
/// Not called anywhere yet outside this module's own tests: node traversal (the next piece of
/// the CPU-rasterizer rewrite) is what actually consumes it. Left `#[allow(dead_code)]` rather
/// than landing unused-and-unreachable, so this step is independently reviewable/testable.
#[allow(dead_code)]
pub fn read_glb(data: &[u8]) -> Result<(gltf::Root, Vec<u8>), String> {
    let (json, bin) = read_glb_container(data)?;
    let root: gltf::Root = serde_json::from_slice(json).map_err(|e| format!("invalid glTF JSON: {e}"))?;
    Ok((root, bin.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_frames_json_and_bin_chunks_with_padding() {
        let json = b"{}"; // 2 bytes -> padded to 4 with spaces
        let bin = &[1u8, 2, 3][..]; // 3 bytes -> padded to 4 with a zero
        let out = write_glb_container(json, bin);

        assert_eq!(&out[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(out[4..8].try_into().unwrap()), 2);
        let total_len = u32::from_le_bytes(out[8..12].try_into().unwrap());
        assert_eq!(total_len as usize, out.len());

        let json_chunk_len = u32::from_le_bytes(out[12..16].try_into().unwrap());
        assert_eq!(json_chunk_len, 4);
        assert_eq!(&out[16..20], b"JSON");
        assert_eq!(&out[20..24], b"{}  ");

        let bin_chunk_len = u32::from_le_bytes(out[24..28].try_into().unwrap());
        assert_eq!(bin_chunk_len, 4);
        assert_eq!(&out[28..32], b"BIN\0");
        assert_eq!(&out[32..36], &[1, 2, 3, 0]);
    }

    #[test]
    fn container_omits_bin_chunk_when_empty() {
        let out = write_glb_container(b"{}", &[]);
        assert_eq!(out.len(), 12 + 8 + 4);
        let total_len = u32::from_le_bytes(out[8..12].try_into().unwrap());
        assert_eq!(total_len as usize, out.len());
    }

    #[test]
    fn bin_writer_aligns_views_to_four_bytes() {
        let mut writer = BinWriter::default();
        writer.push_view(&[1, 2, 3], None); // 3 bytes, unaligned length
        let second = writer.push_view(&[4, 5], None);
        assert_eq!(writer.buffer_views[second as usize].byte_offset, 4);
    }

    #[test]
    fn position_min_max_folds_component_wise() {
        let positions = [[1.0, -2.0, 3.0], [-1.0, 5.0, 0.0]];
        let (min, max) = position_min_max(&positions);
        assert_eq!(min, [-1.0, -2.0, 0.0]);
        assert_eq!(max, [1.0, 5.0, 3.0]);
    }

    #[test]
    fn flatten_le_produces_little_endian_bytes() {
        let data = [[1.0f32, 2.0]];
        let bytes = flatten_le(&data);
        assert_eq!(bytes, [1.0f32.to_le_bytes(), 2.0f32.to_le_bytes()].concat());
    }

    #[test]
    fn read_glb_container_is_the_inverse_of_write_glb_container() {
        // Both already 4-byte aligned, so no padding is added and the round trip is exact.
        let json = b"{}  ";
        let bin = &[1u8, 2, 3, 4][..];
        let out = write_glb_container(json, bin);

        let (parsed_json, parsed_bin) = read_glb_container(&out).unwrap();
        assert_eq!(parsed_json, json);
        assert_eq!(parsed_bin, bin);
    }

    #[test]
    fn read_glb_container_includes_padding_bytes_verbatim() {
        // "{}" pads to "{}  " (spaces); [1,2,3] pads to [1,2,3,0] - the reader hands back the
        // padded chunk as-is, since it doesn't know the writer's original unpadded length.
        let out = write_glb_container(b"{}", &[1, 2, 3]);
        let (json, bin) = read_glb_container(&out).unwrap();
        assert_eq!(json, b"{}  ");
        assert_eq!(bin, &[1, 2, 3, 0]);
    }

    #[test]
    fn read_glb_container_omits_bin_when_absent() {
        let out = write_glb_container(b"{}", &[]);
        let (json, bin) = read_glb_container(&out).unwrap();
        assert_eq!(json, b"{}  ");
        assert!(bin.is_empty());
    }

    #[test]
    fn read_glb_container_rejects_bad_magic() {
        let err = read_glb_container(b"NOPE totally not a glb file").unwrap_err();
        assert!(err.contains("magic"));
    }

    #[test]
    fn read_glb_container_rejects_wrong_version() {
        let mut out = write_glb_container(b"{}", &[]);
        out[4..8].copy_from_slice(&99u32.to_le_bytes());
        let err = read_glb_container(&out).unwrap_err();
        assert!(err.contains("version"));
    }

    #[test]
    fn read_glb_container_rejects_truncated_file() {
        let out = write_glb_container(b"{}", &[1, 2, 3, 4]);
        let err = read_glb_container(&out[..out.len() - 2]).unwrap_err();
        assert!(err.contains("length"));
    }

    #[test]
    fn read_glb_round_trips_a_scene_built_the_same_way_export_glb_does() {
        // Exercises the writer's own helpers directly (no pyo3/GIL needed here, unlike
        // `export_glb` itself), then feeds the result through the reader end to end.
        let mut root = gltf::Root::default();
        let mut writer = BinWriter::default();

        let position = writer.write_positions(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let normal = writer.write_vec3s_no_bounds(&[[0.0, 0.0, 1.0]; 3], gltf::TARGET_ARRAY_BUFFER);
        let texcoord_0 = writer.write_vec2s(&[[0.0, 0.0]; 3]);
        let indices = writer.write_indices(&[0, 1, 2]);

        root.materials.push(gltf::Material::default());
        root.meshes.push(gltf::Mesh {
            name: Some("triangle".to_string()),
            primitives: vec![gltf::Primitive {
                attributes: gltf::Attributes {
                    position,
                    normal,
                    texcoord_0,
                },
                indices,
                material: 0,
            }],
            extras: None,
        });
        root.nodes.push(gltf::Node {
            name: Some("root".to_string()),
            mesh: Some(0),
            ..Default::default()
        });
        root.scenes.push(gltf::Scene { nodes: vec![0] });
        root.scene = Some(0);
        root.accessors = writer.accessors;
        root.buffer_views = writer.buffer_views;
        root.buffers.push(gltf::Buffer {
            byte_length: writer.bytes.len() as u32,
        });

        let json_bytes = serde_json::to_vec(&root).unwrap();
        let glb_bytes = write_glb_container(&json_bytes, &writer.bytes);

        let (parsed, bin) = read_glb(&glb_bytes).unwrap();
        assert_eq!(parsed.nodes.len(), 1);
        assert_eq!(parsed.nodes[0].name.as_deref(), Some("root"));
        assert_eq!(parsed.nodes[0].mesh, Some(0));
        assert_eq!(parsed.meshes[0].primitives[0].indices, indices);
        assert_eq!(bin.len(), writer.bytes.len());
        assert_eq!(bin, writer.bytes);
    }

    #[test]
    fn read_glb_rejects_invalid_json() {
        let out = write_glb_container(b"not json", &[]);
        let err = read_glb(&out).unwrap_err();
        assert!(err.contains("invalid glTF JSON"));
    }
}

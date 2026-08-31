// <ai-owned/>

//! Renders a `.glb` file to PNGs - a pure-CPU rasterizer (`raster.rs`), no GPU/driver dependency,
//! assembled from `glb`/`gltf` (reading), `scene_graph`/`animation` (world-space geometry),
//! `camera` (projection) and `shading`/`texture` (materials). There is no programmatic
//! `Scene.render`/`Model.render` API any more: run `python -m voxels.preview TARGET ...` on a
//! `.glb`, or on a `.py` that builds a `scene`/`model` - [`preview_glb`] resolves that first.
//! `crate::preview` only parses argv with `clap` and calls into here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glam::Vec3;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyAnyMethods;
use pyo3::{Bound, PyAny, PyResult, Python};

mod animation;
mod camera;
mod raster;
mod scene_graph;
mod shading;
mod texture;

use camera::Camera;
use raster::{Framebuffer, ScreenVertex};
use scene_graph::WorldPrimitive;

use super::utils::encode_png;
use super::{glb, gltf};
use crate::scene::Scene;

const SUPERSAMPLE: u32 = 2;

/// Renders `glb_bytes` to PNGs under `output_dir`, one per `(time, angle)` pair, in that order.
/// Runtime failures (bad `.glb` content, filesystem errors) surface as `anyhow` errors with
/// context - CLI *argument* parsing errors don't reach here at all, `clap` handles those itself
/// before `preview_glb` is ever called.
fn render_glb(
    glb_bytes: &[u8],
    angles: &[crate::preview::Angle],
    times: &[f64],
    animation_name: Option<&str>,
    include: &[String],
    exclude: &[String],
    output_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let (root, bin) = glb::read_glb(glb_bytes).context("failed to parse the .glb file")?;

    let animation = match animation_name {
        Some(name) => Some(
            root.animations
                .iter()
                .find(|a| a.name.as_deref() == Some(name))
                .with_context(|| format!("no animation named {name:?} in this .glb file"))?,
        ),
        None => None,
    };
    let time_values: Vec<f64> = if animation.is_some() && !times.is_empty() {
        times.to_vec()
    } else {
        vec![0.0]
    };

    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;

    let mut files = Vec::with_capacity(time_values.len() * angles.len());
    for (time_idx, &time) in time_values.iter().enumerate() {
        let primitives = scene_graph::collect_world_primitives(&root, &bin, animation, time, include, exclude)
            .context("failed to collect scene geometry")?;
        let (min, max) = scene_graph::world_bounds(&primitives);
        let (center, radius) = camera::bounds_center_radius(min, max);
        let base_distance = camera::fit_distance(radius, camera::default_fov_y_rad(), camera::FIT_PADDING);

        for (angle_idx, angle) in angles.iter().enumerate() {
            let cam = camera::place_camera(center, base_distance, angle.yaw, angle.pitch, angle.zoom);
            let (width, height, pixels) = render_frame(&root, &bin, &primitives, &cam)?;
            let file = output_dir.join(format!("t{time_idx}_a{angle_idx}.png"));
            std::fs::write(&file, encode_png(width, height, &pixels, png::ColorType::Rgb))
                .with_context(|| format!("failed to write {}", file.display()))?;
            files.push(file);
        }
    }
    Ok(files)
}

fn render_frame(
    root: &gltf::Root,
    bin: &[u8],
    primitives: &[WorldPrimitive],
    camera: &Camera,
) -> Result<(u32, u32, Vec<u8>)> {
    let screen_size = camera::RESOLUTION * SUPERSAMPLE;
    let mut fb = Framebuffer::new(screen_size, screen_size, [0.0; 3]);
    fb.fill_background(shading::background_gradient);
    let mut material_cache: HashMap<u32, shading::DecodedMaterial> = HashMap::new();

    // No backface culling: voxel meshing only ever emits outward-facing quads (no coincident
    // back faces exist to cull), so this is a pure perf optimization opportunity, not a
    // correctness gap - the z-buffer already resolves occlusion correctly either way.
    let (opaque, mut transmissive): (Vec<_>, Vec<_>) = primitives.iter().partition(|p| {
        !root.materials[p.material as usize]
            .extensions
            .as_ref()
            .is_some_and(|e| e.transmission.is_some())
    });

    for primitive in &opaque {
        draw_primitive(
            &mut fb,
            root,
            bin,
            &mut material_cache,
            primitive,
            camera,
            screen_size as f32,
            false,
        )?;
    }
    // Back-to-front, so a transmissive fragment's background sample sees whatever's already
    // drawn behind it (the opaque scene, plus any farther transmissive layers).
    transmissive.sort_by(|a, b| {
        centroid_distance(b, camera.position)
            .partial_cmp(&centroid_distance(a, camera.position))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for primitive in &transmissive {
        draw_primitive(
            &mut fb,
            root,
            bin,
            &mut material_cache,
            primitive,
            camera,
            screen_size as f32,
            true,
        )?;
    }

    Ok(raster::downsample_to_srgb8(&fb, SUPERSAMPLE))
}

fn centroid_distance(primitive: &WorldPrimitive, camera_pos: Vec3) -> f32 {
    let n = primitive.positions.len().max(1) as f32;
    let sum = primitive
        .positions
        .iter()
        .fold(Vec3::ZERO, |acc, &p| acc + Vec3::from(p));
    (sum / n).distance(camera_pos)
}

#[allow(clippy::too_many_arguments)]
fn draw_primitive(
    fb: &mut Framebuffer,
    root: &gltf::Root,
    bin: &[u8],
    material_cache: &mut HashMap<u32, shading::DecodedMaterial>,
    primitive: &WorldPrimitive,
    camera: &Camera,
    screen_size: f32,
    transmissive: bool,
) -> Result<()> {
    if let std::collections::hash_map::Entry::Vacant(e) = material_cache.entry(primitive.material) {
        e.insert(
            shading::decode_material(root, bin, primitive.material)
                .with_context(|| format!("failed to decode material {}", primitive.material))?,
        );
    }
    let material = &material_cache[&primitive.material];

    for tri in primitive.indices.as_chunks::<3>().0 {
        let vertices: Vec<Option<ScreenVertex<8>>> = tri
            .iter()
            .map(|&i| screen_vertex(camera, primitive, i as usize, screen_size))
            .collect();
        let (Some(v0), Some(v1), Some(v2)) = (vertices[0], vertices[1], vertices[2]) else {
            continue; // a vertex behind the camera - see raster.rs's documented policy
        };
        raster::rasterize_triangle(fb, v0, v1, v2, |_, _, attrs, background| {
            let normal = Vec3::new(attrs[0], attrs[1], attrs[2]);
            let uv = [attrs[3], attrs[4]];
            let world_pos = Vec3::new(attrs[5], attrs[6], attrs[7]);
            let view_dir = (camera.position - world_pos).normalize_or_zero();
            let surface = shading::shade_opaque(material, normal, view_dir, uv);
            let color = if transmissive {
                shading::blend_transmission(material, surface, Vec3::from(background), normal, view_dir, uv)
            } else {
                surface
            };
            Some(color.to_array())
        });
    }
    Ok(())
}

fn screen_vertex(camera: &Camera, primitive: &WorldPrimitive, i: usize, screen_size: f32) -> Option<ScreenVertex<8>> {
    let world_pos = Vec3::from(primitive.positions[i]);
    let (x, y, depth, inv_w) = camera::project_to_screen(camera, world_pos, screen_size)?;
    let n = primitive.normals[i];
    let uv = primitive.uvs[i];
    Some(ScreenVertex {
        x,
        y,
        depth,
        inv_w,
        attributes: [n[0], n[1], n[2], uv[0], uv[1], world_pos.x, world_pos.y, world_pos.z],
    })
}

// --- CLI ---
//
// Argument *parsing* is entirely `clap`'s job now (`crate::preview::Args`, built with
// `#[derive(Parser)]`) - by the time `preview_glb` sees an `Args`, it's already well-formed
// (required fields present, `--angle`/`--time` already the right types). What's left is
// resolving the target, applying the "no --angle means one default view" fallback, and running
// the actual render, which can still fail at runtime (bad path, bad `.glb`/`.py` content, filesystem).

/// Resolves a preview target to glb bytes [`preview_glb`] can read: a `.glb` file is read as-is,
/// a `.py` file is run and its scene serialized via [`scene_file_to_glb_bytes`] - entirely in
/// memory, no intermediate file either way.
fn target_to_glb_bytes(py: Python<'_>, target: &Path) -> Result<Vec<u8>> {
    if target.extension().and_then(|ext| ext.to_str()) == Some("py") {
        Ok(scene_file_to_glb_bytes(py, target)?)
    } else {
        std::fs::read(target).with_context(|| format!("failed to read {}", target.display()))
    }
}

/// Runs a `.py` target and serializes whatever scene it builds straight to glb bytes - the same
/// bytes `Scene.export_glb` would write to a file, without writing one.
///
/// The file is executed with `runpy` (so `python -m voxels.preview` and running the script
/// directly resolve imports the same way); it must leave a module-level `scene`, or a `model`
/// that gets wrapped in a single-node scene.
fn scene_file_to_glb_bytes(py: Python<'_>, path: &Path) -> PyResult<Vec<u8>> {
    let namespace = py.import("runpy")?.call_method1("run_path", (path,))?;

    let scene: Bound<'_, PyAny> = match namespace.get_item("scene") {
        Ok(scene) => scene,
        Err(_) => {
            let model = namespace.get_item("model").map_err(|_| {
                PyValueError::new_err(format!(
                    "{} must define a module-level `scene` or `model`",
                    path.display()
                ))
            })?;
            let scene = py.import("voxels")?.getattr("Scene")?.call0()?;
            scene
                .call_method1("create_root_node", ("root",))?
                .call_method1("add_model", ("model", model))?;
            scene
        }
    };

    let scene: Bound<'_, Scene> = scene.extract().map_err(|_| {
        PyValueError::new_err(format!(
            "{} must define `scene`/`model` as voxels.Scene/voxels.Model objects",
            path.display()
        ))
    })?;
    glb::export_glb(scene)
}

/// Renders a preview `Args`'s target, resolving a `.py` target to glb bytes first (see
/// [`target_to_glb_bytes`]) - the single entry point `crate::preview::_preview` calls into.
pub fn preview_glb(py: Python<'_>, args: crate::preview::Args) -> Result<Vec<PathBuf>> {
    let crate::preview::Args {
        target,
        mut angles,
        times,
        anim: animation,
        include,
        exclude,
        out,
    } = args;
    let glb_bytes = target_to_glb_bytes(py, &target).context("failed to resolve preview target")?;
    if angles.is_empty() {
        angles.push(crate::preview::Angle {
            yaw: 45.0,
            pitch: 25.0,
            zoom: None,
        });
    }
    let output_dir = out.unwrap_or_else(default_output_dir);
    render_glb(
        &glb_bytes,
        &angles,
        &times,
        animation.as_deref(),
        &include,
        &exclude,
        &output_dir,
    )
}

/// A fresh temp directory per invocation. Not a UUID crate: a process id + timestamp + counter
/// is already unique enough for "don't collide with a concurrent CLI invocation", and this
/// avoids adding a dependency for it.
fn default_output_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("voxels-{:x}-{nanos:x}-{counter:x}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(path: &str) -> crate::preview::Args {
        crate::preview::Args {
            target: path.into(),
            angles: Vec::new(),
            times: Vec::new(),
            anim: None,
            include: Vec::new(),
            exclude: Vec::new(),
            out: None,
        }
    }

    #[test]
    fn run_cli_on_an_unreadable_path_is_an_error_not_a_panic() {
        Python::initialize();
        Python::attach(|py| {
            assert!(preview_glb(py, args("/no/such/file.glb")).is_err());
        });
    }
}

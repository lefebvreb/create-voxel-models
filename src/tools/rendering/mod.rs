// <ai-owned/>

//! Renders a `.glb` file to PNGs - a pure-CPU rasterizer (`raster.rs`), no GPU/driver dependency,
//! assembled from `glb`/`gltf` (reading), `scene_graph`/`animation` (world-space geometry),
//! `camera` (projection) and `shading`/`texture` (materials). There is no programmatic
//! `Scene.render`/`Model.render` API any more: export to `.glb` (`Scene.export_glb`), then run
//! `python -m voxels.preview FILE.glb ...` (see `crate::preview`, which parses argv with `clap`
//! and calls [`run_cli`] below).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::Vec3;

use crate::preview::PreviewError;

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

const SUPERSAMPLE: u32 = 2;

/// Renders `glb_bytes` to PNGs under `output_dir`, one per `(time, angle)` pair, in that order.
/// See `crate::preview::PreviewError`'s doc comment for the error-handling policy - CLI
/// *argument* parsing errors don't reach here at all, `clap` handles those itself before
/// `preview_glb` is ever called.
fn render_glb(
    glb_bytes: &[u8],
    angles: &[crate::preview::Angle],
    times: &[f64],
    animation_name: Option<&str>,
    include: &[String],
    exclude: &[String],
    output_dir: &Path,
) -> Result<Vec<PathBuf>, PreviewError> {
    let (root, bin) = glb::read_glb(glb_bytes).map_err(PreviewError)?;

    let animation = match animation_name {
        Some(name) => Some(
            root.animations
                .iter()
                .find(|a| a.name.as_deref() == Some(name))
                .ok_or_else(|| PreviewError(format!("no animation named {name:?} in this .glb file")))?,
        ),
        None => None,
    };
    let time_values: Vec<f64> = if animation.is_some() && !times.is_empty() {
        times.to_vec()
    } else {
        vec![0.0]
    };

    std::fs::create_dir_all(output_dir).map_err(|e| {
        PreviewError(format!(
            "failed to create output directory {}: {e}",
            output_dir.display()
        ))
    })?;

    let mut files = Vec::with_capacity(time_values.len() * angles.len());
    for (time_idx, &time) in time_values.iter().enumerate() {
        let primitives = scene_graph::collect_world_primitives(&root, &bin, animation, time, include, exclude)
            .map_err(PreviewError)?;
        let (min, max) = scene_graph::world_bounds(&primitives);
        let (center, radius) = camera::bounds_center_radius(min, max);
        let base_distance = camera::fit_distance(radius, camera::default_fov_y_rad(), camera::FIT_PADDING);

        for (angle_idx, angle) in angles.iter().enumerate() {
            let cam = camera::place_camera(center, base_distance, angle.yaw, angle.pitch, angle.zoom);
            let (width, height, pixels) = render_frame(&root, &bin, &primitives, &cam)?;
            let file = output_dir.join(format!("t{time_idx}_a{angle_idx}.png"));
            std::fs::write(&file, encode_png(width, height, &pixels, png::ColorType::Rgb))
                .map_err(|e| PreviewError(format!("failed to write {}: {e}", file.display())))?;
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
) -> Result<(u32, u32, Vec<u8>), PreviewError> {
    let screen_size = camera::RESOLUTION * SUPERSAMPLE;
    let mut fb = Framebuffer::new(screen_size, screen_size, shading::clear_color_linear());
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
) -> Result<(), PreviewError> {
    if let std::collections::hash_map::Entry::Vacant(e) = material_cache.entry(primitive.material) {
        e.insert(shading::decode_material(root, bin, primitive.material).map_err(PreviewError)?);
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
// `#[derive(Parser)]`) - by the time `run_cli` sees an `Args`, it's already well-formed
// (required fields present, `--angle`/`--time` already the right types). This function only
// deals with what's left: applying the "no --angle means one default view" fallback and running
// the actual render, which can still fail at runtime (bad path, bad `.glb` content, filesystem).

pub fn preview_glb(args: crate::preview::Args) -> Result<Vec<PathBuf>, PreviewError> {
    let crate::preview::Args {
        glb: path,
        mut angles,
        times,
        anim: animation,
        include,
        exclude,
        out,
    } = args;
    if angles.is_empty() {
        angles.push(crate::preview::Angle {
            yaw: 45.0,
            pitch: 25.0,
            zoom: None,
        });
    }
    let glb_bytes =
        std::fs::read(&path).map_err(|e| PreviewError(format!("failed to read {}: {e}", path.display())))?;
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
            glb: path.into(),
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
        assert!(preview_glb(args("/no/such/file.glb")).is_err());
    }
}

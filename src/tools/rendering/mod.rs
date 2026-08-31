// <ai-owned/>

//! Renders a `.glb` file to PNGs - a pure-CPU rasterizer (`raster.rs`), no GPU/driver dependency,
//! assembled from `glb`/`gltf` (reading), `scene_graph`/`animation` (world-space geometry),
//! `camera` (projection) and `shading`/`texture` (materials).
//!
//! The only entry point is the preview CLI: `python -m voxels.preview TARGET ...` on a `.glb`, or
//! on a `.py` that builds a module-level `scene`/`model` - [`preview_glb`] resolves that first.
//! `crate::preview` parses argv with `clap` and calls into here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyAnyMethods;
use pyo3::{Bound, Py, PyResult, Python};

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
use crate::model::Model;
use crate::scene::{Node, Scene};

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
    if !times.is_empty() && animation.is_none() {
        bail!("--time needs --anim: there is no animation to sample without one");
    }
    let time_values: Vec<f64> = if times.is_empty() { vec![0.0] } else { times.to_vec() };

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
    let mut frame = Frame::new(root, bin, camera);

    // No backface culling: voxel meshing only ever emits outward-facing quads, so this is a perf
    // opportunity, not a correctness gap - the z-buffer resolves occlusion either way.
    let (opaque, transmissive): (Vec<_>, Vec<_>) = primitives.iter().partition(|p| !is_transmissive(root, p));

    for primitive in &opaque {
        frame.draw(primitive, false)?;
    }
    // Back-to-front, so a transmissive fragment's background sample sees whatever's already
    // drawn behind it (the opaque scene, plus any farther transmissive layers).
    let mut transmissive: Vec<(f32, &WorldPrimitive)> = transmissive
        .iter()
        .map(|&p| (centroid_distance(p, camera.position), p))
        .collect();
    transmissive.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, primitive) in &transmissive {
        frame.draw(primitive, true)?;
    }

    Ok(raster::downsample_to_srgb8(&frame.fb, SUPERSAMPLE))
}

fn is_transmissive(root: &gltf::Root, primitive: &WorldPrimitive) -> bool {
    // `.get`, not a bare index: `collect_world_primitives` doesn't range-check `primitive.material`
    // against `root.materials`, so a corrupt `.glb` could point past the end. An unresolved
    // material is treated as opaque here and errors later in `decode_material` with context.
    root.materials
        .get(primitive.material as usize)
        .and_then(|m| m.extensions.as_ref())
        .is_some_and(|e| e.transmission.is_some())
}

fn centroid_distance(primitive: &WorldPrimitive, camera_pos: Vec3) -> f32 {
    let n = primitive.positions.len().max(1) as f32;
    let sum = primitive
        .positions
        .iter()
        .fold(Vec3::ZERO, |acc, &p| acc + Vec3::from(p));
    (sum / n).distance(camera_pos)
}

/// Everything one rendered frame needs: the parsed document and camera it draws from, the pixel
/// buffer it fills, and a material-decode cache reused across every primitive in the frame.
struct Frame<'a> {
    root: &'a gltf::Root,
    bin: &'a [u8],
    camera: &'a Camera,
    screen_size: f32,
    fb: Framebuffer,
    material_cache: HashMap<u32, shading::DecodedMaterial>,
}

impl<'a> Frame<'a> {
    fn new(root: &'a gltf::Root, bin: &'a [u8], camera: &'a Camera) -> Self {
        let screen_size = camera::RESOLUTION * SUPERSAMPLE;
        let mut fb = Framebuffer::new(screen_size, screen_size, [0.0; 3]);
        fb.fill_background(shading::background_gradient);
        Self {
            root,
            bin,
            camera,
            screen_size: screen_size as f32,
            fb,
            material_cache: HashMap::new(),
        }
    }

    /// Decodes `material` on first reference, then keeps it for the rest of the frame.
    fn cache_material(&mut self, material: u32) -> Result<()> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.material_cache.entry(material) {
            e.insert(
                shading::decode_material(self.root, self.bin, material)
                    .with_context(|| format!("failed to decode material {material}"))?,
            );
        }
        Ok(())
    }

    /// Rasterizes every triangle of `primitive`, shading each fragment opaque and (when
    /// `transmissive`) compositing it against whatever is already in the framebuffer behind it.
    fn draw(&mut self, primitive: &WorldPrimitive, transmissive: bool) -> Result<()> {
        self.cache_material(primitive.material)?;
        let material = &self.material_cache[&primitive.material];
        let (camera, screen_size, fb) = (self.camera, self.screen_size, &mut self.fb);

        for tri in primitive.indices.as_chunks::<3>().0 {
            let v: [Option<ScreenVertex<8>>; 3] =
                std::array::from_fn(|k| screen_vertex(camera, primitive, tri[k] as usize, screen_size));
            let (Some(v0), Some(v1), Some(v2)) = (v[0], v[1], v[2]) else {
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
// Argument parsing is `clap`'s job (`crate::preview::Args`, `#[derive(Parser)]`): by the time
// `preview_glb` sees an `Args` it is already well-formed. What is left is resolving the target,
// applying the "no --angle means one default view" fallback, and running the render itself, which
// can still fail at runtime (bad path, bad `.glb`/`.py` content, filesystem).

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

    let scene: Bound<'_, Scene> = match namespace.get_item("scene") {
        Ok(scene) => scene
            .extract()
            .map_err(|_| PyValueError::new_err(format!("{}'s `scene` is not a voxels.Scene", path.display())))?,
        // No `scene`: fall back to a lone `model`, wrapped in a one-node scene assembled straight
        // from the Rust types - no round trip through the `voxels` Python module.
        Err(_) => {
            let model: Py<Model> = namespace
                .get_item("model")
                .ok()
                .and_then(|obj| obj.extract().ok())
                .ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "{} must define a module-level `scene` (voxels.Scene) or `model` (voxels.Model)",
                        path.display()
                    ))
                })?;
            let scene = Bound::new(py, Scene::new())?;
            let root = Scene::create_root_node(scene.clone(), "root".to_string(), None)?;
            Node::add_model(root.bind(py).clone(), "model".to_string(), model, None)?;
            scene
        }
    };

    Ok(glb::export_glb(scene))
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

/// A fresh temp directory per invocation, named with a random UUID so concurrent CLI runs (and
/// repeated runs that reuse a pid) never collide.
fn default_output_dir() -> PathBuf {
    std::env::temp_dir().join(format!("voxels-{}", uuid::Uuid::new_v4().simple()))
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

    #[test]
    fn time_without_anim_is_an_error() {
        Python::initialize();
        Python::attach(|py| {
            let glb = glb::export_glb(Bound::new(py, Scene::new()).unwrap());
            let angle = crate::preview::Angle {
                yaw: 0.0,
                pitch: 0.0,
                zoom: None,
            };
            let err = render_glb(&glb, &[angle], &[1.0], None, &[], &[], Path::new("/nonexistent"))
                .unwrap_err()
                .to_string();
            assert!(err.contains("--time needs --anim"), "got {err:?}");
        });
    }
}

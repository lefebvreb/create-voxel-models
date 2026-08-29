// <ai-owned/>

//! Renders a `.glb` file to PNGs - a pure-CPU rasterizer (`raster.rs`), no GPU/driver dependency,
//! assembled from `glb`/`gltf` (reading), `scene_graph`/`animation` (world-space geometry),
//! `camera` (projection) and `shading`/`texture` (materials). There is no programmatic
//! `Scene.render`/`Model.render` API any more: export to `.glb` (`Scene.export_glb`), then call
//! `voxels.main()` (below) on that path.
//!
//! **`python -m voxels` doesn't reach `main` today.** Making a compiled pyo3 extension module
//! runnable via `-m` needs a `voxels.__main__` submodule that only initializes when specifically
//! resolved that way - but registering one is only reachable through the *parent* module's own
//! init (there's no pyo3 hook for "someone is importing `voxels.__main__` specifically"), so
//! anything placed there runs on every plain `import voxels` too, including the ordinary
//! `Scene`/`Model`/`Palette` usage that already relies on that import working normally. A
//! correct lazy-loading submodule needs either a real on-disk `__main__.py` (a Python source
//! file) or a custom import hook - both bigger and riskier than attempting blind here. `main` is
//! a plain, always-correct `#[pyfunction]` instead: call `voxels.main()` (reads real `sys.argv`)
//! or add a one-line `[project.scripts]` entry pointing at it for a real `voxels-preview`-style
//! command - either gets you a working CLI without this gap.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::Vec3;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::types::PyAnyMethods;
use pyo3::{PyResult, Python, pyfunction};

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

/// A camera position, parsed from a `--angle YAW,PITCH[,ZOOM]` CLI flag.
struct Angle {
    yaw: f64,
    pitch: f64,
    zoom: Option<f64>,
}

impl Angle {
    fn parse(s: &str) -> Result<Self, RenderError> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(RenderError::InvalidInput(format!(
                "invalid --angle {s:?}: expected YAW,PITCH or YAW,PITCH,ZOOM"
            )));
        }
        let number = |raw: &str| -> Result<f64, RenderError> {
            raw.trim()
                .parse::<f64>()
                .map_err(|_| RenderError::InvalidInput(format!("invalid --angle {s:?}: {raw:?} is not a number")))
        };
        Ok(Angle {
            yaw: number(parts[0])?,
            pitch: number(parts[1])?,
            zoom: parts.get(2).map(|z| number(z)).transpose()?,
        })
    }
}

/// User-input error (bad CLI args, malformed `.glb` content) vs. environment error (filesystem).
/// Never a panic here - a panic means a library bug, not a caller mistake. See the
/// renderer-rewrite plan's "Error handling" section.
#[derive(Debug)]
enum RenderError {
    InvalidInput(String),
    Io(String),
}

impl From<String> for RenderError {
    fn from(message: String) -> Self {
        RenderError::InvalidInput(message)
    }
}

/// Renders `glb_bytes` to PNGs under `output_dir`, one per `(time, angle)` pair, in that order.
fn render_glb(
    glb_bytes: &[u8],
    angles: &[Angle],
    times: &[f64],
    animation_name: Option<&str>,
    include: &[String],
    exclude: &[String],
    output_dir: &Path,
) -> Result<Vec<PathBuf>, RenderError> {
    let (root, bin) = glb::read_glb(glb_bytes)?;

    let animation = match animation_name {
        Some(name) => Some(
            root.animations
                .iter()
                .find(|a| a.name.as_deref() == Some(name))
                .ok_or_else(|| RenderError::InvalidInput(format!("no animation named {name:?} in this GLB")))?,
        ),
        None => None,
    };
    let time_values: Vec<f64> = if animation.is_some() && !times.is_empty() {
        times.to_vec()
    } else {
        vec![0.0]
    };

    std::fs::create_dir_all(output_dir).map_err(|e| {
        RenderError::Io(format!(
            "failed to create output directory {}: {e}",
            output_dir.display()
        ))
    })?;

    let mut files = Vec::with_capacity(time_values.len() * angles.len());
    for (time_idx, &time) in time_values.iter().enumerate() {
        let primitives = scene_graph::collect_world_primitives(&root, &bin, animation, time, include, exclude)?;
        let (min, max) = scene_graph::world_bounds(&primitives);
        let (center, radius) = camera::bounds_center_radius(min, max);
        let base_distance = camera::fit_distance(radius, camera::default_fov_y_rad(), camera::FIT_PADDING);

        for (angle_idx, angle) in angles.iter().enumerate() {
            let cam = camera::place_camera(center, base_distance, angle.yaw, angle.pitch, angle.zoom);
            let (width, height, pixels) = render_frame(&root, &bin, &primitives, &cam)?;
            let file = output_dir.join(format!("t{time_idx}_a{angle_idx}.png"));
            std::fs::write(&file, encode_png(width, height, &pixels, png::ColorType::Rgb))
                .map_err(|e| RenderError::Io(format!("failed to write screenshot to {}: {e}", file.display())))?;
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
) -> Result<(u32, u32, Vec<u8>), RenderError> {
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
) -> Result<(), RenderError> {
    if let std::collections::hash_map::Entry::Vacant(e) = material_cache.entry(primitive.material) {
        e.insert(shading::decode_material(root, bin, primitive.material)?);
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

fn run_cli(args: &[String]) -> Result<Vec<PathBuf>, RenderError> {
    let mut path = None;
    let mut angles = Vec::new();
    let mut times = Vec::new();
    let mut animation = None;
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    let mut output = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--angle" => angles.push(Angle::parse(next_value(&mut it, "--angle")?)?),
            "--time" => times.push(
                next_value(&mut it, "--time")?
                    .parse::<f64>()
                    .map_err(|_| RenderError::InvalidInput("invalid --time value".to_string()))?,
            ),
            "--animation" => animation = Some(next_value(&mut it, "--animation")?.to_string()),
            "--include" => include.push(next_value(&mut it, "--include")?.to_string()),
            "--exclude" => exclude.push(next_value(&mut it, "--exclude")?.to_string()),
            "--out" => output = Some(PathBuf::from(next_value(&mut it, "--out")?)),
            other if !other.starts_with("--") && path.is_none() => path = Some(other.to_string()),
            other => return Err(RenderError::InvalidInput(format!("unrecognized argument {other:?}"))),
        }
    }
    let path = path.ok_or_else(|| {
        RenderError::InvalidInput(
            "usage: python -m voxels FILE.glb [--angle YAW,PITCH[,ZOOM]]... [--time T]... \
             [--animation NAME] [--include NAME]... [--exclude NAME]... [--out DIR]"
                .to_string(),
        )
    })?;
    if angles.is_empty() {
        angles.push(Angle {
            yaw: 45.0,
            pitch: 25.0,
            zoom: None,
        });
    }
    // A bad input path is the caller's mistake (like a bad --angle), not an environment problem
    // - InvalidInput, not Io. Only writing *output* below is treated as an environment failure.
    let glb_bytes =
        std::fs::read(&path).map_err(|e| RenderError::InvalidInput(format!("failed to read {path}: {e}")))?;
    let output_dir = output.unwrap_or_else(default_output_dir);
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

fn next_value<'a>(it: &mut std::slice::Iter<'a, String>, flag: &str) -> Result<&'a str, RenderError> {
    it.next()
        .map(String::as_str)
        .ok_or_else(|| RenderError::InvalidInput(format!("{flag} needs a value")))
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

/// The CLI entry point: `voxels.main()` reads real `sys.argv` (so it works as a
/// `[project.scripts]` target with zero extra glue if one is ever added), or pass `args`
/// explicitly to drive it programmatically/from tests. **`python -m voxels` does not reach this
/// today** - see the module doc comment; call `voxels.main()` directly, or add a
/// `[project.scripts]` entry pointing at it, until/unless that's revisited. Prints each written
/// PNG's path, one per line.
#[pyfunction]
#[pyo3(signature = (args=None))]
pub fn main(py: Python<'_>, args: Option<Vec<String>>) -> PyResult<()> {
    let args = match args {
        Some(args) => args,
        None => {
            let argv: Vec<String> = py.import("sys")?.getattr("argv")?.extract()?;
            argv.into_iter().skip(1).collect()
        }
    };
    match run_cli(&args) {
        Ok(files) => {
            for file in files {
                println!("{}", file.display());
            }
            Ok(())
        }
        Err(RenderError::InvalidInput(message)) => Err(PyValueError::new_err(message)),
        Err(RenderError::Io(message)) => Err(PyRuntimeError::new_err(message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_parses_yaw_pitch() {
        let a = Angle::parse("45,25").unwrap();
        assert_eq!((a.yaw, a.pitch, a.zoom), (45.0, 25.0, None));
    }

    #[test]
    fn angle_parses_yaw_pitch_zoom() {
        let a = Angle::parse("45,25,1.5").unwrap();
        assert_eq!((a.yaw, a.pitch, a.zoom), (45.0, 25.0, Some(1.5)));
    }

    #[test]
    fn angle_rejects_malformed_input() {
        assert!(Angle::parse("45").is_err());
        assert!(Angle::parse("a,b").is_err());
    }

    #[test]
    fn run_cli_without_a_path_is_an_invalid_input_error_not_a_panic() {
        let err = run_cli(&[]).err().unwrap();
        assert!(matches!(err, RenderError::InvalidInput(_)));
    }
}

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::camera::RenderTarget;
use bevy::camera::primitives::Aabb;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::gltf::Gltf;
use bevy::image::Image;
use bevy::light::{DirectionalLight, GlobalAmbientLight};
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::{Extent3d, PollType, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::renderer::RenderDevice;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::{ExitCondition, WindowPlugin};
use bevy::world_serialization::{WorldAssetRoot, WorldInstance, WorldInstanceSpawner};
use glam::DVec3;
use pyo3::exceptions::PyValueError;
use pyo3::{Bound, PyResult, pyclass, pymethods};

use crate::glb::export_glb;
use crate::scene::Scene;
use crate::utils::encode_rgb_png;

const RESOLUTION: u32 = 512;
const MAX_POLL_TICKS: u32 = 300;
const FIT_PADDING: f64 = 1.3;
const MIN_RADIUS: f64 = 0.5;
const MAX_PITCH_DEG: f64 = 89.9;

#[pyclass(frozen, from_py_object)]
#[derive(Copy, Clone)]
pub struct CameraAngle {
    #[pyo3(get)]
    pub yaw: f64,
    #[pyo3(get)]
    pub pitch: f64,
    #[pyo3(get)]
    pub zoom: Option<f64>,
}

#[pymethods]
impl CameraAngle {
    #[new]
    #[pyo3(signature = (yaw, pitch, *, zoom = None))]
    fn __new__(yaw: f64, pitch: f64, zoom: Option<f64>) -> Self {
        Self { yaw, pitch, zoom }
    }
}

#[pyclass(frozen, get_all)]
pub struct RenderOutput {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    scene: Bound<Scene>,
    angles: Vec<CameraAngle>,
    times: Vec<f64>,
    animation: Option<String>,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    background: Option<(u8, u8, u8)>,
    output_dir: Option<PathBuf>,
) -> PyResult<RenderOutput> {
    if angles.is_empty() {
        return Err(PyValueError::new_err("angles must not be empty"));
    }
    let include = include.unwrap_or_default();
    let exclude = exclude.unwrap_or_default();
    let glb_bytes = export_glb(scene)?;

    let mut app = build_headless_app(glb_bytes)?;

    let (scene_handle, animation_clip) = load_gltf(&mut app, animation.as_deref())?;

    let world_root = app.world_mut().spawn(WorldAssetRoot(scene_handle)).id();
    poll_until(&mut app, |app| {
        let world = app.world();
        Ok(world
            .get::<WorldInstance>(world_root)
            .is_some_and(|instance| world.resource::<WorldInstanceSpawner>().instance_is_ready(**instance)))
    })?;

    fix_transmission_texture_format(app.world_mut());
    apply_visibility(app.world_mut(), world_root, &include, &exclude);

    let animation_setup = animation_clip.map(|clip| setup_animation(app.world_mut(), clip));

    let bg_color = background_color(background);
    setup_lighting(app.world_mut());

    let target_handle = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(new_render_target_image());
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(bg_color),
                ..default()
            },
            RenderTarget::from(target_handle.clone()),
            Projection::Perspective(PerspectiveProjection::default()),
            // Bypass filmic tonemapping: an agent inspecting colors for correctness needs the
            // material colors it authored, not a hue-shifting display curve. `TonyMcMapface`
            // (Bevy's default) would also silently render everything wrong without the
            // `tonemapping_luts` feature, which we deliberately don't enable.
            Tonemapping::None,
            Transform::IDENTITY,
        ))
        .id();

    // Settle transform/visibility propagation before measuring bounds.
    app.update();
    let (min, max) = compute_bounds(app.world_mut());
    let (center, radius) = bounds_center_radius(min, max);
    let fov_y = PerspectiveProjection::default().fov as f64;
    let base_distance = fit_distance(radius, fov_y, FIT_PADDING);

    let output_dir = output_dir.unwrap_or_else(default_output_dir);
    std::fs::create_dir_all(&output_dir)?;

    let time_values: Vec<f64> = match &animation_setup {
        Some(_) if !times.is_empty() => times,
        Some(_) => vec![0.0],
        None => vec![0.0],
    };

    let mut files = Vec::with_capacity(time_values.len() * angles.len());
    for (time_idx, &time) in time_values.iter().enumerate() {
        if let Some((entities, node_index)) = &animation_setup {
            seek_animation(app.world_mut(), entities, *node_index, time);
        }
        for (angle_idx, angle) in angles.iter().enumerate() {
            position_camera(app.world_mut(), camera, center, base_distance, angle);
            let (width, height, pixels) = capture_screenshot(&mut app, &target_handle)?;
            let filename = output_filename(time_idx, angle_idx);
            std::fs::write(output_dir.join(&filename), encode_rgb_png(width, height, &pixels)?)?;
            files.push(filename);
        }
    }

    Ok(RenderOutput { dir: output_dir, files })
}

// --- Headless app setup ---

fn build_headless_app(glb_bytes: Vec<u8>) -> PyResult<App> {
    let dir = Dir::default();
    dir.insert_asset(Path::new("scene.glb"), glb_bytes);

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() }) as _),
    );
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            }),
    );
    app.finish();
    app.cleanup();
    Ok(app)
}

fn load_gltf(
    app: &mut App,
    animation: Option<&str>,
) -> PyResult<(
    Handle<bevy::world_serialization::WorldAsset>,
    Option<Handle<AnimationClip>>,
)> {
    let handle: Handle<Gltf> = app.world().resource::<AssetServer>().load("scene.glb");
    poll_until(app, |app| {
        let server = app.world().resource::<AssetServer>();
        match server.load_state(handle.id()) {
            LoadState::Failed(err) => Err(PyValueError::new_err(format!(
                "failed to load exported scene into the renderer: {err}"
            ))),
            _ => Ok(server.is_loaded_with_dependencies(handle.id())),
        }
    })?;

    let gltf_assets = app.world().resource::<Assets<Gltf>>();
    let gltf = gltf_assets
        .get(&handle)
        .expect("gltf finished loading, so its asset must be present");
    let scene_handle = gltf
        .default_scene
        .clone()
        .or_else(|| gltf.scenes.first().cloned())
        .ok_or_else(|| PyValueError::new_err("exported scene contains no glTF scene"))?;
    let clip = match animation {
        Some(name) => Some(
            gltf.named_animations
                .get(name)
                .cloned()
                .ok_or_else(|| PyValueError::new_err(format!("no animation named {name:?} on this scene")))?,
        ),
        None => None,
    };
    Ok((scene_handle, clip))
}

fn tick(app: &mut App) {
    app.update();
    let _ = app
        .world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });
}

fn poll_until(app: &mut App, mut ready: impl FnMut(&mut App) -> PyResult<bool>) -> PyResult<()> {
    for _ in 0..MAX_POLL_TICKS {
        tick(app);
        if ready(app)? {
            return Ok(());
        }
    }
    Err(PyValueError::new_err(
        "timed out waiting for the headless renderer to become ready",
    ))
}

// --- Material fixup ---

/// Bevy 0.19.1's glTF importer marks `KHR_materials_transmission`'s texture as sRGB, though the
/// glTF spec defines it as linear data. Endpoints (0/255) are unaffected, but intermediate
/// transmission values render less transmissive than authored. The pixel bytes are already
/// correct `Rgba8`; only the format tag the GPU reads them with needs fixing.
fn fix_transmission_texture_format(world: &mut World) {
    let handles: Vec<Handle<Image>> = world
        .resource::<Assets<StandardMaterial>>()
        .iter()
        .filter_map(|(_, material)| material.specular_transmission_texture.clone())
        .collect();
    let mut images = world.resource_mut::<Assets<Image>>();
    for handle in handles {
        if let Some(mut image) = images.get_mut(&handle) {
            image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
        }
    }
}

// --- Visibility filtering ---

fn apply_visibility(world: &mut World, world_root: Entity, include: &[String], exclude: &[String]) {
    if !include.is_empty() {
        if let Some(root_nodes) = scene_root_nodes(world, world_root) {
            for entity in root_nodes {
                world.entity_mut(entity).insert(Visibility::Hidden);
            }
        }
        set_visibility_by_name(world, include, Visibility::Visible);
    }
    if !exclude.is_empty() {
        set_visibility_by_name(world, exclude, Visibility::Hidden);
    }
}

fn scene_root_nodes(world: &World, world_root: Entity) -> Option<Vec<Entity>> {
    let scene_entity = world.get::<Children>(world_root)?.first().copied()?;
    Some(world.get::<Children>(scene_entity)?.iter().collect())
}

fn set_visibility_by_name(world: &mut World, names: &[String], visibility: Visibility) {
    let matches: Vec<Entity> = world
        .query::<(Entity, &Name)>()
        .iter(world)
        .filter(|(_, name)| names.iter().any(|n| n.as_str() == name.as_str()))
        .map(|(entity, _)| entity)
        .collect();
    for entity in matches {
        world.entity_mut(entity).insert(visibility);
    }
}

// --- Animation ---

fn setup_animation(world: &mut World, clip: Handle<AnimationClip>) -> (Vec<Entity>, AnimationNodeIndex) {
    let (graph, node_index) = AnimationGraph::from_clip(clip);
    let graph_handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);

    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<AnimationPlayer>>()
        .iter(world)
        .collect();
    for &entity in &entities {
        world
            .entity_mut(entity)
            .insert(AnimationGraphHandle(graph_handle.clone()));
        if let Some(mut player) = world.get_mut::<AnimationPlayer>(entity) {
            player.play(node_index).pause();
        }
    }
    (entities, node_index)
}

fn seek_animation(world: &mut World, entities: &[Entity], node_index: AnimationNodeIndex, time: f64) {
    for &entity in entities {
        if let Some(mut player) = world.get_mut::<AnimationPlayer>(entity)
            && let Some(active) = player.animation_mut(node_index)
        {
            active.set_seek_time(time as f32);
        }
    }
}

// --- Lighting/background ---

fn setup_lighting(world: &mut World) {
    world.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.5, -1.0, -0.3), Vec3::Y),
    ));
    world.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 120.0,
        ..default()
    });
}

fn background_color(background: Option<(u8, u8, u8)>) -> Color {
    let (r, g, b) = background.unwrap_or((46, 46, 46));
    Color::srgb_u8(r, g, b)
}

// --- Camera bounds and placement ---

const CORNER_SIGNS: [Vec3; 8] = [
    Vec3::new(-1.0, -1.0, -1.0),
    Vec3::new(1.0, -1.0, -1.0),
    Vec3::new(-1.0, 1.0, -1.0),
    Vec3::new(1.0, 1.0, -1.0),
    Vec3::new(-1.0, -1.0, 1.0),
    Vec3::new(1.0, -1.0, 1.0),
    Vec3::new(-1.0, 1.0, 1.0),
    Vec3::new(1.0, 1.0, 1.0),
];

fn compute_bounds(world: &mut World) -> (DVec3, DVec3) {
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    let mut found = false;

    let mut query = world.query::<(&GlobalTransform, &Aabb, &InheritedVisibility)>();
    for (transform, aabb, visibility) in query.iter(world) {
        if !visibility.get() {
            continue;
        }
        found = true;
        for &sign in &CORNER_SIGNS {
            let local = Vec3::from(aabb.center) + Vec3::from(aabb.half_extents) * sign;
            let world_pos = transform.transform_point(local);
            let world_pos = DVec3::new(world_pos.x as f64, world_pos.y as f64, world_pos.z as f64);
            min = min.min(world_pos);
            max = max.max(world_pos);
        }
    }

    if found {
        (min, max)
    } else {
        (DVec3::splat(-0.5), DVec3::splat(0.5))
    }
}

fn bounds_center_radius(min: DVec3, max: DVec3) -> (DVec3, f64) {
    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).length().max(MIN_RADIUS);
    (center, radius)
}

fn spherical_position(center: DVec3, distance: f64, yaw_deg: f64, pitch_deg: f64) -> DVec3 {
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg
        .to_radians()
        .clamp(-MAX_PITCH_DEG.to_radians(), MAX_PITCH_DEG.to_radians());
    center + distance * DVec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos())
}

fn fit_distance(radius: f64, fov_y_rad: f64, padding: f64) -> f64 {
    (radius * padding) / (fov_y_rad / 2.0).sin()
}

fn position_camera(world: &mut World, camera: Entity, center: DVec3, base_distance: f64, angle: &CameraAngle) {
    let distance = base_distance / angle.zoom.unwrap_or(1.0);
    let position = spherical_position(center, distance, angle.yaw, angle.pitch);
    let position = Vec3::new(position.x as f32, position.y as f32, position.z as f32);
    let center = Vec3::new(center.x as f32, center.y as f32, center.z as f32);
    let transform = Transform::from_translation(position).looking_at(center, Vec3::Y);
    world.entity_mut(camera).insert(transform);
}

// --- Screenshot capture ---

fn new_render_target_image() -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width: RESOLUTION,
            height: RESOLUTION,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
    image
}

fn capture_screenshot(app: &mut App, target: &Handle<Image>) -> PyResult<(u32, u32, Vec<u8>)> {
    let captured: Arc<Mutex<Option<Image>>> = Arc::new(Mutex::new(None));
    let sink = captured.clone();
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(move |trigger: On<ScreenshotCaptured>| {
            *sink.lock().unwrap() = Some(trigger.image.clone());
        });

    poll_until(app, |_| Ok(captured.lock().unwrap().is_some()))?;

    let image = captured
        .lock()
        .unwrap()
        .take()
        .expect("poll_until only returns once captured is set");
    let rgb = image
        .try_into_dynamic()
        .map_err(|e| PyValueError::new_err(e.to_string()))?
        .to_rgb8();
    let (width, height) = (rgb.width(), rgb.height());
    Ok((width, height, rgb.into_raw()))
}

// --- Output paths ---

fn default_output_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("voxels-render-{}-{}", std::process::id(), nanos))
}

fn output_filename(time_idx: usize, angle_idx: usize) -> PathBuf {
    PathBuf::from(format!("t{time_idx}_a{angle_idx}.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_center_radius_folds_min_max() {
        let (center, radius) = bounds_center_radius(DVec3::new(-1.0, -2.0, -3.0), DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(center, DVec3::ZERO);
        assert!((radius - DVec3::new(1.0, 2.0, 3.0).length()).abs() < 1e-9);
    }

    #[test]
    fn bounds_center_radius_floors_degenerate_case() {
        let (_, radius) = bounds_center_radius(DVec3::ZERO, DVec3::ZERO);
        assert_eq!(radius, MIN_RADIUS);
    }

    #[test]
    fn spherical_position_front_view_at_zero_yaw_zero_pitch() {
        let pos = spherical_position(DVec3::ZERO, 10.0, 0.0, 0.0);
        assert!((pos - DVec3::new(0.0, 0.0, 10.0)).length() < 1e-9);
    }

    #[test]
    fn spherical_position_top_down_at_ninety_pitch_clamped() {
        let pos = spherical_position(DVec3::ZERO, 10.0, 0.0, 90.0);
        // Clamped just under 90 degrees, so mostly +Y with a tiny horizontal residual.
        assert!(pos.y > 9.99);
        assert!(pos.y < 10.0);
    }

    #[test]
    fn fit_distance_grows_with_radius() {
        let fov = std::f64::consts::FRAC_PI_4;
        assert!(fit_distance(2.0, fov, 1.3) > fit_distance(1.0, fov, 1.3));
    }

    #[test]
    fn output_filenames_are_row_major_times_outer_angles_inner() {
        assert_eq!(output_filename(0, 0), PathBuf::from("t0_a0.png"));
        assert_eq!(output_filename(0, 1), PathBuf::from("t0_a1.png"));
        assert_eq!(output_filename(1, 0), PathBuf::from("t1_a0.png"));
    }
}

// <ai-owned/>

use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::uuid::Uuid;
use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::camera::primitives::Aabb;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::gltf::Gltf;
use bevy::image::Image;
use bevy::light::{DirectionalLight, EnvironmentMapLight, light_consts};
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::{
    Extent3d, PollType, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor, TextureViewDimension
};
use bevy::render::renderer::RenderDevice;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::{ExitCondition, WindowPlugin};
use bevy::world_serialization::{WorldAssetRoot, WorldInstance, WorldInstanceSpawner};
use glam::DVec3;
use png::ColorType;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::{Bound, PyResult};

use super::glb::export_glb;
use super::utils::encode_png;
use crate::render::{CameraAngle, RenderOutput};
use crate::scene::Scene;

const RESOLUTION: u32 = 448;
const MAX_POLL_TICKS: u32 = 300;
const FIT_PADDING: f64 = 1.3;
const MIN_RADIUS: f64 = 0.5;
const MAX_PITCH_DEG: f64 = 89.9;

pub fn render(
    scene: Bound<Scene>,
    angles: Vec<CameraAngle>,
    times: Vec<f64>,
    animation: Option<String>,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
) -> PyResult<RenderOutput> {
    if angles.is_empty() {
        return Err(PyValueError::new_err("angles must not be empty"));
    }
    if let Some(name) = &animation
        && !scene.borrow().anims.contains_key(name)
    {
        return Err(PyValueError::new_err(format!(
            "no animation named {name:?} on this scene"
        )));
    }
    let include = include.unwrap_or_default();
    let exclude = exclude.unwrap_or_default();
    let glb_bytes = export_glb(scene)?;

    with_render_app(|ra| {
        let (scene_handle, animation_clip) = ra.load_gltf(glb_bytes, animation.as_deref())?;

        let world_root = ra.app.world_mut().spawn(WorldAssetRoot(scene_handle)).id();
        poll_until(&mut ra.app, |app| {
            let world = app.world();
            Ok(world
                .get::<WorldInstance>(world_root)
                .is_some_and(|instance| world.resource::<WorldInstanceSpawner>().instance_is_ready(**instance)))
        })?;

        fix_transmission_texture_format(ra.app.world_mut());
        apply_visibility(ra.app.world_mut(), world_root, &include, &exclude);
        let animation_setup = animation_clip.map(|clip| setup_animation(ra.app.world_mut(), clip));

        if let Some(mut camera) = ra.app.world_mut().get_mut::<Camera>(ra.camera) {
            camera.clear_color = ClearColorConfig::Custom(Color::srgb_u8(46, 46, 46));
        }

        // Settle transform/visibility propagation before measuring bounds.
        ra.app.update();
        let (min, max) = compute_bounds(ra.app.world_mut());
        let (center, radius) = bounds_center_radius(min, max);
        let fov_y = PerspectiveProjection::default().fov as f64;
        let base_distance = fit_distance(radius, fov_y, FIT_PADDING);

        let output_dir = output_dir();
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("failed to create output directory {}: {e}", output_dir.display()),
            )
        })?;

        let time_values: Vec<f64> = match &animation_setup {
            Some(_) if !times.is_empty() => times,
            Some(_) => vec![0.0],
            None => vec![0.0],
        };

        let mut files = Vec::with_capacity(time_values.len() * angles.len());
        for (time_idx, &time) in time_values.iter().enumerate() {
            if let Some((entities, node_index)) = &animation_setup {
                seek_animation(ra.app.world_mut(), entities, *node_index, time);
            }
            for (angle_idx, angle) in angles.iter().enumerate() {
                position_camera(ra.app.world_mut(), ra.camera, center, base_distance, angle);
                let (width, height, pixels) = capture_screenshot(&mut ra.app, &ra.target)?;
                let file = output_dir.join(output_filename(time_idx, angle_idx));
                std::fs::write(&file, encode_png(width, height, &pixels, ColorType::Rgb)).map_err(|e| {
                    std::io::Error::new(
                        e.kind(),
                        format!("failed to write screenshot to {}: {e}", file.display()),
                    )
                })?;
                files.push(file);
            }
        }

        // The spawned scene (and everything under it) is specific to this call; despawning it
        // (recursive by default) drops the last strong handles to its meshes/materials/images too,
        // so Bevy's asset GC reclaims them before the next call builds a fresh scene. The camera,
        // lights, and render target are shared app-wide state and stay alive across calls.
        ra.app.world_mut().entity_mut(world_root).despawn();

        Ok(RenderOutput { files })
    })
}

// --- Persistent headless app, reused across render() calls ---
//
// Building a headless Bevy `App` (GPU adapter/device setup, synchronous shader compilation) is
// the dominant cost of a single render, not the actual draw calls - so a fresh `App` per call
// would pay that cost every time. Keeping one alive per thread (an agent script is normally
// single-threaded, and there is no cross-call state to keep it correct even if not) amortizes it
// across a whole session's worth of `Scene.render`/`Model.render` calls.

struct RenderApp {
    app: App,
    dir: Dir,
    camera: Entity,
    target: Handle<Image>,
    next_id: u64,
}

// SAFETY: `App` isn't `Send` solely because of its boxed `runner: Box<dyn FnOnce(App) -> AppExit>`
// field - we never call `App::run()` (the app is driven by hand via `app.update()`), so that
// closure is never invoked or inspected, and there's nothing else thread-affine about it. Every
// access to `RenderApp` goes through the `Mutex` below, which serializes access to at most one
// thread at a time regardless.
unsafe impl Send for RenderApp {}

// A single global instance behind a `Mutex`, not one per thread: this is a dev tool, not
// something meant to render concurrently, so there's no need for more than one headless renderer
// at a time. A plain `static` (unlike a `thread_local`) is simply never dropped at process exit
// under normal termination, which conveniently sidesteps needing to prove `App` is safe to tear
// down during Rust's own shutdown sequence at all.
static RENDER_APP: LazyLock<Mutex<RenderApp>> = LazyLock::new(|| Mutex::new(RenderApp::new()));

fn with_render_app<T>(f: impl FnOnce(&mut RenderApp) -> PyResult<T>) -> PyResult<T> {
    let mut app = RENDER_APP.lock().unwrap();
    f(&mut app)
}

impl RenderApp {
    fn new() -> Self {
        let dir = Dir::default();
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new({
                let dir = dir.clone();
                move || Box::new(MemoryAssetReader { root: dir.clone() }) as _
            }),
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

        let target = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(new_render_target_image());
        let camera = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Camera::default(),
                Hdr,
                RenderTarget::from(target.clone()),
                Projection::Perspective(PerspectiveProjection::default()),
                // Bypass filmic tonemapping: an agent inspecting colors for correctness needs
                // the material colors it authored, not a hue-shifting display curve.
                // `TonyMcMapface` (Bevy's default) would also silently render everything wrong
                // without the `tonemapping_luts` feature, which we deliberately don't enable.
                Tonemapping::None,
                Transform::IDENTITY,
            ))
            .id();

        setup_lighting(app.world_mut(), camera);

        Self {
            app,
            dir,
            camera,
            target,
            next_id: 0,
        }
    }

    fn load_gltf(
        &mut self,
        glb_bytes: Vec<u8>,
        animation: Option<&str>,
    ) -> PyResult<(
        Handle<bevy::world_serialization::WorldAsset>,
        Option<Handle<AnimationClip>>,
    )> {
        self.next_id += 1;
        let path = PathBuf::from(format!("scene-{}.glb", self.next_id));
        self.dir.insert_asset(&path, glb_bytes);

        let handle: Handle<Gltf> = self
            .app
            .world()
            .resource::<AssetServer>()
            .load(path.to_string_lossy().into_owned());
        poll_until(&mut self.app, |app| {
            let server = app.world().resource::<AssetServer>();
            match server.load_state(handle.id()) {
                LoadState::Failed(err) => Err(PyRuntimeError::new_err(format!(
                    "failed to load exported scene into the renderer: {err}"
                ))),
                _ => Ok(server.is_loaded_with_dependencies(handle.id())),
            }
        })?;
        // The in-memory source is only read while loading; the bytes aren't needed once the
        // GLTF/mesh/material/image sub-assets have been parsed out of them.
        self.dir.remove_asset(&path);

        let gltf_assets = self.app.world().resource::<Assets<Gltf>>();
        let gltf = gltf_assets
            .get(&handle)
            .expect("gltf finished loading, so its asset must be present");
        let scene_handle = gltf
            .default_scene
            .clone()
            .or_else(|| gltf.scenes.first().cloned())
            .expect("export_glb always emits exactly one glTF scene");
        let clip = animation.map(|name| {
            gltf.named_animations
                .get(name)
                .cloned()
                .expect("animation name was already validated in render()")
        });
        Ok((scene_handle, clip))
    }
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
    Err(PyRuntimeError::new_err(
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

/// Lights the scene like a neutral product-photography studio rather than a single hard sun:
/// there is no ground plane in this renderer, so a shadow-casting directional light only ever
/// self-shadows the model (an artifact, not a useful depth cue), and a flat ambient term gives
/// metallic materials nothing to reflect. A soft hemispherical environment light replaces the
/// flat ambient with direction-varying fill (its smooth gradient is physically correct for
/// diffuse irradiance), while the directional light stays only for its Lambertian shading
/// gradient (its shadow disabled).
///
/// The diffuse map alone isn't enough to make a mirror-like surface *read* as reflective
/// though: it's uniform across all four side faces, so a specular surface shows only a smooth
/// pitch-based tint, indistinguishable from ordinary diffuse shading. A Lambertian surface can
/// never reproduce spatial structure regardless of the environment, so giving the specular
/// probe an actual checkered pattern (`specular_probe_cubemap`) is what makes reflectivity an
/// unambiguous visual signal: only a specular material will show the pattern, and it will
/// visibly shift as the camera orbits or the model animates.
fn setup_lighting(world: &mut World, camera: Entity) {
    world.spawn((
        DirectionalLight {
            // A gentle form-defining light, not a dominant one: matches Bevy's own basic
            // lighting example (`examples/3d/lighting.rs`) rather than the library default
            // (`AMBIENT_DAYLIGHT`, 10x brighter), which left the environment fill below
            // completely unable to compete - the actual cause of the model's shaded side
            // reading as "in shadow" even with shadow casting already disabled.
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.5, -1.0, -0.3), Vec3::Y),
    ));

    let mut env_light = {
        let mut images = world.resource_mut::<Assets<Image>>();
        let mut env_light = EnvironmentMapLight::hemispherical_gradient(
            &mut images,
            Color::srgb_u8(210, 210, 214),
            Color::srgb_u8(140, 140, 144),
            Color::srgb_u8(70, 70, 74),
        );
        env_light.specular_map = specular_probe_cubemap(&mut images);
        env_light
    };
    // In the 500-2000 range Bevy's own examples use for this field (e.g.
    // `examples/3d/light_probe_blending.rs`, `examples/3d/rotate_environment_map.rs`), and the
    // same order of magnitude as the directional light above rather than two orders below it.
    env_light.intensity = 1500.0;
    world.entity_mut(camera).insert(env_light);
}

const SPECULAR_PROBE_SIZE: u32 = 8;
const SPECULAR_PROBE_BLOCK: u32 = 2;

/// A small checkered cubemap for `EnvironmentMapLight::specular_map`. Unlike the smooth
/// gradient used for diffuse (see `setup_lighting`), the specular probe needs real per-face
/// spatial structure - a uniform face is indistinguishable, in a reflection, from plain
/// ambient shading. This matters most at the level of whole cube faces, not fine texture
/// detail: this renderer draws voxel models, whose surfaces are flat axis-aligned quads, so a
/// flat face's reflection vector barely varies across it and only ever samples a narrow patch
/// of one (or two, near an edge) cubemap face - any *within-face* checker detail mostly
/// blends away. What a flat voxel face *can* show is a stark difference between which of the
/// 6 cubemap faces it happens to reflect, so every face gets a distinct, non-monotonic
/// brightness (not just "top vs. everything else") - two adjacent voxel faces (e.g. front and
/// top) then reflect visibly unrelated tones, which plain Lambertian diffuse shading (whose
/// brightness varies smoothly and consistently with the light direction) can never produce.
/// Grayscale-only so it adds reflection detail without tinting authored material colors.
fn specular_probe_cubemap(images: &mut Assets<Image>) -> Handle<Image> {
    // Face order matches wgpu's cubemap layer convention: +X, -X, +Y, -Y, +Z, -Z.
    // Values are deliberately non-monotonic so no two (let alone adjacent) faces blend into
    // "the same tone" - but, unlike the first attempt at this, kept within a band that never
    // drops near-black: real neutral-studio environments (e.g. the `RoomEnvironment` behind
    // `<model-viewer>`'s default lighting) stay "even from all sides, though not uniform"
    // rather than leaving any direction dark, which is what a wide 5-255 spread was doing to
    // this renderer's metal materials.
    const FACES: [(u8, u8); 6] = [
        (240, 170), // +X
        (155, 85),  // -X
        (255, 185), // +Y (top, brightest)
        (135, 65),  // -Y (bottom, darkest)
        (180, 110), // +Z
        (210, 140), // -Z
    ];

    let mut data = Vec::with_capacity((SPECULAR_PROBE_SIZE * SPECULAR_PROBE_SIZE * 6 * 4) as usize);
    for (light, dark) in FACES {
        for y in 0..SPECULAR_PROBE_SIZE {
            for x in 0..SPECULAR_PROBE_SIZE {
                let checker = (x / SPECULAR_PROBE_BLOCK + y / SPECULAR_PROBE_BLOCK) % 2 == 0;
                let value = if checker { light } else { dark };
                data.extend_from_slice(&[value, value, value, 255]);
            }
        }
    }

    images.add(Image {
        texture_view_descriptor: Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        }),
        ..Image::new(
            Extent3d {
                width: SPECULAR_PROBE_SIZE,
                height: SPECULAR_PROBE_SIZE,
                depth_or_array_layers: 6,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        )
    })
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
        .expect("render target is always constructed as Rgba8UnormSrgb, which is always convertible")
        .to_rgb8();
    let (width, height) = (rgb.width(), rgb.height());
    Ok((width, height, rgb.into_raw()))
}

// --- Output paths ---

fn output_dir() -> PathBuf {
    let mut buf = [0; 32];
    std::env::temp_dir().join(Uuid::new_v4().as_simple().encode_lower(&mut buf))
}

fn output_filename(time_idx: usize, angle_idx: usize) -> String {
    format!("t{time_idx}_a{angle_idx}.png")
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

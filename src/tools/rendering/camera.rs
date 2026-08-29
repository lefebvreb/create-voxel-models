// <ai-owned/>

//! Camera framing/placement math (unchanged in substance from the old Bevy-based renderer's pure
//! functions of the same names - they never touched Bevy) plus the new view/projection matrix
//! and world-to-screen projection this renderer needs instead of a `bevy::Transform`.

use glam::{DVec3, Mat4, Vec3, Vec4Swizzles};

pub const RESOLUTION: u32 = 448;
pub const FIT_PADDING: f64 = 1.3;
pub const MIN_RADIUS: f64 = 0.5;
const MAX_PITCH_DEG: f64 = 89.9;
/// Matches Bevy's `PerspectiveProjection::default().fov`, so framing looks the same as before.
const FOV_Y_DEG: f64 = 45.0;

pub fn bounds_center_radius(min: DVec3, max: DVec3) -> (DVec3, f64) {
    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).length().max(MIN_RADIUS);
    (center, radius)
}

pub fn fit_distance(radius: f64, fov_y_rad: f64, padding: f64) -> f64 {
    (radius * padding) / (fov_y_rad / 2.0).sin()
}

pub fn default_fov_y_rad() -> f64 {
    FOV_Y_DEG.to_radians()
}

pub fn spherical_position(center: DVec3, distance: f64, yaw_deg: f64, pitch_deg: f64) -> DVec3 {
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg
        .to_radians()
        .clamp(-MAX_PITCH_DEG.to_radians(), MAX_PITCH_DEG.to_radians());
    center + distance * DVec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos())
}

pub struct Camera {
    pub view_projection: Mat4,
    pub position: Vec3,
}

pub fn place_camera(center: DVec3, base_distance: f64, yaw_deg: f64, pitch_deg: f64, zoom: Option<f64>) -> Camera {
    let distance = base_distance / zoom.unwrap_or(1.0);
    let position = spherical_position(center, distance, yaw_deg, pitch_deg);
    let position = Vec3::new(position.x as f32, position.y as f32, position.z as f32);
    let center = Vec3::new(center.x as f32, center.y as f32, center.z as f32);

    // Near/far derived from the camera's own distance, not fixed constants: keeps depth
    // precision reasonable at any model scale. See `raster.rs`'s doc comment on the one thing
    // this doesn't handle - a triangle straddling the near plane at extreme zoom.
    let near = (distance * 0.01).max(0.001) as f32;
    let far = (distance * 4.0).max(near as f64 * 2.0) as f32;
    // `look_at_rh`/`perspective_rh` are deprecated in glam 0.33 in favor of `glam::camera`'s
    // handedness/API-specific helpers, but remain correct and are the simplest stable call here;
    // this renderer's z-buffer only needs a monotonic depth (see `raster.rs`), so it doesn't
    // matter which NDC z-range convention is used.
    #[allow(deprecated)]
    let view = Mat4::look_at_rh(position, center, Vec3::Y);
    #[allow(deprecated)]
    let projection = Mat4::perspective_rh(FOV_Y_DEG.to_radians() as f32, 1.0, near, far);
    Camera {
        view_projection: projection * view,
        position,
    }
}

/// Projects a world-space point to screen pixel coordinates against a `screen_size`-square
/// framebuffer. `None` when the point is behind (or exactly at) the camera - per `raster.rs`'s
/// documented policy, such triangles are discarded by the caller rather than clipped here.
/// Returns `(x, y, ndc_depth, inv_w)`.
pub fn project_to_screen(camera: &Camera, world_pos: Vec3, screen_size: f32) -> Option<(f32, f32, f32, f32)> {
    let clip = camera.view_projection * world_pos.extend(1.0);
    if clip.w <= 1e-5 {
        return None;
    }
    let ndc = clip.xyz() / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * screen_size;
    let y = (1.0 - (ndc.y * 0.5 + 0.5)) * screen_size; // NDC is Y-up; screen rows go down.
    Some((x, y, ndc.z, 1.0 / clip.w))
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
        assert!(pos.y > 9.99 && pos.y < 10.0);
    }

    #[test]
    fn fit_distance_grows_with_radius() {
        let fov = std::f64::consts::FRAC_PI_4;
        assert!(fit_distance(2.0, fov, 1.3) > fit_distance(1.0, fov, 1.3));
    }

    #[test]
    fn a_point_at_the_center_projects_to_the_middle_of_the_screen() {
        let camera = place_camera(DVec3::ZERO, 10.0, 0.0, 0.0, None);
        let (x, y, depth, inv_w) = project_to_screen(&camera, Vec3::ZERO, 448.0).unwrap();
        assert!((x - 224.0).abs() < 0.5);
        assert!((y - 224.0).abs() < 0.5);
        assert!((0.0..1.0).contains(&depth));
        assert!(inv_w > 0.0);
    }

    #[test]
    fn a_point_behind_the_camera_does_not_project() {
        let camera = place_camera(DVec3::ZERO, 10.0, 0.0, 0.0, None);
        // The camera looks toward the origin from +Z; a point far behind it along +Z is behind
        // the camera's view direction.
        assert!(project_to_screen(&camera, Vec3::new(0.0, 0.0, camera.position.z + 100.0), 448.0).is_none());
    }
}

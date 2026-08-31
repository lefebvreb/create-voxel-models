// <ai-owned/>

//! A deliberately simple shading model - Lambertian diffuse + a hemisphere ambient gradient + a
//! six-tone reflection cue for metals. The intent is a neutral studio look with no self-shadowing,
//! real material cues, and authored colors rendered as authored. [`AMBIENT_INTENSITY`] and friends
//! are brightness constants tuned by eye against real renders, not physical quantities.
//!
//! Transmission/volume follows `KHR_materials_transmission`/`KHR_materials_volume`'s reference
//! formulas (authored thickness, Beer's law), simplified to straight-through background sampling
//! rather than a laterally-refracted ray: correct tinting/absorption, without bending the sampled
//! background sideways. Full refraction would need the transmission pass to know the screen-space
//! offset a bent ray would land at - a much bigger addition for a subtle effect at this size.

use anyhow::{Context, Result};
use glam::Vec3;

use super::super::gltf;
use super::texture::{Texture, decode_texture};

// --- Tunable brightness - see the module doc comment ---
pub const AMBIENT_INTENSITY: f32 = 0.7;
pub const DIRECTIONAL_INTENSITY: f32 = 1.3;
pub const REFLECTION_INTENSITY: f32 = 0.5;
/// A weak fill from the camera's own direction, added on top of the fixed key light so faces the
/// key light rakes past don't fall all the way to bare ambient - dark matte materials read as
/// holes punched in the mesh against the backdrop otherwise. Deliberately small: the key light
/// still does the form-sculpting and keeps the same face lit the same way across angles.
pub const FILL_INTENSITY: f32 = 0.3;

/// `(-0.5, -1.0, -0.3)` is the direction the key light *points*; the direction *toward* it (what
/// the diffuse term needs) is its negation.
fn light_direction() -> Vec3 {
    -Vec3::new(-0.5, -1.0, -0.3).normalize()
}

const SKY: [u8; 3] = [210, 210, 214];
const EQUATOR: [u8; 3] = [140, 140, 144];
const GROUND: [u8; 3] = [70, 70, 74];
const BACKGROUND_TOP_SRGB: [u8; 3] = [36, 36, 40];
const BACKGROUND_BOTTOM_SRGB: [u8; 3] = [78, 78, 86];

/// The backdrop colour `y_frac` of the way down the frame (`0.0` = top row, `1.0` = bottom),
/// a vertical gradient interpolated in linear space. A single flat fill let dark matte faces
/// collapse into it; grading it lighter toward the bottom keeps a near-black silhouette edged
/// against the backdrop.
pub fn background_gradient(y_frac: f32) -> [f32; 3] {
    linear_rgb_u8(BACKGROUND_TOP_SRGB)
        .lerp(linear_rgb_u8(BACKGROUND_BOTTOM_SRGB), y_frac.clamp(0.0, 1.0))
        .to_array()
}

/// One flat tone per cube face (+X, -X, +Y, -Y, +Z, -Z). On a voxel model's large flat faces the
/// per-face distinction is all that reads anyway, so a real cubemap would buy nothing here.
const REFLECTION_TONES_SRGB: [u8; 6] = [205, 120, 220, 100, 145, 175];

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_rgb_u8(c: [u8; 3]) -> Vec3 {
    Vec3::new(
        srgb_to_linear(c[0] as f32 / 255.0),
        srgb_to_linear(c[1] as f32 / 255.0),
        srgb_to_linear(c[2] as f32 / 255.0),
    )
}

fn hemisphere_ambient(normal_y: f32) -> Vec3 {
    let equator = linear_rgb_u8(EQUATOR);
    if normal_y >= 0.0 {
        equator.lerp(linear_rgb_u8(SKY), normal_y)
    } else {
        equator.lerp(linear_rgb_u8(GROUND), -normal_y)
    }
}

fn reflection_cue(reflected: Vec3) -> Vec3 {
    let (ax, ay, az) = (reflected.x.abs(), reflected.y.abs(), reflected.z.abs());
    let tone = if ax >= ay && ax >= az {
        REFLECTION_TONES_SRGB[if reflected.x >= 0.0 { 0 } else { 1 }]
    } else if ay >= ax && ay >= az {
        REFLECTION_TONES_SRGB[if reflected.y >= 0.0 { 2 } else { 3 }]
    } else {
        REFLECTION_TONES_SRGB[if reflected.z >= 0.0 { 4 } else { 5 }]
    };
    Vec3::splat(srgb_to_linear(tone as f32 / 255.0))
}

/// A material's textures/scalars, decoded once per material and reused across every fragment
/// that references it.
pub struct DecodedMaterial {
    base_color: Texture,
    metallic_roughness: Texture,
    transmission: Option<Texture>,
    ior: f32,
    /// Resolved `emissiveFactor * KHR_materials_emissive_strength` - `None` when there's no
    /// emissive contribution at all (this exporter only ever writes `emissiveFactor` alongside
    /// the strength extension, so their presence is coupled).
    emissive: Option<f32>,
    /// `(attenuation_color (linear), attenuation_distance, thickness)`.
    volume: Option<(Vec3, f32, f32)>,
}

pub fn decode_material(root: &gltf::Root, bin: &[u8], material_index: u32) -> Result<DecodedMaterial> {
    let material = root
        .materials
        .get(material_index as usize)
        .with_context(|| format!("material index {material_index} is out of range"))?;
    let base_color_index = material
        .pbr_metallic_roughness
        .base_color_texture
        .context("material has no base color texture")?
        .index;
    let mr_index = material
        .pbr_metallic_roughness
        .metallic_roughness_texture
        .context("material has no metallic-roughness texture")?
        .index;

    let extensions = material.extensions.as_ref();
    let transmission = match extensions.and_then(|e| e.transmission.as_ref()) {
        Some(t) => Some(decode_texture(root, bin, t.transmission_texture.index)?),
        None => None,
    };
    let ior = extensions.and_then(|e| e.ior.as_ref()).map_or(1.5, |i| i.ior as f32);
    let emissive = material.emissive_factor.map(|_| {
        extensions
            .and_then(|e| e.emissive_strength.as_ref())
            .map_or(1.0, |e| e.emissive_strength as f32)
    });
    let volume = extensions.and_then(|e| e.volume.as_ref()).map(|v| {
        (
            linear_rgb_u8([
                (v.attenuation_color[0] * 255.0).round() as u8,
                (v.attenuation_color[1] * 255.0).round() as u8,
                (v.attenuation_color[2] * 255.0).round() as u8,
            ]),
            v.attenuation_distance as f32,
            v.thickness_factor as f32,
        )
    });

    Ok(DecodedMaterial {
        base_color: decode_texture(root, bin, base_color_index)?,
        metallic_roughness: decode_texture(root, bin, mr_index)?,
        transmission,
        ior,
        emissive,
        volume,
    })
}

/// Shades a fragment as if its surface were fully opaque - the base response every material
/// gets, before `blend_transmission` (if applicable) composites it against the background.
pub fn shade_opaque(material: &DecodedMaterial, world_normal: Vec3, view_dir: Vec3, uv: [f32; 2]) -> Vec3 {
    let base = material.base_color.sample(uv[0], uv[1]);
    let base_color = linear_rgb_u8([base[0], base[1], base[2]]);
    let mr = material.metallic_roughness.sample(uv[0], uv[1]);
    let (roughness, metallic) = (mr[1] as f32 / 255.0, mr[2] as f32 / 255.0);

    let n = world_normal.normalize_or_zero();
    let ambient = hemisphere_ambient(n.y) * AMBIENT_INTENSITY;
    let diffuse = n.dot(light_direction()).max(0.0) * DIRECTIONAL_INTENSITY;
    let fill = n.dot(view_dir).max(0.0) * FILL_INTENSITY;
    let diffuse_response = base_color * (ambient + Vec3::splat(diffuse + fill)) * (1.0 - metallic);

    let reflected = (-view_dir).reflect(n);
    let reflection = reflection_cue(reflected) * REFLECTION_INTENSITY * (1.0 - roughness * 0.5);
    // Dielectrics (metallic=0) get only a ~4% specular weight - the common "F0=0.04" convention
    // real-time PBR shaders use for non-metals (a rough dielectric like wood or plastic reflects
    // its environment weakly, not at full strength); metals ramp up to the full reflection.
    // A flat full-strength highlight on every dielectric otherwise reads as a uniform wash.
    let specular_response = reflection.lerp(reflection * base_color, metallic) * metallic.max(0.04);

    let mut color = diffuse_response + specular_response;
    if let Some(emissive) = material.emissive {
        color += base_color * emissive;
    }
    color
}

/// Composites a transmissive material's own [`shade_opaque`] response against `background`
/// (already-shaded linear color read from the framebuffer at this pixel), per
/// `KHR_materials_transmission`/`KHR_materials_volume`. `view_dir` and `world_normal` drive the
/// Fresnel term that biases how much of the surface's own reflection shows through at grazing
/// angles versus straight transmission face-on.
pub fn blend_transmission(
    material: &DecodedMaterial,
    surface_color: Vec3,
    background: Vec3,
    world_normal: Vec3,
    view_dir: Vec3,
    uv: [f32; 2],
) -> Vec3 {
    let Some(transmission_texture) = &material.transmission else {
        return surface_color;
    };
    let t = transmission_texture.sample(uv[0], uv[1])[0] as f32 / 255.0;
    if t <= 0.0 {
        return surface_color;
    }

    let attenuation = match material.volume {
        Some((color, distance, thickness)) if distance > 0.0 => {
            let exponent = thickness / distance;
            Vec3::new(color.x.powf(exponent), color.y.powf(exponent), color.z.powf(exponent))
        }
        _ => Vec3::ONE,
    };
    let transmitted = background * attenuation;

    let cos_theta = world_normal.normalize_or_zero().dot(view_dir).max(0.0);
    let f0 = ((1.0 - material.ior) / (1.0 + material.ior)).powi(2);
    let fresnel = f0 + (1.0 - f0) * (1.0 - cos_theta).powi(5);

    surface_color.lerp(transmitted, t * (1.0 - fresnel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hemisphere_ambient_is_brightest_straight_up() {
        let up = hemisphere_ambient(1.0);
        let level = hemisphere_ambient(0.0);
        let down = hemisphere_ambient(-1.0);
        assert!(up.x > level.x && level.x > down.x);
    }

    #[test]
    fn reflection_cue_picks_the_dominant_axis() {
        assert_eq!(reflection_cue(Vec3::new(1.0, 0.1, 0.0)), reflection_cue(Vec3::X));
        assert_ne!(reflection_cue(Vec3::X), reflection_cue(Vec3::NEG_X));
    }

    #[test]
    fn background_gradient_grades_lighter_toward_the_bottom_and_clamps() {
        let top = background_gradient(0.0);
        let middle = background_gradient(0.5);
        let bottom = background_gradient(1.0);
        assert!(bottom[0] > middle[0] && middle[0] > top[0]);
        assert_eq!(background_gradient(-1.0), top);
        assert_eq!(background_gradient(2.0), bottom);
    }

    #[test]
    fn fill_light_brightens_a_face_turned_toward_the_camera() {
        let material = DecodedMaterial {
            base_color: dummy_texture(),
            metallic_roughness: dummy_texture(),
            transmission: None,
            ior: 1.5,
            emissive: None,
            volume: None,
        };
        // A normal along +Z catches the same key light and ambient whichever way the camera
        // faces it, so any brightness difference here is the camera-follow fill term.
        let facing = shade_opaque(&material, Vec3::Z, Vec3::Z, [0.5, 0.5]);
        let grazing = shade_opaque(&material, Vec3::Z, Vec3::X, [0.5, 0.5]);
        assert!(facing.length() > grazing.length());
    }

    #[test]
    fn blend_transmission_is_a_no_op_without_a_transmission_texture() {
        let material = DecodedMaterial {
            base_color: dummy_texture(),
            metallic_roughness: dummy_texture(),
            transmission: None,
            ior: 1.5,
            emissive: None,
            volume: None,
        };
        let surface = Vec3::new(0.1, 0.2, 0.3);
        let result = blend_transmission(&material, surface, Vec3::ONE, Vec3::Y, Vec3::Y, [0.5, 0.5]);
        assert_eq!(result, surface);
    }

    fn dummy_texture() -> Texture {
        // 1x1 opaque texture, built the same way `decode_texture` would from a real PNG.
        let png = super::super::super::utils::encode_png(1, 1, &[128, 128, 128], png::ColorType::Rgb);
        let mut root = gltf::Root::default();
        root.buffer_views.push(gltf::BufferView {
            buffer: 0,
            byte_offset: 0,
            byte_length: png.len() as u32,
            target: None,
        });
        root.images.push(gltf::Image {
            mime_type: "image/png".to_string(),
            buffer_view: 0,
        });
        root.textures.push(gltf::Texture { sampler: 0, source: 0 });
        decode_texture(&root, &png, 0).unwrap()
    }
}

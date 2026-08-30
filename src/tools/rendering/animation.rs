// <ai-owned/>

//! Evaluates a parsed glTF animation's `channels`/`samplers` at an arbitrary time `t`, producing
//! a node's local translation/rotation/scale. Pure functions operating on `gltf::Root` + the
//! binary buffer (via `glb.rs`'s accessor decoders) - no pyo3, independently testable like the
//! rest of `tools/`.
//!
//! This is the read-side counterpart to `crate::anim`'s `Channel<T>`/`Interpolation`: that module
//! holds keyframe data as authored (used only to *write* `export_glb`'s animation accessors);
//! this one evaluates keyframe data as *decoded* from a `.glb`'s own accessors, which is what the
//! renderer actually needs (see `Architecture` in the renderer-rewrite plan).

use anyhow::{Context, Result, bail};

use super::super::glb::{decode_floats, decode_vec3s, decode_vec4s};
use super::super::gltf;

/// A node's animated TRS at some time `t`. A `None` field means no channel in this animation
/// targets that node/path - the caller falls back to the node's own static transform, the same
/// convention `gltf::Node`'s own `Option` fields already use.
#[derive(Default, Debug, PartialEq)]
pub struct EvaluatedTrs {
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}

/// Evaluates every channel of `animation` that targets `node_index`, at time `t` (seconds).
pub fn evaluate_node_trs(
    root: &gltf::Root,
    bin: &[u8],
    animation: &gltf::Animation,
    node_index: u32,
    time: f64,
) -> Result<EvaluatedTrs> {
    let mut result = EvaluatedTrs::default();
    let t = time as f32;

    for channel in &animation.channels {
        if channel.target.node != node_index {
            continue;
        }
        let sampler = animation
            .samplers
            .get(channel.sampler as usize)
            .with_context(|| format!("animation channel references out-of-range sampler {}", channel.sampler))?;

        let times = decode_floats(root, bin, sampler.input)?;
        // A channel with no keyframes is skipped on write (`push_channel` in `glb.rs`) and can't
        // be meaningfully sampled here either.
        if times.is_empty() {
            continue;
        }
        let interpolation = parse_interpolation(sampler.interpolation.as_deref());

        match channel.target.path.as_str() {
            "translation" => {
                let values = decode_vec3s(root, bin, sampler.output)?;
                result.translation = Some(sample_vec3(&times, &values, interpolation, t));
            }
            "scale" => {
                let values = decode_vec3s(root, bin, sampler.output)?;
                result.scale = Some(sample_vec3(&times, &values, interpolation, t));
            }
            "rotation" => {
                let values = decode_vec4s(root, bin, sampler.output)?;
                result.rotation = Some(sample_rotation(&times, &values, interpolation, t));
            }
            other => bail!("unsupported animation channel target path {other:?}"),
        }
    }

    Ok(result)
}

#[derive(Clone, Copy)]
enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}

fn parse_interpolation(s: Option<&str>) -> Interpolation {
    match s {
        None | Some("LINEAR") => Interpolation::Linear,
        Some("STEP") => Interpolation::Step,
        Some("CUBICSPLINE") => Interpolation::CubicSpline,
        // An unrecognized string isn't spec-valid; falling back to the spec default (LINEAR)
        // mirrors how the rest of this reader tolerates unmodeled-but-present JSON rather than
        // hard-erroring on it.
        Some(_) => Interpolation::Linear,
    }
}

/// Locates the keyframe segment `[i, i+1]` containing `t`, plus how far into it (`alpha`,
/// clamped to `[0, 1]`) and the segment's duration (`dt`, used only by cubic-spline). Times
/// before the first or after the last keyframe clamp to the first/last segment's boundary -
/// matching the spec's "hold the nearest keyframe" behavior outside the animation's range.
/// Requires `times.len() >= 2`; single-keyframe animations are handled by callers before this.
fn locate(times: &[f32], t: f32) -> (usize, f32, f32) {
    let last = times.len() - 1;
    if t <= times[0] {
        return (0, 0.0, times[1] - times[0]);
    }
    if t >= times[last] {
        return (last - 1, 1.0, times[last] - times[last - 1]);
    }
    let i = match times.binary_search_by(|probe| probe.partial_cmp(&t).expect("keyframe times are never NaN")) {
        Ok(exact) if exact < last => exact,
        Ok(_) => last - 1, // exact match on the final keyframe
        Err(insert_at) => insert_at - 1,
    };
    let (t0, t1) = (times[i], times[i + 1]);
    let alpha = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
    (i, alpha, t1 - t0)
}

fn keyframe3(values: &[[f32; 3]], interpolation: Interpolation, i: usize) -> [f32; 3] {
    match interpolation {
        // Cubic-spline output packs [in-tangent, value, out-tangent] per keyframe.
        Interpolation::CubicSpline => values[i * 3 + 1],
        _ => values[i],
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// Hermite cubic-spline interpolation per the glTF spec's formula, using the out-tangent at `i`
/// and the in-tangent at `i + 1`.
fn hermite3(values: &[[f32; 3]], i: usize, alpha: f32, dt: f32) -> [f32; 3] {
    let p0 = values[i * 3 + 1];
    let m0 = values[i * 3 + 2];
    let p1 = values[(i + 1) * 3 + 1];
    let m1 = values[(i + 1) * 3];
    let (t, t2, t3) = (alpha, alpha * alpha, alpha * alpha * alpha);
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    std::array::from_fn(|c| h00 * p0[c] + h10 * dt * m0[c] + h01 * p1[c] + h11 * dt * m1[c])
}

fn sample_vec3(times: &[f32], values: &[[f32; 3]], interpolation: Interpolation, t: f32) -> [f32; 3] {
    if times.len() == 1 {
        return keyframe3(values, interpolation, 0);
    }
    let (i, alpha, dt) = locate(times, t);
    match interpolation {
        Interpolation::Step => keyframe3(values, interpolation, i),
        Interpolation::Linear => lerp3(
            keyframe3(values, interpolation, i),
            keyframe3(values, interpolation, i + 1),
            alpha,
        ),
        Interpolation::CubicSpline => hermite3(values, i, alpha, dt),
    }
}

fn keyframe4(values: &[[f32; 4]], interpolation: Interpolation, i: usize) -> [f32; 4] {
    match interpolation {
        Interpolation::CubicSpline => values[i * 3 + 1],
        _ => values[i],
    }
}

fn hermite4(values: &[[f32; 4]], i: usize, alpha: f32, dt: f32) -> [f32; 4] {
    let p0 = values[i * 3 + 1];
    let m0 = values[i * 3 + 2];
    let p1 = values[(i + 1) * 3 + 1];
    let m1 = values[(i + 1) * 3];
    let (t, t2, t3) = (alpha, alpha * alpha, alpha * alpha * alpha);
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    std::array::from_fn(|c| h00 * p0[c] + h10 * dt * m0[c] + h01 * p1[c] + h11 * dt * m1[c])
}

/// Rotation gets its own sampler, not a generic-over-N one shared with `sample_vec3`: `LINEAR`
/// interpolation on a quaternion should be spherical (`slerp`), not a per-component lerp - see
/// `crate::anim::Interpolation`'s doc comment, which already documents this for the write side.
/// The cubic-spline result is renormalized after Hermite interpolation, since that formula
/// doesn't preserve unit length on its own.
fn sample_rotation(times: &[f32], values: &[[f32; 4]], interpolation: Interpolation, t: f32) -> [f32; 4] {
    if times.len() == 1 {
        return keyframe4(values, interpolation, 0);
    }
    let (i, alpha, dt) = locate(times, t);
    match interpolation {
        Interpolation::Step => keyframe4(values, interpolation, i),
        Interpolation::Linear => {
            let a = glam::Quat::from_array(keyframe4(values, interpolation, i));
            let b = glam::Quat::from_array(keyframe4(values, interpolation, i + 1));
            a.slerp(b, alpha).to_array()
        }
        Interpolation::CubicSpline => glam::Quat::from_array(hermite4(values, i, alpha, dt))
            .normalize()
            .to_array(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_with_accessors(accessors: Vec<gltf::Accessor>, buffer_views: Vec<gltf::BufferView>) -> gltf::Root {
        gltf::Root {
            accessors,
            buffer_views,
            ..Default::default()
        }
    }

    fn float_accessor(buffer_view: u32, count: u32, type_: &str) -> gltf::Accessor {
        gltf::Accessor {
            buffer_view,
            component_type: gltf::COMPONENT_TYPE_FLOAT,
            count,
            type_: type_.to_string(),
            min: None,
            max: None,
        }
    }

    fn view(byte_offset: u32, byte_length: u32) -> gltf::BufferView {
        gltf::BufferView {
            buffer: 0,
            byte_offset,
            byte_length,
            target: None,
        }
    }

    fn le_bytes(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// Builds a translation-only animation with one channel/sampler targeting `node_index`, with
    /// keyframe `times` (SCALAR) and 3-wide `values` (already in cubic-spline's tripled layout
    /// when `interpolation` is `"CUBICSPLINE"`).
    fn translation_animation(
        node_index: u32,
        times: &[f32],
        values: &[[f32; 3]],
        interpolation: Option<&str>,
    ) -> (gltf::Root, Vec<u8>, gltf::Animation) {
        let mut bin = le_bytes(times);
        let input_view = view(0, bin.len() as u32);
        let input_accessor = float_accessor(0, times.len() as u32, "SCALAR");

        let flat_values: Vec<f32> = values.iter().flatten().copied().collect();
        let output_bytes = le_bytes(&flat_values);
        let output_view = view(bin.len() as u32, output_bytes.len() as u32);
        bin.extend_from_slice(&output_bytes);
        let output_accessor = float_accessor(1, values.len() as u32, "VEC3");

        let root = root_with_accessors(vec![input_accessor, output_accessor], vec![input_view, output_view]);
        let animation = gltf::Animation {
            name: None,
            channels: vec![gltf::AnimationChannel {
                sampler: 0,
                target: gltf::AnimationChannelTarget {
                    node: node_index,
                    path: "translation".to_string(),
                },
            }],
            samplers: vec![gltf::AnimationSampler {
                input: 0,
                output: 1,
                interpolation: interpolation.map(str::to_string),
            }],
            extras: None,
        };
        (root, bin, animation)
    }

    #[test]
    fn linear_interpolates_between_keyframes() {
        let (root, bin, animation) = translation_animation(0, &[0.0, 2.0], &[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]], None);
        let trs = evaluate_node_trs(&root, &bin, &animation, 0, 1.0).unwrap();
        assert_eq!(trs.translation, Some([2.0, 0.0, 0.0]));
        assert_eq!(trs.rotation, None);
        assert_eq!(trs.scale, None);
    }

    #[test]
    fn step_holds_the_previous_keyframe() {
        let (root, bin, animation) = translation_animation(
            0,
            &[0.0, 2.0, 4.0],
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            Some("STEP"),
        );
        let trs = evaluate_node_trs(&root, &bin, &animation, 0, 3.5).unwrap();
        assert_eq!(trs.translation, Some([1.0, 0.0, 0.0]));
    }

    #[test]
    fn time_before_first_keyframe_clamps_to_it() {
        let (root, bin, animation) = translation_animation(0, &[1.0, 2.0], &[[5.0, 0.0, 0.0], [9.0, 0.0, 0.0]], None);
        let trs = evaluate_node_trs(&root, &bin, &animation, 0, -10.0).unwrap();
        assert_eq!(trs.translation, Some([5.0, 0.0, 0.0]));
    }

    #[test]
    fn time_after_last_keyframe_clamps_to_it() {
        let (root, bin, animation) = translation_animation(0, &[1.0, 2.0], &[[5.0, 0.0, 0.0], [9.0, 0.0, 0.0]], None);
        let trs = evaluate_node_trs(&root, &bin, &animation, 0, 99.0).unwrap();
        assert_eq!(trs.translation, Some([9.0, 0.0, 0.0]));
    }

    #[test]
    fn single_keyframe_is_constant() {
        let (root, bin, animation) = translation_animation(0, &[5.0], &[[3.0, 1.0, 4.0]], None);
        assert_eq!(
            evaluate_node_trs(&root, &bin, &animation, 0, 0.0).unwrap().translation,
            Some([3.0, 1.0, 4.0])
        );
        assert_eq!(
            evaluate_node_trs(&root, &bin, &animation, 0, 1000.0)
                .unwrap()
                .translation,
            Some([3.0, 1.0, 4.0])
        );
    }

    #[test]
    fn channels_targeting_a_different_node_are_ignored() {
        let (root, bin, animation) = translation_animation(7, &[0.0, 2.0], &[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]], None);
        let trs = evaluate_node_trs(&root, &bin, &animation, 0, 1.0).unwrap();
        assert_eq!(trs.translation, None);
    }

    #[test]
    fn cubic_spline_passes_through_keyframe_values_at_their_own_time() {
        // At the exact keyframe times, Hermite interpolation always reproduces the authored
        // value regardless of tangents (h00=1/h01=0 at alpha=0, and vice versa at alpha=1).
        let values = [
            [0.0, 0.0, 0.0],  // in-tangent @ t=0 (unused, before the first keyframe)
            [1.0, 0.0, 0.0],  // value @ t=0
            [10.0, 0.0, 0.0], // out-tangent @ t=0
            [0.0, 0.0, 0.0],  // in-tangent @ t=1
            [5.0, 0.0, 0.0],  // value @ t=1
            [0.0, 0.0, 0.0],  // out-tangent @ t=1 (unused, after the last keyframe)
        ];
        let (root, bin, animation) = translation_animation(0, &[0.0, 1.0], &values, Some("CUBICSPLINE"));
        assert_eq!(
            evaluate_node_trs(&root, &bin, &animation, 0, 0.0).unwrap().translation,
            Some([1.0, 0.0, 0.0])
        );
        assert_eq!(
            evaluate_node_trs(&root, &bin, &animation, 0, 1.0).unwrap().translation,
            Some([5.0, 0.0, 0.0])
        );
    }

    #[test]
    fn rotation_linear_slerps_and_stays_normalized() {
        let mut bin = le_bytes(&[0.0, 1.0]);
        let input_view = view(0, bin.len() as u32);
        let input_accessor = float_accessor(0, 2, "SCALAR");

        let a = glam::Quat::from_rotation_y(0.0).to_array();
        let b = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array();
        let flat: Vec<f32> = a.into_iter().chain(b).collect();
        let output_bytes = le_bytes(&flat);
        let output_view = view(bin.len() as u32, output_bytes.len() as u32);
        bin.extend_from_slice(&output_bytes);
        let output_accessor = float_accessor(1, 2, "VEC4");

        let root = root_with_accessors(vec![input_accessor, output_accessor], vec![input_view, output_view]);
        let animation = gltf::Animation {
            name: None,
            channels: vec![gltf::AnimationChannel {
                sampler: 0,
                target: gltf::AnimationChannelTarget {
                    node: 0,
                    path: "rotation".to_string(),
                },
            }],
            samplers: vec![gltf::AnimationSampler {
                input: 0,
                output: 1,
                interpolation: None,
            }],
            extras: None,
        };

        let trs = evaluate_node_trs(&root, &bin, &animation, 0, 0.5).unwrap();
        let rotation = glam::Quat::from_array(trs.rotation.unwrap());
        // Halfway between a 0deg and 90deg y-rotation should be ~45deg, and slerp always
        // produces a unit quaternion.
        assert!((rotation.length() - 1.0).abs() < 1e-5);
        let expected = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        assert!(rotation.angle_between(expected) < 1e-4);
    }

    #[test]
    fn unknown_target_path_is_an_error_not_a_panic() {
        let (root, bin, mut animation) = translation_animation(0, &[0.0], &[[0.0, 0.0, 0.0]], None);
        animation.channels[0].target.path = "weight".to_string(); // morph-target weights: unsupported
        let err = evaluate_node_trs(&root, &bin, &animation, 0, 0.0)
            .unwrap_err()
            .to_string();
        assert!(err.contains("weight"));
    }
}

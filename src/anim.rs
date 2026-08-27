use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

use crate::math::{Quat, Vec3};
use crate::scene::{Node, Scene};
use crate::utils::{Dict, HashPy};

/// A named animation, holding per-node keyframe tracks for translation, rotation and scale.
#[pyclass]
pub struct Anim {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub extras: Option<Dict>,
    pub nodes: HashMap<HashPy<Node>, Trs>,
    #[pyo3(get)]
    pub scene: Py<Scene>,
}

impl Anim {
    fn with_node_trs(slf: Bound<Self>, node: Py<Node>, f: impl FnOnce(&mut Trs)) -> PyResult<()> {
        let mut slf_brw = slf.borrow_mut();
        if !node.get().scene.is(&slf_brw.scene) {
            return Err(PyValueError::new_err("node does not belong to this anim's scene"));
        }
        f(slf_brw.nodes.entry(HashPy(node)).or_default());
        Ok(())
    }
}

#[pymethods]
impl Anim {
    /// Set `node`'s translation track, replacing any existing one.
    ///
    /// Args:
    ///     node: The node to animate; it must belong to this animation's scene.
    ///     input: Keyframe times, in seconds, in ascending order.
    ///     output: The translation at each keyframe — or, with cubic-spline interpolation, an
    ///         in-tangent, value and out-tangent for each.
    ///     interpolation: How values between keyframes are interpolated; linear when omitted.
    ///
    /// Raises:
    ///     ValueError: If `node` is from another scene, or `output` has the wrong length.
    #[pyo3(signature = (node, input, output, *, interpolation = None))]
    pub fn add_translation(
        slf: Bound<Self>,
        node: Py<Node>,
        input: Vec<f64>,
        output: Vec<Vec3>,
        interpolation: Option<Interpolation>,
    ) -> PyResult<()> {
        let channel = Channel::new(input, output, interpolation)?;
        Self::with_node_trs(slf, node, |trs| trs.translation = Some(channel))
    }

    /// Set `node`'s rotation track, replacing any existing one.
    ///
    /// Args:
    ///     node: The node to animate; it must belong to this animation's scene.
    ///     input: Keyframe times, in seconds, in ascending order.
    ///     output: The rotation at each keyframe — or, with cubic-spline interpolation, an
    ///         in-tangent, value and out-tangent for each.
    ///     interpolation: How values between keyframes are interpolated; linear when omitted.
    ///
    /// Raises:
    ///     ValueError: If `node` is from another scene, or `output` has the wrong length.
    #[pyo3(signature = (node, input, output, *, interpolation = None))]
    pub fn add_rotation(
        slf: Bound<Self>,
        node: Py<Node>,
        input: Vec<f64>,
        output: Vec<Quat>,
        interpolation: Option<Interpolation>,
    ) -> PyResult<()> {
        let channel = Channel::new(input, output, interpolation)?;
        Self::with_node_trs(slf, node, |trs| trs.rotation = Some(channel))
    }

    /// Set `node`'s scale track, replacing any existing one.
    ///
    /// Args:
    ///     node: The node to animate; it must belong to this animation's scene.
    ///     input: Keyframe times, in seconds, in ascending order.
    ///     output: The per-axis scale at each keyframe — or, with cubic-spline interpolation,
    ///         an in-tangent, value and out-tangent for each.
    ///     interpolation: How values between keyframes are interpolated; linear when omitted.
    ///
    /// Raises:
    ///     ValueError: If `node` is from another scene, or `output` has the wrong length.
    #[pyo3(signature = (node, input, output, *, interpolation = None))]
    pub fn add_scale(
        slf: Bound<Self>,
        node: Py<Node>,
        input: Vec<f64>,
        output: Vec<Vec3>,
        interpolation: Option<Interpolation>,
    ) -> PyResult<()> {
        let channel = Channel::new(input, output, interpolation)?;
        Self::with_node_trs(slf, node, |trs| trs.scale = Some(channel))
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.nodes.keys().try_for_each(|node| visit.call(&node.0))?;
        visit.call(&self.scene)
    }
}

/// How an animation track interpolates between keyframes.
///
/// `Linear` blends between neighboring keyframes (spherically, for rotations); `Step` holds
/// each keyframe until the next; `CubicSpline` uses Hermite splines and needs three output
/// values per keyframe.
#[pyclass(from_py_object, frozen)]
#[derive(Copy, Clone)]
pub enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}

#[derive(Default)]
pub struct Trs {
    pub translation: Option<Channel<Vec3>>,
    pub rotation: Option<Channel<Quat>>,
    pub scale: Option<Channel<Vec3>>,
}

pub struct Channel<T> {
    pub input: Vec<f64>,
    pub output: Vec<T>,
    pub interpolation: Option<Interpolation>,
}

impl<T> Channel<T> {
    fn new(input: Vec<f64>, output: Vec<T>, interpolation: Option<Interpolation>) -> PyResult<Self> {
        if !input.iter().all(|t| t.is_finite()) {
            return Err(PyValueError::new_err("keyframe times must all be finite"));
        }
        match interpolation {
            Some(Interpolation::CubicSpline) => {
                if input.len() * 3 != output.len() {
                    return Err(PyValueError::new_err(
                        "outputs' array must be 3 times longer as the inputs' for cubic spline interpolation",
                    ));
                }
            }
            _ => {
                if input.len() != output.len() {
                    return Err(PyValueError::new_err(
                        "outputs' array must have the same length as the inputs'",
                    ));
                }
            }
        }
        Ok(Channel {
            input,
            output,
            interpolation,
        })
    }
}

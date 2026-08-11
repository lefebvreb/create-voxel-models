use pyo3::{Bound, Py, pyclass, pymethods};

use crate::math::{Quat, Vec3};
use crate::scene::{Node, Scene};

#[pyclass]
pub struct Anim {
    #[pyo3(get)]
    pub name: String,
    pub nodes: Vec<NodeAnim>,
    #[pyo3(get)]
    pub scene: Py<Scene>,
}

#[pymethods]
impl Anim {
    #[pyo3(signature = (node, input, output, *, interpolation = Interpolation::Linear))]
    pub fn add_translation(
        slf: Bound<Self>,
        node: Py<Node>,
        input: Vec<f64>,
        output: Vec<Vec3>,
        interpolation: Interpolation,
    ) {
        todo!()
    }
}

#[pyclass(frozen, from_py_object)]
#[derive(Copy, Clone)]
pub enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}

pub struct NodeAnim {
    pub node: usize,
    pub translation: Option<Frames<Vec3>>,
    pub rotation: Option<Frames<Quat>>,
    pub scale: Option<Frames<Vec3>>,
}

pub struct Frames<T> {
    pub interpolation: Interpolation,
    pub times: Vec<f64>,
    pub values: Vec<T>,
}

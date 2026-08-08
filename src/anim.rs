use pyo3::pyclass;

use crate::math::{Quat, Vec3};

#[pyclass]
pub struct Anim {
    pub name: String,
    pub nodes: Vec<NodeAnim>,
}

#[pyclass]
pub struct NodeAnim {
    pub node: usize,
    pub translation: Option<Vec<Keyframe<Vec3>>>,
    pub rotation: Option<Vec<Keyframe<Quat>>>,
    pub scale: Option<Vec<Keyframe<Vec3>>>,
}

pub struct Keyframe<T> {
    pub time: f64,
    pub value: T,
}

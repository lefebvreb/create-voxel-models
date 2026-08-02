use pyo3::{pyclass, pymethods};

use crate::math::{Quaternion, Vec3};
use crate::model::Model;
use crate::palette::Palette;

#[pyclass]
pub struct Scene {
    palette: Palette,
    nodes: Vec<Node>,
    animations: Vec<Animation>,
}

#[pymethods]
impl Scene {
    #[new]
    fn new() -> Self {
        Self {
            palette: Palette::new(),
            nodes: vec![],
            animations: vec![],
        }
    }
}

pub struct Node {
    model: Option<Box<Model>>,
}

pub struct Animation {
    name: Box<str>,
    nodes: Vec<NodeAnimation>,
}

pub struct NodeAnimation {
    node: usize,
    pub translation: Option<Vec<Keyframe<Vec3>>>,
    pub rotation: Option<Vec<Keyframe<Quaternion>>>,
    pub scale: Option<Vec<Keyframe<Vec3>>>,
}

pub struct Keyframe<T> {
    pub time: f32,
    pub value: T,
}

use pyo3::{Py, pyclass, pymethods};

use crate::math::{Quaternion, Vec3};
use crate::model::Model;
use crate::palette::Palette;

#[pyclass]
pub struct Scene {
    palette: Py<Palette>,
    nodes: Vec<Node>,
    models: Vec<Model>,
    animations: Vec<Animation>,
}

#[pymethods]
impl Scene {
    #[new]
    fn new() -> Self {
        // Self {
        //     palette: Palette::new(),
        //     nodes: vec![],
        //     animations: vec![],
        // }
        todo!()
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

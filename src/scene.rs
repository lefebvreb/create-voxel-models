use pyo3::{Py, pyclass, pymethods};

use crate::anim::Anim;
use crate::model::Model;
use crate::palette::Palette;

#[pyclass]
pub struct Scene {
    palette: Py<Palette>,
    nodes: Vec<Py<Node>>,
    models: Vec<Model>,
    animations: Vec<Anim>,
}

#[pymethods]
impl Scene {
    #[new]
    fn new(palette: Py<Palette>) -> Self {
        Self {
            palette,
            nodes: Vec::new(),
            models: Vec::new(),
            animations: Vec::new(),
        }
    }
}

pub struct Node {
    model: Option<Box<Model>>,
}

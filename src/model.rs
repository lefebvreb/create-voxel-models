use pyo3::{Py, pyclass};

use crate::palette::Palette;
use crate::scene::{Node, Scene};

#[pyclass]
pub struct Model {
    pub name: String,
    pub data: Box<[u8]>,
    pub dims: (u8, u8, u8),
    pub palette: Py<Palette>,
    pub parent: Py<Node>,
    pub scene: Py<Scene>,
}

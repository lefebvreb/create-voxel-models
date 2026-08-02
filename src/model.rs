use pyo3::{Py, pyclass};

use crate::palette::Palette;
use crate::scene::Node;

#[pyclass]
pub(crate) struct Model {
    data: Box<[u8]>,
    dims: (u8, u8, u8),
    palette: Py<Palette>,
    parent: Py<Node>,
}

impl Model {
    pub(crate) fn new() -> Model {
        todo!()
    }
}

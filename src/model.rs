use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, pyclass, pymethods};

use crate::palette::{Color, Palette};
use crate::render::{CameraAngle, RenderOutput};
use crate::scene::{Node, Scene};
use crate::tools::render;
use crate::utils::Int3;

#[pyclass]
pub struct Model {
    #[pyo3(get)]
    pub dimensions: Int3,
    pub data: Box<[u8]>,
    #[pyo3(get)]
    pub palette: Py<Palette>,
}

impl Model {
    fn pos_to_index(&self, pos: Int3) -> PyResult<usize> {
        let (x, y, z) = pos;
        let (dx, dy, dz) = self.dimensions;
        if x >= dx || y >= dy || z >= dz {
            return Err(PyValueError::new_err("coordinates are out of bounds of this model"));
        }
        Ok(x + y * dx + z * dx * dy)
    }

    fn check_color_get_id(&self, color: Option<Bound<Color>>) -> PyResult<u8> {
        let Some(color) = color else {
            return Ok(0);
        };
        if !color.get().palette.is(&self.palette) {
            return Err(PyValueError::new_err("color does not belong to this model's palette"));
        }
        Ok(color.get().index + 1)
    }
}

#[pymethods]
impl Model {
    #[new]
    pub fn __new__(dimensions: Int3, palette: Py<Palette>) -> PyResult<Self> {
        let (dx, dy, dz) = dimensions;
        if dx > 256 || dy > 256 || dz > 256 {
            return Err(PyValueError::new_err(
                "dimension along any given axis cannot surpass 256",
            ));
        }
        Ok(Self {
            dimensions,
            data: vec![0; dx * dy * dz].into_boxed_slice(),
            palette,
        })
    }

    #[pyo3(signature = (position, color = None))]
    pub fn put(&mut self, position: Int3, color: Option<Bound<Color>>) -> PyResult<()> {
        let index = self.pos_to_index(position)?;
        let color = self.check_color_get_id(color)?;
        self.data[index] = color;
        Ok(())
    }

    #[pyo3(signature = (angles, *, background = None, output_dir = None))]
    pub fn render(
        slf: Bound<Self>,
        angles: Vec<CameraAngle>,
        background: Option<(u8, u8, u8)>,
        output_dir: Option<PathBuf>,
    ) -> PyResult<RenderOutput> {
        let py = slf.py();
        let scene = Py::new(py, Scene::default())?.into_bound(py);
        let node = Scene::create_root_node(scene.clone(), "root".to_string(), None)?;
        Node::add_model(node.bind(py).clone(), "model".to_string(), slf.unbind(), None)?;
        render(scene, angles, vec![], None, None, None, background, output_dir)
    }
}

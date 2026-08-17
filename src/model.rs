use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, Python, pyclass, pymethods};

use crate::palette::{Material, Palette};
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
    fn check_contains(&self, a: Int3) -> PyResult<()> {
        let ((ax, ay, az), (dx, dy, dz)) = (a, self.dimensions);
        if ax >= dx || ay >= dy || az >= dz {
            return Err(PyValueError::new_err(format!(
                "coordinates ({ax}, {ay}, {az}) are out of bounds of this model"
            )));
        }
        Ok(())
    }

    fn index(&self, pos: Int3) -> usize {
        let ((x, y, z), (dx, dy, _)) = (pos, self.dimensions);
        x + y * dx + z * dx * dy
    }

    fn check_color_get_id(&self, color: Option<&Material>) -> PyResult<u8> {
        let Some(color) = color else {
            return Ok(0);
        };
        if !color.palette.is(&self.palette) {
            return Err(PyValueError::new_err("color does not belong to this model's palette"));
        }
        Ok(color.index + 1)
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

    pub fn copy(&self, py: Python) -> Self {
        Self {
            dimensions: self.dimensions,
            data: self.data.clone(),
            palette: self.palette.clone_ref(py),
        }
    }

    #[pyo3(signature = (color, a))]
    pub fn put(&mut self, color: Option<&Material>, a: Int3) -> PyResult<()> {
        self.check_contains(a)?;
        let color = self.check_color_get_id(color)?;
        self.data[self.index(a)] = color;
        Ok(())
    }

    #[pyo3(signature = (color, a, b))]
    pub fn aabb(&mut self, color: Option<&Material>, a: Int3, b: Int3) -> PyResult<()> {
        self.check_contains(a)?;
        self.check_contains(b)?;
        let color = self.check_color_get_id(color)?;
        let ((ax, ay, az), (bx, by, bz)) = (a, b);
        for z in az.min(bz)..az.max(bz) {
            for y in ay.min(by)..ay.max(by) {
                for x in ax.min(bx)..ax.max(bx) {
                    self.data[self.index((x, y, z))] = color;
                }
            }
        }
        Ok(())
    }

    #[pyo3(signature = (angles))]
    pub fn render(slf: Bound<Self>, angles: Vec<CameraAngle>) -> PyResult<RenderOutput> {
        let scene = Py::new(slf.py(), Scene::default())?.into_bound(slf.py());
        let node = Scene::create_root_node(scene.clone(), "root".to_string(), None)?;
        Node::add_model(node.bind(slf.py()).clone(), "model".to_string(), slf.unbind(), None)?;
        render(scene, angles, vec![], None, None, None)
    }
}

use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, pyclass, pymethods};

use crate::math::Pos;
use crate::palette::{Color, Palette};

#[pyclass]
pub struct Model {
    pub name: String,
    pub dims: Pos,
    pub data: Box<[u8]>,
    pub palette: Py<Palette>,
}

impl Model {
    fn pos_to_index(&self, pos: Pos) -> PyResult<usize> {
        let (x, y, z) = pos;
        let (dx, dy, dz) = self.dims;
        if x >= dx || y >= dy || z >= dz {
            return Err(PyValueError::new_err("coordinates are out of bounds of this model"));
        }
        Ok(x + y * dx + z * dx * dy)
    }

    fn check_color_get_id(&self, color: &Option<Bound<Color>>) -> PyResult<u8> {
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
    fn new(name: String, dims: Pos, palette: Py<Palette>) -> PyResult<Self> {
        let (dx, dy, dz) = dims;
        if dx > 256 || dy > 256 || dz > 256 {
            return Err(PyValueError::new_err(
                "dimension along any given axis cannot surpass 256",
            ));
        }
        Ok(Self {
            name,
            dims,
            data: vec![0; dx * dy * dz].into_boxed_slice(),
            palette,
        })
    }

    #[pyo3(signature = (pos, color = None))]
    fn put(&mut self, pos: Pos, color: Option<Bound<Color>>) -> PyResult<()> {
        let index = self.pos_to_index(pos)?;
        let color = self.check_color_get_id(&color)?;
        self.data[index] = color;
        Ok(())
    }
}

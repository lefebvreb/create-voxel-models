use pyo3::exceptions::PyIndexError;
use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

#[pyclass(frozen)]
pub(crate) struct Color {
    index: usize,
    palette: Py<Palette>,
    #[pyo3(get)]
    rgb: (u8, u8, u8),
    #[pyo3(get)]
    roughness: f32,
    #[pyo3(get)]
    metallic: f32,
    #[pyo3(get)]
    ior: f32,
    #[pyo3(get)]
    transmission: f32,
    #[pyo3(get)]
    emissive: f32,
}

#[pymethods]
impl Color {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.palette)
    }
}

#[pyclass]
pub(crate) struct Palette {
    colors: Vec<Py<Color>>,
}

#[pymethods]
impl Palette {
    #[new]
    fn __new__() -> Self {
        Self { colors: Vec::new() }
    }

    #[pyo3(signature = (rgb, *, roughness = 1.0, metallic = 0.0, ior = 1.5, transmission = 0.0, emissive = 0.0))]
    fn add_color(
        slf: Bound<Self>,
        rgb: (u8, u8, u8),
        roughness: f32,
        metallic: f32,
        ior: f32,
        transmission: f32,
        emissive: f32,
    ) -> PyResult<Py<Color>> {
        let mut obj = slf.borrow_mut();
        let index = obj.colors.len();
        if index == 255 {
            return Err(PyIndexError::new_err(
                "palette already contains 255 colors, which is the maximum permitted",
            ));
        }
        let color = Py::new(
            slf.py(),
            Color {
                index,
                rgb,
                roughness,
                metallic,
                ior,
                transmission,
                emissive,
                palette: slf.as_unbound().clone_ref(slf.py()),
            },
        )?;
        obj.colors.push(color.clone_ref(slf.py()));
        Ok(color)
    }

    fn __len__(&self) -> usize {
        self.colors.len()
    }

    fn __getitem__(slf: Bound<Self>, index: usize) -> PyResult<Py<Color>> {
        let obj = slf.borrow();
        let color = obj
            .colors
            .get(index)
            .ok_or_else(|| PyIndexError::new_err("color index out of bounds"))?;
        Ok(color.clone_ref(slf.py()))
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.colors.iter().try_for_each(|color| visit.call(color))
    }

    fn __clear__(&mut self) {
        self.colors.clear();
    }
}

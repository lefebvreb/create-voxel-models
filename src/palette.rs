use pyo3::exceptions::PyIndexError;
use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

#[pyclass(frozen)]
pub struct Color {
    pub index: u8,
    #[pyo3(get)]
    pub rgb: (u8, u8, u8),
    #[pyo3(get)]
    pub roughness: f64,
    #[pyo3(get)]
    pub metallic: f64,
    #[pyo3(get)]
    pub ior: f64,
    #[pyo3(get)]
    pub transmission: f64,
    #[pyo3(get)]
    pub emissive: f64,
    #[pyo3(get)]
    pub palette: Py<Palette>,
}

#[pymethods]
impl Color {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.palette)
    }
}

#[pyclass]
pub struct Palette {
    pub colors: Vec<Py<Color>>,
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
        roughness: f64,
        metallic: f64,
        ior: f64,
        transmission: f64,
        emissive: f64,
    ) -> PyResult<Py<Color>> {
        let mut slf_brw = slf.borrow_mut();
        let index = slf_brw.colors.len();
        if index >= 255 {
            return Err(PyIndexError::new_err(
                "palette already contains 255 colors, which is the maximum permitted",
            ));
        }
        let color = Py::new(
            slf.py(),
            Color {
                rgb,
                roughness,
                metallic,
                ior,
                transmission,
                emissive,
                index: u8::try_from(index).expect("color index should fit in a byte"),
                palette: slf.clone().unbind(),
            },
        )?;
        slf_brw.colors.push(color.clone_ref(slf.py()));
        Ok(color)
    }

    fn __len__(&self) -> usize {
        self.colors.len()
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.colors.iter().try_for_each(|color| visit.call(color))
    }

    fn __clear__(&mut self) {
        self.colors.clear();
    }
}

use pyo3::exceptions::PyValueError;
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
    pub volume: Option<Volume>,
    #[pyo3(get)]
    pub palette: Py<Palette>,
}

#[pymethods]
impl Color {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.palette)
    }
}

#[pyclass(frozen, from_py_object)]
#[derive(Copy, Clone)]
pub struct Volume {
    #[pyo3(get)]
    pub color: (u8, u8, u8),
    #[pyo3(get)]
    pub distance: f64,
    #[pyo3(get)]
    pub thickness: f64,
}

#[pymethods]
impl Volume {
    #[new]
    pub fn __new__(color: (u8, u8, u8), distance: f64, thickness: f64) -> PyResult<Self> {
        if distance <= 0.0 {
            return Err(PyValueError::new_err("distance must be greater than 0.0"));
        }
        if thickness <= 0.0 {
            return Err(PyValueError::new_err("thickness must be greater than 0.0"));
        }
        Ok(Self {
            color,
            distance,
            thickness,
        })
    }
}

#[pyclass]
#[derive(Default)]
pub struct Palette {
    pub colors: Vec<Py<Color>>,
}

#[pymethods]
impl Palette {
    #[new]
    pub fn __new__() -> Self {
        Self::default()
    }

    #[pyo3(signature = (rgb, *, roughness = 1.0, metallic = 0.0, ior = 1.5, transmission = 0.0, emissive = 0.0, volume = None))]
    #[allow(clippy::too_many_arguments)]
    pub fn add_color(
        slf: Bound<Self>,
        rgb: (u8, u8, u8),
        roughness: f64,
        metallic: f64,
        ior: f64,
        transmission: f64,
        emissive: f64,
        volume: Option<Volume>,
    ) -> PyResult<Py<Color>> {
        if !(0.0..=1.0).contains(&roughness) {
            return Err(PyValueError::new_err("roughness must be between 0.0 and 1.0"));
        }
        if !(0.0..=1.0).contains(&metallic) {
            return Err(PyValueError::new_err("metallic must be between 0.0 and 1.0"));
        }
        if !(0.0..=1.0).contains(&transmission) {
            return Err(PyValueError::new_err("transmission must be between 0.0 and 1.0"));
        }
        if !(ior == 0.0 || ior >= 1.0) {
            return Err(PyValueError::new_err(
                "ior must be 0.0, or greater than or equal to 1.0",
            ));
        }
        if emissive < 0.0 {
            return Err(PyValueError::new_err("emissive must be greater than or equal to 0.0"));
        }
        let mut slf_brw = slf.borrow_mut();
        let index = slf_brw.colors.len();
        if index >= 255 {
            return Err(PyValueError::new_err(
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
                volume,
                index: u8::try_from(index).expect("color index should fit in a byte"),
                palette: slf.clone().unbind(),
            },
        )?;
        slf_brw.colors.push(color.clone_ref(slf.py()));
        Ok(color)
    }

    pub fn __len__(&self) -> usize {
        self.colors.len()
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.colors.iter().try_for_each(|color| visit.call(color))
    }

    fn __clear__(&mut self) {
        self.colors.clear();
    }
}

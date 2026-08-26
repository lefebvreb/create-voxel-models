use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::{PyResult, pyclass, pymethods};

#[pyclass(get_all, from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct CameraAngle {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: Option<f64>,
}

#[pymethods]
impl CameraAngle {
    #[new]
    #[pyo3(signature = (yaw, pitch, *, zoom = None))]
    fn new(yaw: f64, pitch: f64, zoom: Option<f64>) -> PyResult<Self> {
        if !yaw.is_finite() {
            return Err(PyValueError::new_err("yaw must be finite"));
        }
        if !pitch.is_finite() {
            return Err(PyValueError::new_err("pitch must be finite"));
        }
        if let Some(zoom) = zoom
            && !zoom.is_finite()
        {
            return Err(PyValueError::new_err("zoom must be finite"));
        }
        Ok(Self { yaw, pitch, zoom })
    }
}

#[pyclass(get_all, frozen)]
pub struct RenderOutput {
    pub files: Vec<PathBuf>,
}

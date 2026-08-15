use std::path::PathBuf;

use pyo3::{pyclass, pymethods};

#[pyclass(frozen, from_py_object, get_all)]
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
    fn __new__(yaw: f64, pitch: f64, zoom: Option<f64>) -> Self {
        Self { yaw, pitch, zoom }
    }
}

#[pyclass(frozen, get_all)]
pub struct RenderOutput {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
}

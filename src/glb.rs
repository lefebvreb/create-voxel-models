use std::path::PathBuf;

use pyo3::{Bound, PyResult};

use crate::scene::Scene;

pub fn export_glb(slf: Bound<Scene>, path: PathBuf) -> PyResult<()> {
    let _ = (slf, path);
    todo!()
}

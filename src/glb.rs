use pyo3::{Bound, PyResult};

use crate::scene::Scene;

pub fn export_glb(scene: Bound<Scene>) -> PyResult<Vec<u8>> {
    let _ = scene;
    todo!()
}

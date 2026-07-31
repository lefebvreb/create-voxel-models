use pyo3::pymodule;

mod math;
mod model;
mod palette;
mod scene;

#[pymodule]
mod voxels {
    #[pymodule_export]
    use crate::scene::Scene;
}

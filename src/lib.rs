use pyo3::pymodule;

mod math;
mod model;
mod palette;
mod scene;

#[pymodule]
mod voxels {
    use pyo3::pyfunction;

    #[pyfunction]
    pub fn sum(a: i32, b: i32) -> i32 {
        a + b
    }
}

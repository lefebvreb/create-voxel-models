use pyo3::pymodule;

mod anim;
mod math;
mod model;
mod palette;
mod scene;

#[pymodule]
mod voxels {
    #[pymodule_export]
    use crate::palette::{Color, Palette};
    #[pymodule_export]
    use crate::scene::{Node, Scene};
}

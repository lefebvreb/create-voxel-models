use pyo3::pymodule;

mod anim;
mod math;
mod model;
mod palette;
mod scene;
mod utils;

#[pymodule]
mod voxels {
    #[pymodule_export]
    use crate::anim::{Anim, Interpolation};
    #[pymodule_export]
    use crate::math::{Quat, Vec3};
    #[pymodule_export]
    use crate::model::Model;
    #[pymodule_export]
    use crate::palette::{Color, Palette};
    #[pymodule_export]
    use crate::scene::{Node, Scene};
}

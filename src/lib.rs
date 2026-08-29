use pyo3::pymodule;

mod anim;
mod math;
mod model;
mod palette;
mod preview;
mod scene;
mod tools;
mod utils;

#[pymodule]
mod _voxels {
    #[pymodule_export]
    use crate::anim::{Anim, Interpolation};
    #[pymodule_export]
    use crate::math::{Quat, Vec3};
    #[pymodule_export]
    use crate::model::{Dimensions, Model, Pivot};
    #[pymodule_export]
    use crate::palette::{Color, Material, Palette, Volume};
    #[pymodule_export]
    use crate::preview::_preview;
    #[pymodule_export]
    use crate::scene::{Mesh, Node, Scene};
}

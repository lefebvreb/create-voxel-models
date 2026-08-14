use pyo3::pymodule;

mod anim;
mod glb;
mod gltf;
mod math;
mod meshing;
mod model;
mod palette;
mod render;
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
    use crate::render::{CameraAngle, RenderOutput};
    #[pymodule_export]
    use crate::scene::{Mesh, Node, Scene};
}

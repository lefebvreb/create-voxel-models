use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, Python, pyclass, pymethods};

use crate::palette::{Material, MaterialCode, Palette};
use crate::render::{CameraAngle, RenderOutput};
use crate::scene::{Node, Scene};
use crate::tools::render;
use crate::utils::Int3;

#[pyclass(get_all, from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct Dimensions {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

#[pymethods]
impl Dimensions {
    #[new]
    fn __new__(x: usize, y: usize, z: usize) -> PyResult<Self> {
        if !(1..=256).contains(&x) {
            return Err(PyValueError::new_err("x dimension must be between 1 and 256 inclusive"));
        }
        if !(1..=256).contains(&y) {
            return Err(PyValueError::new_err("y dimension must be between 1 and 256 inclusive"));
        }
        if !(1..=256).contains(&z) {
            return Err(PyValueError::new_err("z dimension must be between 1 and 256 inclusive"));
        }
        Ok(Self { x, y, z })
    }

    fn contains(&self, a: Int3) -> bool {
        let (x, y, z) = a;
        x < self.x && y < self.y && z < self.z
    }
}

#[pyclass]
pub struct Model {
    #[pyo3(get)]
    pub dimensions: Dimensions,
    pub zstride: usize,
    pub data: Box<[Option<MaterialCode>]>,
    #[pyo3(get)]
    pub palette: Py<Palette>,
}

impl Model {
    fn check_contains(&self, a: Int3) -> PyResult<()> {
        if !self.dimensions.contains(a) {
            return Err(PyValueError::new_err(format!(
                "coordinates ({a:?}) are out of bounds of this model"
            )));
        }
        Ok(())
    }

    fn index(&self, pos: Int3) -> usize {
        let (x, y, z) = pos;
        x + y * self.dimensions.x + z * self.zstride
    }

    fn get_material_code(&self, color: Option<&Material>) -> PyResult<Option<MaterialCode>> {
        let Some(color) = color else {
            return Ok(None);
        };
        if !color.palette.is(&self.palette) {
            return Err(PyValueError::new_err("color does not belong to this model's palette"));
        }
        Ok(Some(color.code))
    }
}

#[pymethods]
impl Model {
    #[new]
    pub fn __new__(dimensions: Dimensions, palette: Py<Palette>) -> Self {
        Self {
            dimensions,
            zstride: dimensions.x * dimensions.y,
            data: vec![None; dimensions.x * dimensions.y * dimensions.z].into_boxed_slice(),
            palette,
        }
    }

    pub fn copy(&self, py: Python) -> Self {
        Self {
            dimensions: self.dimensions,
            zstride: self.zstride,
            data: self.data.clone(),
            palette: self.palette.clone_ref(py),
        }
    }

    pub fn put(&mut self, material: Option<&Material>, a: Int3) -> PyResult<()> {
        self.check_contains(a)?;
        let code = self.get_material_code(material)?;
        self.data[self.index(a)] = code;
        Ok(())
    }

    // #[pyo3(signature = (color, a, b))]
    // pub fn aabb(&mut self, color: Option<&Material>, a: Int3, b: Int3) -> PyResult<()> {
    //     self.check_contains(a)?;
    //     self.check_contains(b)?;
    //     let color = self.check_color_get_id(color)?;
    //     let ((ax, ay, az), (bx, by, bz)) = (a, b);
    //     for z in az.min(bz)..az.max(bz) {
    //         for y in ay.min(by)..ay.max(by) {
    //             for x in ax.min(bx)..ax.max(bx) {
    //                 self.data[self.index((x, y, z))] = color;
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    pub fn render(slf: Bound<Self>, angles: Vec<CameraAngle>) -> PyResult<RenderOutput> {
        let scene = Py::new(slf.py(), Scene::default())?.into_bound(slf.py());
        let node = Scene::create_root_node(scene.clone(), "root".to_string(), None)?;
        Node::add_model(node.bind(slf.py()).clone(), "model".to_string(), slf.unbind(), None)?;
        render(scene, angles, vec![], None, None, None)
    }
}

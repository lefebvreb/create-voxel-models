use either::Either;
use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, Python, pyclass, pymethods};

use crate::math::Vec3;
use crate::palette::{Material, MaterialCode, Palette};
use crate::render::{CameraAngle, RenderOutput};
use crate::scene::{Node, Scene};
use crate::tools::render;
use crate::utils::{Int3, int3};

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

    fn as_vec(&self) -> Vec3 {
        int3::into_vec3((self.x, self.y, self.z))
    }
}

impl Dimensions {
    fn last_index(&self) -> Int3 {
        (self.x - 1, self.y - 1, self.z - 1)
    }
}

#[pyclass(from_py_object, frozen)]
#[derive(Copy, Clone)]
pub enum Pivot {
    Corner,
    Center,
    BottomCenter,
}

#[pyclass]
pub struct Model {
    #[pyo3(get)]
    pub dims: Dimensions,
    pub zstride: usize,
    pub data: Box<[Option<MaterialCode>]>,
    #[pyo3(get)]
    pub pivot: Either<Pivot, Vec3>,
    #[pyo3(get)]
    pub palette: Py<Palette>,
}

impl Model {
    fn check_contains(&self, a: Int3) -> PyResult<()> {
        if !self.dims.contains(a) {
            return Err(PyValueError::new_err(format!(
                "coordinates ({a:?}) are out of bounds of this model"
            )));
        }
        Ok(())
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

    fn index(&self, pos: Int3) -> usize {
        let (x, y, z) = pos;
        x + y * self.dims.x + z * self.zstride
    }

    pub fn pivot_offset(&self) -> Vec3 {
        match self.pivot {
            Either::Left(Pivot::Corner) => Vec3::ZERO,
            Either::Left(Pivot::Center) => self.dims.as_vec().__mul__(0.5),
            Either::Left(Pivot::BottomCenter) => Vec3::__new__(self.dims.x as f64 * 0.5, 0.0, self.dims.z as f64 * 0.5),
            Either::Right(v) => v,
        }
    }

    fn get(&self, pos: Int3) -> Option<MaterialCode> {
        self.data[self.index(pos)]
    }

    fn set(&mut self, pos: Int3, code: Option<MaterialCode>) {
        self.data[self.index(pos)] = code;
    }

    fn fill_ellipsoid(&mut self, material: Option<&Material>, c: Int3, r: Int3) -> PyResult<()> {
        self.check_contains(c)?;
        let code = self.get_material_code(material)?;
        let (rx, ry, rz) = r;
        if rx == 0 || ry == 0 || rz == 0 {
            return Err(PyValueError::new_err("radii must be at least 1"));
        }
        let center = int3::into_vec3(c);
        let lo = int3::saturating_sub(c, r);
        let hi = int3::min(int3::add(c, r), self.dims.last_index());
        for pos in box_positions(lo, hi) {
            let delta = int3::into_vec3(pos).__sub__(center);
            let nx = delta.x() / rx as f64;
            let ny = delta.y() / ry as f64;
            let nz = delta.z() / rz as f64;
            if nx * nx + ny * ny + nz * nz <= 1.0 {
                self.set(pos, code);
            }
        }
        Ok(())
    }

    fn flip_axis(&mut self, block_len: usize, axis_count: usize) {
        let chunk_len = block_len * axis_count;
        for chunk_start in (0..self.data.len()).step_by(chunk_len) {
            for i in 0..axis_count / 2 {
                let a = chunk_start + i * block_len;
                let b = chunk_start + (axis_count - 1 - i) * block_len;
                for offset in 0..block_len {
                    self.data.swap(a + offset, b + offset);
                }
            }
        }
    }
}

#[pymethods]
impl Model {
    #[new]
    pub fn __new__(dims: Dimensions, palette: Py<Palette>, pivot: Either<Pivot, Vec3>) -> Self {
        Self {
            dims,
            zstride: dims.x * dims.y,
            data: vec![None; dims.x * dims.y * dims.z].into_boxed_slice(),
            pivot,
            palette,
        }
    }

    pub fn copy(&self, py: Python) -> Self {
        Self {
            dims: self.dims,
            zstride: self.zstride,
            data: self.data.clone(),
            pivot: self.pivot,
            palette: self.palette.clone_ref(py),
        }
    }

    pub fn flip_x(slf: Bound<Self>) -> Bound<Self> {
        let mut slf_brw = slf.borrow_mut();
        let dims = slf_brw.dims;
        slf_brw.flip_axis(1, dims.x);
        slf
    }

    pub fn flip_y(slf: Bound<Self>) -> Bound<Self> {
        let mut slf_brw = slf.borrow_mut();
        let dims = slf_brw.dims;
        slf_brw.flip_axis(dims.x, dims.y);
        slf
    }

    pub fn flip_z(slf: Bound<Self>) -> Bound<Self> {
        let mut slf_brw = slf.borrow_mut();
        let (zstride, z) = (slf_brw.zstride, slf_brw.dims.z);
        slf_brw.flip_axis(zstride, z);
        slf
    }

    pub fn put(&mut self, material: Option<&Material>, pos: Int3) -> PyResult<()> {
        self.check_contains(pos)?;
        let code = self.get_material_code(material)?;
        self.set(pos, code);
        Ok(())
    }

    pub fn aabb(&mut self, material: Option<&Material>, a: Int3, b: Int3) -> PyResult<()> {
        self.check_contains(a)?;
        self.check_contains(b)?;
        let code = self.get_material_code(material)?;
        for pos in box_positions(a, b) {
            self.set(pos, code);
        }
        Ok(())
    }

    pub fn sphere(&mut self, material: Option<&Material>, c: Int3, r: usize) -> PyResult<()> {
        self.fill_ellipsoid(material, c, (r, r, r))
    }

    pub fn spheroid(&mut self, material: Option<&Material>, c: Int3, r_eq: usize, r_polar: usize) -> PyResult<()> {
        self.fill_ellipsoid(material, c, (r_eq, r_polar, r_eq))
    }

    pub fn ellipsoid(&mut self, material: Option<&Material>, c: Int3, rx: usize, ry: usize, rz: usize) -> PyResult<()> {
        self.fill_ellipsoid(material, c, (rx, ry, rz))
    }

    pub fn include(&mut self, other: &Model, offset: Int3) -> PyResult<()> {
        if !other.palette.is(&self.palette) {
            return Err(PyValueError::new_err("model does not belong to this model's palette"));
        }
        for local in box_positions((0, 0, 0), other.dims.last_index()) {
            let Some(code) = other.get(local) else {
                continue;
            };
            let pos = int3::add(offset, local);
            if self.dims.contains(pos) {
                self.set(pos, Some(code));
            }
        }
        Ok(())
    }

    pub fn render(slf: Bound<Self>, angles: Vec<CameraAngle>) -> PyResult<RenderOutput> {
        let scene = Py::new(slf.py(), Scene::default())?.into_bound(slf.py());
        let node = Scene::create_root_node(scene.clone(), "root".to_string(), None)?;
        Node::add_model(node.bind(slf.py()).clone(), "model".to_string(), slf.unbind(), None)?;
        render(scene, angles, vec![], None, None, None)
    }
}

fn box_positions(a: Int3, b: Int3) -> impl Iterator<Item = Int3> {
    let (xmin, ymin, zmin) = int3::min(a, b);
    let (xmax, ymax, zmax) = int3::max(a, b);
    (zmin..=zmax).flat_map(move |z| (ymin..=ymax).flat_map(move |y| (xmin..=xmax).map(move |x| (x, y, z))))
}

use either::Either;
use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, Python, pyclass, pymethods};

use crate::math::Vec3;
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
        let (dx, dy, dz) = (self.dims.x as f64, self.dims.y as f64, self.dims.z as f64);
        match self.pivot {
            Either::Left(Pivot::Corner) => Vec3::ZERO,
            Either::Left(Pivot::Center) => Vec3::__new__(dx / 2.0, dy / 2.0, dz / 2.0),
            Either::Left(Pivot::BottomCenter) => Vec3::__new__(dx / 2.0, 0.0, dz / 2.0),
            Either::Right(v) => v,
        }
    }

    fn get(&self, pos: Int3) -> Option<MaterialCode> {
        self.data[self.index(pos)]
    }

    fn set(&mut self, pos: Int3, code: Option<MaterialCode>) {
        self.data[self.index(pos)] = code;
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

    #[pyo3(signature = (material, c, r1, r2 = None, r3 = None))]
    pub fn spheroid(
        &mut self,
        material: Option<&Material>,
        c: Int3,
        r1: usize,
        r2: Option<usize>,
        r3: Option<usize>,
    ) -> PyResult<()> {
        self.check_contains(c)?;
        if r1 == 0 || r2 == Some(0) || r3 == Some(0) {
            return Err(PyValueError::new_err("radii must be at least 1"));
        }
        let r = match (r2, r3) {
            (None, None) => (r1, r1, r1),
            (Some(r2), None) => (r1, r1, r2),
            (Some(r2), Some(r3)) => (r1, r2, r3),
            (None, Some(_)) => {
                return Err(PyValueError::new_err("r3 requires r2 to also be specified"));
            }
        };
        let code = self.get_material_code(material)?;
        let (cx, cy, cz) = c;
        let (rx, ry, rz) = r;
        let lo = (cx.saturating_sub(rx), cy.saturating_sub(ry), cz.saturating_sub(rz));
        let hi = (
            (cx + rx).min(self.dims.x - 1),
            (cy + ry).min(self.dims.y - 1),
            (cz + rz).min(self.dims.z - 1),
        );
        for pos in box_positions(lo, hi) {
            if in_ellipsoid(pos, c, r) {
                self.set(pos, code);
            }
        }
        Ok(())
    }

    pub fn include(&mut self, other: &Model, offset: Int3) -> PyResult<()> {
        if !other.palette.is(&self.palette) {
            return Err(PyValueError::new_err("model does not belong to this model's palette"));
        }
        let (ox, oy, oz) = offset;
        let extent = (other.dims.x - 1, other.dims.y - 1, other.dims.z - 1);
        for local in box_positions((0, 0, 0), extent) {
            let Some(code) = other.get(local) else {
                continue;
            };
            let (lx, ly, lz) = local;
            let pos = (ox + lx, oy + ly, oz + lz);
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
    let (ax, ay, az) = a;
    let (bx, by, bz) = b;
    let (x0, x1) = (ax.min(bx), ax.max(bx));
    let (y0, y1) = (ay.min(by), ay.max(by));
    let (z0, z1) = (az.min(bz), az.max(bz));
    (z0..=z1).flat_map(move |z| (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y, z))))
}

fn in_ellipsoid(pos: Int3, center: Int3, radii: Int3) -> bool {
    let (x, y, z) = pos;
    let (cx, cy, cz) = center;
    let (rx, ry, rz) = radii;
    let nx = (x as f64 - cx as f64) / rx as f64;
    let ny = (y as f64 - cy as f64) / ry as f64;
    let nz = (z as f64 - cz as f64) / rz as f64;
    nx * nx + ny * ny + nz * nz <= 1.0
}

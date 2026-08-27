use either::Either;
use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, Python, pyclass, pymethods};

use crate::math::{Int3, Vec3};
use crate::palette::{Material, MaterialCode, Palette};
use crate::render::{CameraAngle, RenderOutput};
use crate::scene::{Node, Scene};
use crate::tools::render;

/// The size of a model's voxel grid along each axis.
#[pyclass(get_all, from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct Dimensions {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl From<Int3> for Dimensions {
    fn from(Int3 { x, y, z }: Int3) -> Self {
        Self { x, y, z }
    }
}

impl Dimensions {
    fn last_index(self) -> Int3 {
        Int3::from(self) - Int3::ONE
    }
}

#[pymethods]
impl Dimensions {
    /// Create dimensions of `x` by `y` by `z` voxels, each from 1 to 256.
    #[new]
    fn new(x: usize, y: usize, z: usize) -> PyResult<Self> {
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

    /// Return whether `a` is a valid voxel coordinate within these dimensions.
    fn contains(&self, a: Int3) -> bool {
        a.x < self.x && a.y < self.y && a.z < self.z
    }

    /// Return these dimensions as a `Vec3`.
    fn as_vec(&self) -> Vec3 {
        Int3::new(self.x, self.y, self.z).into()
    }
}

/// Which point of a model's voxel grid to place at its node's origin once in a scene.
///
/// `Corner` uses the grid's `(0, 0, 0)` corner; `Center` uses its center; `BottomCenter` uses
/// the center of its base. For an arbitrary point, pass a `Vec3` in grid coordinates (voxels,
/// one per unit) instead of a `Pivot`.
#[pyclass(from_py_object, frozen)]
#[derive(Copy, Clone)]
pub enum Pivot {
    Corner,
    Center,
    BottomCenter,
}

/// A 3D grid of voxels, each empty or set to one material of the model's palette.
///
/// Voxel coordinates are `(x, y, z)` tuples indexing from `(0, 0, 0)`; y is up. Coordinates
/// outside the grid raise `ValueError`, though spheres and ellipsoids reaching past an edge
/// are clipped to it.
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
                "coordinates ({}, {}, {}) are out of bounds of this model",
                a.x, a.y, a.z,
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
        pos.x + pos.y * self.dims.x + pos.z * self.zstride
    }

    pub fn pivot_offset(&self) -> Vec3 {
        match self.pivot {
            Either::Left(Pivot::Corner) => Vec3::ZERO,
            Either::Left(Pivot::Center) => self.dims.as_vec().__mul__(0.5),
            Either::Left(Pivot::BottomCenter) => {
                Vec3::new_unchecked(self.dims.x as f64 * 0.5, 0.0, self.dims.z as f64 * 0.5)
            }
            Either::Right(v) => v,
        }
    }

    fn get(&self, pos: Int3) -> Option<MaterialCode> {
        self.data[self.index(pos)]
    }

    fn set(&mut self, pos: Int3, code: Option<MaterialCode>) {
        self.data[self.index(pos)] = code;
    }

    fn fill_ellipsoid(&mut self, material: Option<&Material>, center: Int3, radii: Int3) -> PyResult<()> {
        self.check_contains(center)?;
        let code = self.get_material_code(material)?;
        if radii.any(|r| r == 0) {
            return Err(PyValueError::new_err("radius must be at least 1"));
        }
        let r = Vec3::from(radii);
        let lo = center.saturating_sub(radii);
        let hi = (center + radii).min(self.dims.last_index());
        for pos in box_positions(lo, hi) {
            let delta = Vec3::from(pos).__sub__(center.into());
            let nx = delta.x() / r.x();
            let ny = delta.y() / r.y();
            let nz = delta.z() / r.z();
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
    /// Create an empty model.
    ///
    /// Args:
    ///     dims: The grid's size along each axis.
    ///     palette: The palette whose materials this model's voxels may use.
    ///     pivot: Which point of the grid to place at the node's origin — a `Pivot`, or a
    ///         `Vec3` in grid coordinates (the same voxel space as `put` and `aabb`).
    #[new]
    pub fn new(dims: Dimensions, palette: Py<Palette>, pivot: Either<Pivot, Vec3>) -> Self {
        Self {
            dims,
            zstride: dims.x * dims.y,
            data: vec![None; dims.x * dims.y * dims.z].into_boxed_slice(),
            pivot,
            palette,
        }
    }

    /// Return an independent copy of the model that shares the same palette.
    pub fn copy(&self, py: Python) -> Self {
        Self {
            dims: self.dims,
            zstride: self.zstride,
            data: self.data.clone(),
            pivot: self.pivot,
            palette: self.palette.clone_ref(py),
        }
    }

    /// Return a new model holding just the voxels in the box between `p1` and `p2` inclusive.
    ///
    /// The new model keeps this model's palette and pivot.
    pub fn clip(&self, p1: Int3, p2: Int3, py: Python) -> PyResult<Self> {
        self.check_contains(p1)?;
        self.check_contains(p2)?;
        let lo = p1.min(p2);
        let hi = p1.max(p2);
        let dims = Dimensions::from(hi - lo + Int3::ONE);
        let mut clipped = Self::new(dims, self.palette.clone_ref(py), self.pivot);
        for pos in box_positions(lo, hi) {
            clipped.set(pos - lo, self.get(pos));
        }
        Ok(clipped)
    }

    /// Mirror the model in place across the x axis.
    pub fn flip_x(&mut self) {
        self.flip_axis(1, self.dims.x);
    }

    /// Mirror the model in place across the y axis.
    pub fn flip_y(&mut self) {
        self.flip_axis(self.dims.x, self.dims.y);
    }

    /// Mirror the model in place across the z axis.
    pub fn flip_z(&mut self) {
        self.flip_axis(self.zstride, self.dims.z);
    }

    /// Set the voxel at `p`, or clear it when `material` is `None`.
    pub fn put(&mut self, material: Option<&Material>, p: Int3) -> PyResult<()> {
        self.check_contains(p)?;
        let code = self.get_material_code(material)?;
        self.set(p, code);
        Ok(())
    }

    /// Fill the axis-aligned box between corners `p1` and `p2` inclusive.
    ///
    /// A `material` of `None` clears the box instead.
    pub fn aabb(&mut self, material: Option<&Material>, p1: Int3, p2: Int3) -> PyResult<()> {
        self.check_contains(p1)?;
        self.check_contains(p2)?;
        let code = self.get_material_code(material)?;
        for pos in box_positions(p1, p2) {
            self.set(pos, code);
        }
        Ok(())
    }

    /// Fill a sphere of radius `r`, in voxels, centered on `c`.
    ///
    /// A `material` of `None` clears the sphere instead.
    pub fn sphere(&mut self, material: Option<&Material>, c: Int3, r: usize) -> PyResult<()> {
        self.fill_ellipsoid(material, c, Int3::new(r, r, r))
    }

    /// Fill a spheroid centered on `c`, with horizontal radius `r_eq` and vertical radius `r_polar`.
    ///
    /// A `material` of `None` clears the spheroid instead.
    pub fn spheroid(&mut self, material: Option<&Material>, c: Int3, r_eq: usize, r_polar: usize) -> PyResult<()> {
        self.fill_ellipsoid(material, c, Int3::new(r_eq, r_polar, r_eq))
    }

    /// Fill an ellipsoid centered on `c`, with radii `rx`, `ry` and `rz` along each axis.
    ///
    /// A `material` of `None` clears the ellipsoid instead.
    pub fn ellipsoid(&mut self, material: Option<&Material>, c: Int3, rx: usize, ry: usize, rz: usize) -> PyResult<()> {
        self.fill_ellipsoid(material, c, Int3::new(rx, ry, rz))
    }

    /// Stamp every set voxel of `other` into this model, shifted by `offset`.
    ///
    /// Both models must share the same palette. Empty voxels in `other` leave this model
    /// unchanged; `other` must fit entirely within bounds once shifted.
    pub fn include(&mut self, other: &Model, offset: Int3) -> PyResult<()> {
        self.check_contains(other.dims.last_index() + offset)?;
        if !other.palette.is(&self.palette) {
            return Err(PyValueError::new_err(
                "self and other have different palettes, which isn't allowed",
            ));
        }
        for local in box_positions(Int3::ZERO, other.dims.last_index()) {
            if let code @ Some(_) = other.get(local) {
                self.set(offset + local, code)
            }
        }
        Ok(())
    }

    /// Render the model on its own from each camera angle and return the image files.
    ///
    /// A shortcut for wrapping the model in a one-node scene; see `Scene.render`.
    pub fn render(slf: Bound<Self>, angles: Vec<CameraAngle>) -> PyResult<RenderOutput> {
        let scene = Bound::new(slf.py(), Scene::default())?;
        let node = Scene::create_root_node(scene.clone(), "root".to_string(), None)?;
        Node::add_model(node.bind(slf.py()).clone(), "model".to_string(), slf.unbind(), None)?;
        render(scene, angles, vec![], None, None, None)
    }
}

fn box_positions(a: Int3, b: Int3) -> impl Iterator<Item = Int3> {
    let lo = a.min(b);
    let hi = a.max(b);
    (lo.z..=hi.z).flat_map(move |z| (lo.y..=hi.y).flat_map(move |y| (lo.x..=hi.x).map(move |x| Int3::new(x, y, z))))
}

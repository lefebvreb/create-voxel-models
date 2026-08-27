use std::f64::consts::PI;
use std::ops::{Add, Sub};

use pyo3::exceptions::PyValueError;
use pyo3::inspect::PyStaticExpr;
use pyo3::{Borrowed, FromPyObject, PyAny, PyResult, pyclass, pymethods};

use crate::model::Dimensions;

/// A 3D vector with double-precision components.
#[pyclass(from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct Vec3 {
    pub inner: glam::DVec3,
}

impl Vec3 {
    pub fn new_unchecked(x: f64, y: f64, z: f64) -> Self {
        Self {
            inner: glam::DVec3::new(x, y, z),
        }
    }
}

#[pymethods]
impl Vec3 {
    /// The zero vector.
    #[classattr]
    pub const ZERO: Self = Self {
        inner: glam::DVec3::ZERO,
    };

    /// The vector whose components are all one.
    #[classattr]
    pub const ONES: Self = Self {
        inner: glam::DVec3::ONE,
    };

    /// Create a vector with every component equal to `t`.
    #[staticmethod]
    pub fn splat(t: f64) -> PyResult<Self> {
        if !t.is_finite() {
            return Err(PyValueError::new_err("t must be finite"));
        }
        Ok(Self {
            inner: glam::DVec3::splat(t),
        })
    }

    #[new]
    pub fn new(x: f64, y: f64, z: f64) -> PyResult<Self> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(PyValueError::new_err("x, y, and z must all be finite"));
        }
        Ok(Self {
            inner: glam::DVec3::new(x, y, z),
        })
    }

    #[getter]
    pub fn x(&self) -> f64 {
        self.inner.x
    }

    #[getter]
    pub fn y(&self) -> f64 {
        self.inner.y
    }

    #[getter]
    pub fn z(&self) -> f64 {
        self.inner.z
    }

    /// Negate each component.
    pub fn __neg__(&self) -> Self {
        Self { inner: -self.inner }
    }

    /// Add `self` and `other`, another `Vec3`, component-wise.
    pub fn __add__(&self, other: Self) -> Self {
        Self {
            inner: self.inner + other.inner,
        }
    }

    /// Subtract `other`, another `Vec3`, from `self` component-wise.
    pub fn __sub__(&self, other: Self) -> Self {
        Self {
            inner: self.inner - other.inner,
        }
    }

    /// Scale `self` by the scalar `other`.
    pub fn __mul__(&self, other: f64) -> Self {
        Self {
            inner: self.inner * other,
        }
    }

    /// Divide `self` by the scalar `other`.
    pub fn __truediv__(&self, other: f64) -> Self {
        Self {
            inner: self.inner / other,
        }
    }
}

impl From<Int3> for Vec3 {
    fn from(Int3 { x, y, z }: Int3) -> Self {
        Self::new_unchecked(x as f64, y as f64, z as f64)
    }
}

/// A unit quaternion with double-precision components, representing a 3D rotation.
#[pyclass(from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct Quat {
    pub inner: glam::DQuat,
}

#[pymethods]
impl Quat {
    /// The identity quaternion, representing no rotation.
    #[classattr]
    pub const IDENTITY: Self = Self {
        inner: glam::DQuat::IDENTITY,
    };

    /// Create a quaternion rotating `angle` degrees about the x axis.
    #[staticmethod]
    pub fn from_rotation_x(angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_rotation_x(deg_to_rad(angle)),
        })
    }

    /// Create a quaternion rotating `angle` degrees about the y axis.
    #[staticmethod]
    pub fn from_rotation_y(angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_rotation_y(deg_to_rad(angle)),
        })
    }

    /// Create a quaternion rotating `angle` degrees about the z axis.
    #[staticmethod]
    pub fn from_rotation_z(angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_rotation_z(deg_to_rad(angle)),
        })
    }

    /// Create a quaternion rotating `angle` degrees about `axis`.
    ///
    /// `axis` need not be normalized.
    #[staticmethod]
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_axis_angle(axis.inner.normalize(), deg_to_rad(angle)),
        })
    }

    /// Return the conjugate of `self`, which is the inverse rotation.
    pub fn conjugate(&self) -> Self {
        Self {
            inner: self.inner.conjugate(),
        }
    }

    /// Compose `self` with `other`, another `Quat`, applying `other` first.
    pub fn __mul__(&self, other: Self) -> Self {
        Self {
            inner: self.inner * other.inner,
        }
    }
}

pub fn deg_to_rad(a: f64) -> f64 {
    a * (PI / 180.0)
}

#[derive(Copy, Clone)]
pub struct Int3 {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl Int3 {
    pub const ZERO: Self = Self::new(0, 0, 0);

    pub const ONE: Self = Self::new(1, 1, 1);

    pub const fn new(x: usize, y: usize, z: usize) -> Self {
        Self { x, y, z }
    }

    pub fn any(self, f: impl FnMut(usize) -> bool) -> bool {
        [self.x, self.y, self.z].into_iter().any(f)
    }

    pub fn min(self, other: Self) -> Self {
        Self::new(self.x.min(other.x), self.y.min(other.y), self.z.min(other.z))
    }

    pub fn max(self, other: Self) -> Self {
        Self::new(self.x.max(other.x), self.y.max(other.y), self.z.max(other.z))
    }

    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self::new(
            self.x.saturating_sub(rhs.x),
            self.y.saturating_sub(rhs.y),
            self.z.saturating_sub(rhs.z),
        )
    }
}

impl Add for Int3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Int3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl From<Dimensions> for Int3 {
    fn from(Dimensions { x, y, z }: Dimensions) -> Self {
        Self { x, y, z }
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Int3 {
    type Error = <(usize, usize, usize) as FromPyObject<'a, 'py>>::Error;

    const INPUT_TYPE: PyStaticExpr = <(usize, usize, usize) as FromPyObject<'a, 'py>>::INPUT_TYPE;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        obj.extract::<(usize, usize, usize)>()
            .map(|(x, y, z)| Self::new(x, y, z))
    }
}

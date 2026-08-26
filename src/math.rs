use std::f64::consts::PI;

use pyo3::exceptions::PyValueError;
use pyo3::{PyResult, pyclass, pymethods};

/// A 3-dimensional real column vector, with double precision components.
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
    /// A vector that only contains zeros.
    #[classattr]
    pub const ZERO: Self = Self {
        inner: glam::DVec3::ZERO,
    };

    /// A vector that only contains ones.
    #[classattr]
    pub const ONES: Self = Self {
        inner: glam::DVec3::ONE,
    };

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

    pub fn __neg__(&self) -> Self {
        Self { inner: -self.inner }
    }

    /// Component-wise addition of `self` and `other`.
    ///
    /// `other` must be a `Vec3`.
    pub fn __add__(&self, other: Self) -> Self {
        Self {
            inner: self.inner + other.inner,
        }
    }

    /// Component-wise subtraction of `self` and `other`.
    ///
    /// `other` must be a `Vec3`.
    pub fn __sub__(&self, other: Self) -> Self {
        Self {
            inner: self.inner - other.inner,
        }
    }

    /// Scaling of `self` by the scalar factor `other`.
    ///
    /// `other` must be a `float`.
    pub fn __mul__(&self, other: f64) -> Self {
        Self {
            inner: self.inner * other,
        }
    }

    /// Scaling of `self` by the scalar factor `other⁻¹`.
    ///
    /// `other` must be a `float`.
    pub fn __truediv__(&self, other: f64) -> Self {
        Self {
            inner: self.inner / other,
        }
    }
}

/// A quaternion, with double precisions components.
#[pyclass(from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct Quat {
    pub inner: glam::DQuat,
}

#[pymethods]
impl Quat {
    /// The identity quaternion, equivalent to no rotation.
    #[classattr]
    pub const IDENTITY: Self = Self {
        inner: glam::DQuat::IDENTITY,
    };

    /// Creates a quaternion that rotates `angle` degrees around the x axis.
    #[staticmethod]
    pub fn from_rotation_x(angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_rotation_x(deg_to_rad(angle)),
        })
    }

    /// Creates a quaternion that rotates `angle` degrees around the y axis.
    #[staticmethod]
    pub fn from_rotation_y(angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_rotation_y(deg_to_rad(angle)),
        })
    }

    /// Creates a quaternion that rotates `angle` degrees around the z axis.
    #[staticmethod]
    pub fn from_rotation_z(angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_rotation_z(deg_to_rad(angle)),
        })
    }

    /// Creates a quaternion that rotates `angle` degrees around `axis`.
    /// `axis` does not need to be normalized.
    #[staticmethod]
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> PyResult<Self> {
        if !angle.is_finite() {
            return Err(PyValueError::new_err("angle must be finite"));
        }
        Ok(Self {
            inner: glam::DQuat::from_axis_angle(axis.inner.normalize(), deg_to_rad(angle)),
        })
    }

    pub fn conjugate(&self) -> Self {
        Self {
            inner: self.inner.conjugate(),
        }
    }

    /// Quaternion product, `other` must be a `Quat`.
    pub fn __mul__(&self, other: Self) -> Self {
        Self {
            inner: self.inner * other.inner,
        }
    }
}

pub fn deg_to_rad(a: f64) -> f64 {
    a * (PI / 180.0)
}

pub type Int3 = (usize, usize, usize);

pub mod int3 {
    use super::{Int3, Vec3};

    pub const ZERO: Int3 = (0, 0, 0);
    pub const ONE: Int3 = (1, 1, 1);

    pub fn contains_zero((x, y, z): Int3) -> bool {
        x == 0 || y == 0 || z == 0
    }

    pub fn min((ax, ay, az): Int3, (bx, by, bz): Int3) -> Int3 {
        (ax.min(bx), ay.min(by), az.min(bz))
    }

    pub fn max((ax, ay, az): Int3, (bx, by, bz): Int3) -> Int3 {
        (ax.max(bx), ay.max(by), az.max(bz))
    }

    pub fn add((ax, ay, az): Int3, (bx, by, bz): Int3) -> Int3 {
        (ax + bx, ay + by, az + bz)
    }

    pub fn sub((ax, ay, az): Int3, (bx, by, bz): Int3) -> Int3 {
        (ax - bx, ay - by, az - bz)
    }

    pub fn saturating_sub((ax, ay, az): Int3, (bx, by, bz): Int3) -> Int3 {
        (ax.saturating_sub(bx), ay.saturating_sub(by), az.saturating_sub(bz))
    }

    pub fn into_vec3((x, y, z): Int3) -> Vec3 {
        Vec3::new_unchecked(x as f64, y as f64, z as f64)
    }
}

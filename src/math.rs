use std::f64::consts::PI;

use either::Either;
use pyo3::{Bound, pyclass, pymethods};

/// A 3-dimensional real column vector, with double precision components.
#[pyclass(frozen, from_py_object)]
#[derive(Copy, Clone)]
pub struct Vec3 {
    pub inner: glam::DVec3,
}

#[pymethods]
impl Vec3 {
    /// A vector that only contains zeros.
    #[classattr]
    const ZERO: Self = Self {
        inner: glam::DVec3::ZERO,
    };
    
    /// A vector that only contains ones.
    #[classattr]
    const ONES: Self = Self {
        inner: glam::DVec3::ONE,
    };

    #[new]
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            inner: glam::DVec3::new(x, y, z),
        }
    }

    #[getter]
    fn x(&self) -> f64 {
        self.inner.x
    }

    #[getter]
    fn y(&self) -> f64 {
        self.inner.y
    }

    #[getter]
    fn z(&self) -> f64 {
        self.inner.z
    }

    fn __neg__(&self) -> Self {
        Self { inner: -self.inner }
    }

    /// Component-wise addition, `other` must be a `Vec3`.
    fn __add__(&self, other: &Self) -> Self {
        Self {
            inner: self.inner + other.inner,
        }
    }

    /// Component-wise subtraction, `other` must be a `Vec3`.
    fn __sub__(&self, other: &Self) -> Self {
        Self {
            inner: self.inner - other.inner,
        }
    }

    /// Component-wise subtraction when `other` is a `Vec3`.
    /// Otherwise just scales `self` by `other`, which is then required to be a `float`.
    fn __mul__(&self, other: Either<f64, Bound<Self>>) -> Self {
        Self {
            inner: match other {
                Either::Left(scale) => self.inner * scale,
                Either::Right(other) => self.inner * other.get().inner,
            },
        }
    }
    /// Scales `self` by `other⁻¹`, which is required to be a `float`.
    fn __truediv__(&self, other: f64) -> Self {
        Self {
            inner: self.inner / other,
        }
    }
}

/// A quaternion, with double precisions components.
#[pyclass(frozen, from_py_object)]
#[derive(Copy, Clone)]
pub struct Quat {
    pub inner: glam::DQuat,
}

#[pymethods]
impl Quat {
    /// The identity quaternion, equivalent to no rotation.
    #[classattr]
    const IDENTITY: Self = Self {
        inner: glam::DQuat::IDENTITY,
    };

    /// Creates a queternion that rotates `angle` degrees around the x axis.
    #[staticmethod]
    fn from_rotation_x(angle: f64) -> Self {
        Self {
            inner: glam::DQuat::from_rotation_x(deg_to_rad(angle)),
        }
    }

    /// Creates a queternion that rotates `angle` degrees around the y axis.
    #[staticmethod]
    fn from_rotation_y(angle: f64) -> Self {
        Self {
            inner: glam::DQuat::from_rotation_y(deg_to_rad(angle)),
        }
    }

    /// Creates a queternion that rotates `angle` degrees around the z axis.
    #[staticmethod]
    fn from_rotation_z(angle: f64) -> Self {
        Self {
            inner: glam::DQuat::from_rotation_z(deg_to_rad(angle)),
        }
    }

    fn conjugate(&self) -> Self {
        Self {
            inner: self.inner.conjugate(),
        }
    }

    /// Quaternion product, `other` must be a `Quat`.
    fn __mul__(&self, other: &Self) -> Self {
        Self {
            inner: self.inner * other.inner,
        }
    }
}

fn deg_to_rad(a: f64) -> f64 {
    a * (PI / 180.0)
}

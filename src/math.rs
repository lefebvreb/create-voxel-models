use std::f64::consts::PI;

use either::Either;
use pyo3::{Bound, pyclass, pymethods};

pub type Pos = (usize, usize, usize);

#[pyclass(frozen)]
pub struct Vec3 {
    pub inner: glam::DVec3,
}

#[pymethods]
impl Vec3 {
    #[classattr]
    const ZERO: Self = Self {
        inner: glam::DVec3::ZERO,
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

    fn __add__(&self, other: Bound<Self>) -> Self {
        Self {
            inner: self.inner + other.get().inner,
        }
    }

    fn __sub__(&self, other: Bound<Self>) -> Self {
        Self {
            inner: self.inner - other.get().inner,
        }
    }

    fn __mul__(&self, other: Either<f64, Bound<Self>>) -> Self {
        Self {
            inner: match other {
                Either::Left(scale) => self.inner * scale,
                Either::Right(other) => self.inner * other.get().inner,
            },
        }
    }

    fn __div__(&self, scale: f64) -> Self {
        Self {
            inner: self.inner / scale,
        }
    }
}

fn deg_to_rad(a: f64) -> f64 {
    a * (PI / 180.0)
}

#[pyclass(frozen)]
pub struct Quat {
    pub inner: glam::DQuat,
}

#[pymethods]
impl Quat {
    #[classattr]
    const IDENTITY: Self = Self {
        inner: glam::DQuat::IDENTITY,
    };

    #[staticmethod]
    fn from_rotation_x(angle: f64) -> Self {
        Self {
            inner: glam::DQuat::from_rotation_x(deg_to_rad(angle)),
        }
    }

    #[staticmethod]
    fn from_rotation_y(angle: f64) -> Self {
        Self {
            inner: glam::DQuat::from_rotation_y(deg_to_rad(angle)),
        }
    }

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

    fn __mul__(&self, other: Bound<Self>) -> Self {
        Self {
            inner: self.inner * other.get().inner,
        }
    }
}

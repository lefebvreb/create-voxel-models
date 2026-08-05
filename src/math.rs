use std::f64::consts::PI;

use either::Either;
use pyo3::{Bound, pyclass, pymethods};

#[pyclass(frozen, get_all)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[pymethods]
impl Vec3 {
    #[new]
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn __neg__(&self) -> Self {
        Self::new(-self.x, -self.y,- self.z)
    }

    fn __add__(&self, other: Bound<Self>) -> Self {
        let Self { x, y, z } = other.get();
        Self::new(self.x + x, self.y + y, self.z + z)
    }

    fn __sub__(&self, other: Bound<Self>) -> Self {
        let Self { x, y, z } = other.get();
        Self::new(self.x - x, self.y - y, self.z - z)
    }

    fn __mul__(&self, other: Either<f64, Bound<Self>>) -> Self {
        match other {
            Either::Left(scale) => Self::new(self.x * scale, self.y * scale, self.z * scale),
            Either::Right(other) => {
                let Self { x, y, z } = other.get();
                Self::new(self.x * x, self.y * y, self.z * z)
            },
        }
    }

    fn __div__(&self, scale: f64) -> Self {
        Self::new(self.x / scale, self.y / scale, self.z / scale)
    }
}

#[pyclass(frozen, get_all)]
pub struct Quat {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}

impl Quat {
    const fn new(a: f64, b: f64, c: f64, d: f64) -> Self {
        Self { a, b, c, d }
    }
}

#[pymethods]
impl Quat {
    #[classattr]
    const ID: Self = Self::new(1.0, 0.0, 0.0, 0.0);

    #[staticmethod]
    fn from_rotation_x(angle: f64) -> Self {
        let (s, c) = (deg_to_rad(angle) * 0.5).sin_cos();
        Self::new(s, 0.0, 0.0, c)
    }

    #[staticmethod]
    fn from_rotation_y(angle: f64) -> Self {
        let (s, c) = (deg_to_rad(angle) * 0.5).sin_cos();
        Self::new(0.0, s, 0.0, c)
    }

    #[staticmethod]
    fn from_rotation_z(angle: f64) -> Self {
        let (s, c) = (deg_to_rad(angle) * 0.5).sin_cos();
        Self::new(0.0, 0.0, s, c)
    }

    fn conjugate(&self) -> Self {
        Self::new(self.a, -self.b, -self.c, -self.d)
    }

    fn __mul__(&self, other: Bound<Self>) -> Self {
        let Self { a: a1, b: b1, c: c1, d: d1 } = self;
        let Self { a: a2, b: b2, c: c2, d: d2 } = other.get();
        Self::new(
            a1 * a2 - b1 * b2 - c1 * c2 - d1 * d2,
            a1 * b2 + b1 * a2 + c1 * d2 - d1 * c2,
            a1 * c2 - b1 * d2 + c1 * a2 + d1 * b2,
            a1 * d2 + b1 * c2 - c1 * b2 + d1 * a2,
        )
    }
}

fn deg_to_rad(a: f64) -> f64 {
    a * (PI / 180.0)
}

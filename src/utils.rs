use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use pyo3::Py;

pub type Dict = HashMap<String, String>;

pub type Int3 = (usize, usize, usize);

pub mod int3 {
    use crate::math::Vec3;
    use crate::utils::Int3;

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
        Vec3::new(x as f64, y as f64, z as f64)
    }
}

pub struct HashPy<T>(pub Py<T>);

impl<T> PartialEq for HashPy<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.is(&other.0)
    }
}

impl<T> Eq for HashPy<T> {}

impl<T> Hash for HashPy<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.0.as_ptr() as usize);
    }
}

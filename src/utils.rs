use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use pyo3::Py;

pub type Dict = HashMap<String, String>;

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

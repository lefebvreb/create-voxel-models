use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use pyo3::exceptions::PyValueError;
use pyo3::{Py, PyResult};

pub type Dict = HashMap<String, String>;

pub type Int3 = (usize, usize, usize);

pub struct HashPy<T>(pub Py<T>);

pub fn encode_rgb_png(width: u32, height: u32, pixels: &[u8]) -> PyResult<Vec<u8>> {
    encode_png(width, height, pixels, png::ColorType::Rgb)
}

pub fn encode_gray_png(width: u32, height: u32, pixels: &[u8]) -> PyResult<Vec<u8>> {
    encode_png(width, height, pixels, png::ColorType::Grayscale)
}

fn encode_png(width: u32, height: u32, pixels: &[u8], color: png::ColorType) -> PyResult<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    writer
        .write_image_data(pixels)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    writer.finish().map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(bytes)
}

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

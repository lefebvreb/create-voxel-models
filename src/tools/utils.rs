// <ai-owned/>

use png::{BitDepth, ColorType, Encoder};
use pyo3::PyResult;
use pyo3::exceptions::PyValueError;

pub fn encode_png(width: u32, height: u32, pixels: &[u8], color: ColorType) -> PyResult<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    writer
        .write_image_data(pixels)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    writer.finish().map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(bytes)
}

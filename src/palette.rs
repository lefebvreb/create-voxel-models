use pyo3::exceptions::PyIndexError;
use pyo3::{Bound, Py, PyResult, pyclass, pymethods};

pub struct ColorData {
    pub rgba: [u8; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub ior: f32,
    pub transmission: f32,
    pub emissive: f32,
}

#[pyclass]
pub struct Color {
    index: usize,
    palette: Py<Palette>,
}

#[pyclass]
pub struct Palette {
    colors: Vec<ColorData>,
}

#[pymethods]
impl Palette {
    #[new]
    pub fn new() -> Self {
        Self { colors: Vec::new() }
    }

    #[pyo3(signature = (rgba, *, roughness = 1.0, metallic = 0.0, ior = 1.5, transmission = 0.0, emissive = 0.0))]
    pub fn add_color(
        bound: Bound<Self>,
        rgba: [u8; 3],
        roughness: f32,
        metallic: f32,
        ior: f32,
        transmission: f32,
        emissive: f32,
    ) -> PyResult<Color> {
        let mut this = bound.borrow_mut();
        if this.colors.len() == 255 {
            return Err(PyIndexError::new_err(
                "palette already contains 255 colors, which is the maximum permitted",
            ));
        }
        this.colors.push(ColorData {
            rgba,
            roughness,
            metallic,
            ior,
            transmission,
            emissive,
        });
        Ok(Color {
            index: this.colors.len(),
            palette: bound.unbind(),
        })
    }
}

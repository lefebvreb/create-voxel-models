use pyo3::{pyclass, pymethods};

#[pyclass]
pub struct Color {
    pub rgba: [u8; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub ior: f32,
    pub transmission: f32,
    pub emissive: f32,
}

#[pymethods]
impl Color {
    #[new]
    #[pyo3(signature = (rgba, *, roughness = 1.0, metallic = 0.0, ior = 1.5, transmission = 0.0, emissive = 0.0))]
    fn new(rgba: [u8; 3], roughness: f32, metallic: f32, ior: f32, transmission: f32, emissive: f32) -> Self {
        Self {
            rgba,
            roughness,
            metallic,
            ior,
            transmission,
            emissive,
        }
    }
}

pub struct Palette {}

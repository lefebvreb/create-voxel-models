use pyo3::exceptions::PyIndexError;
use pyo3::{Bound, Py, PyResult, pyclass, pymethods};

pub struct ColorData {
    pub rgba: (u8, u8, u8),
    pub roughness: f32,
    pub metallic: f32,
    pub ior: f32,
    pub transmission: f32,
    pub emissive: f32,
}

#[pyclass(frozen)]
pub struct Color {
    /// 1-based index into the palette's `colors` field.
    index: u8,
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
        slf: Bound<Self>,
        rgba: (u8, u8, u8),
        roughness: f32,
        metallic: f32,
        ior: f32,
        transmission: f32,
        emissive: f32,
    ) -> PyResult<Color> {
        let mut obj = slf.borrow_mut();
        if obj.colors.len() == 255 {
            return Err(PyIndexError::new_err(
                "palette already contains 255 colors, which is the maximum permitted",
            ));
        }
        obj.colors.push(ColorData {
            rgba,
            roughness,
            metallic,
            ior,
            transmission,
            emissive,
        });
        Ok(Color {
            index: u8::try_from(obj.colors.len()).expect("palette index should fit in a byte"),
            palette: slf.unbind(),
        })
    }
}

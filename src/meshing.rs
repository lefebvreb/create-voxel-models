use pyo3::{Bound, PyResult};

use crate::model::Model;
use crate::palette::Palette;

pub struct MeshData {
    // Vertices, Triangles, UVs...
}

pub fn export_model(model: Bound<Model>) -> PyResult<MeshData> {
    todo!()
}

pub struct PaletteData {
    // Textures...
}

pub fn export_palette(palette: Bound<Palette>) -> PyResult<PaletteData> {
    todo!()
}

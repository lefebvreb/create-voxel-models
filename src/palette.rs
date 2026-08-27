use std::num::NonZeroU8;

use pyo3::exceptions::PyValueError;
use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

/// An RGB color with 8 bits per channel.
#[pyclass(from_py_object, frozen, get_all)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[pymethods]
impl Color {
    /// Create a color from its red, green and blue channels, each from 0 to 255.
    #[new]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MaterialCode(NonZeroU8);

impl MaterialCode {
    pub fn new(index: usize) -> Self {
        let code = u8::try_from(index + 1).expect("index should be 254 at most");
        Self(NonZeroU8::new(code).unwrap())
    }

    pub fn index(self) -> usize {
        usize::from(self.0.get()) - 1
    }
}

/// A PBR material held by a `Palette`. Create one with `Palette.add_material`.
#[pyclass(frozen)]
pub struct Material {
    pub code: MaterialCode,
    #[pyo3(get)]
    pub color: Color,
    #[pyo3(get)]
    pub roughness: f64,
    #[pyo3(get)]
    pub metallic: f64,
    #[pyo3(get)]
    pub ior: f64,
    #[pyo3(get)]
    pub transmission: f64,
    #[pyo3(get)]
    pub emissive: f64,
    #[pyo3(get)]
    pub volume: Option<Volume>,
    #[pyo3(get)]
    pub palette: Py<Palette>,
}

#[pymethods]
impl Material {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.palette)
    }
}

/// Volumetric light attenuation for a transmissive material, giving colored glass its tint.
#[pyclass(get_all, from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct Volume {
    pub color: Color,
    pub distance: f64,
    pub thickness: f64,
}

#[pymethods]
impl Volume {
    /// Define how transmitted light is absorbed inside a material.
    ///
    /// Args:
    ///     color: The color light is attenuated toward as it travels through the volume.
    ///     distance: Distance, in voxels, over which light is attenuated to `color`. Must be
    ///         positive.
    ///     thickness: Thickness, in voxels, of the material's walls.
    #[new]
    #[pyo3(signature = (color, distance, *, thickness = 1.0))]
    pub fn new(color: Color, distance: f64, thickness: f64) -> PyResult<Self> {
        if !(distance.is_finite() && distance > 0.0) {
            return Err(PyValueError::new_err("distance must be finite and greater than 0.0"));
        }
        if !(thickness.is_finite() && thickness > 0.0) {
            return Err(PyValueError::new_err("thickness must be finite and greater than 0.0"));
        }
        Ok(Self {
            color,
            distance,
            thickness,
        })
    }
}

/// An ordered set of up to 255 materials, shared between the models that use them.
#[pyclass]
#[derive(Default)]
pub struct Palette {
    pub materials: Vec<Py<Material>>,
}

#[pymethods]
impl Palette {
    /// Create an empty palette.
    #[new]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a material to the palette and return it.
    ///
    /// Args:
    ///     color: The material's base color.
    ///     roughness: Surface roughness, from 0.0 (mirror-smooth) to 1.0 (fully matte).
    ///     metallic: 0.0 for a dielectric, 1.0 for a metal.
    ///     ior: Index of refraction; either 0.0 or at least 1.0. Only affects transmissive
    ///         materials.
    ///     transmission: Fraction of light passing through the surface, from 0.0 (opaque) to
    ///         1.0 (fully transmissive).
    ///     emissive: Emitted light strength; 0.0 for a non-emitter, larger values glow brighter.
    ///     volume: Volumetric attenuation applied to transmitted light, if any.
    ///
    /// Raises:
    ///     ValueError: If an argument is out of range, or the palette already holds 255 materials.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (color, *, roughness = 1.0, metallic = 0.0, ior = 1.5, transmission = 0.0, emissive = 0.0, volume = None))]
    pub fn add_material(
        slf: Bound<Self>,
        color: Color,
        roughness: f64,
        metallic: f64,
        ior: f64,
        transmission: f64,
        emissive: f64,
        volume: Option<Volume>,
    ) -> PyResult<Py<Material>> {
        if !(0.0..=1.0).contains(&roughness) {
            return Err(PyValueError::new_err("roughness must be between 0.0 and 1.0"));
        }
        if !(0.0..=1.0).contains(&metallic) {
            return Err(PyValueError::new_err("metallic must be between 0.0 and 1.0"));
        }
        if !(0.0..=1.0).contains(&transmission) {
            return Err(PyValueError::new_err("transmission must be between 0.0 and 1.0"));
        }
        if !(ior == 0.0 || (ior.is_finite() && ior >= 1.0)) {
            return Err(PyValueError::new_err(
                "ior must be 0.0, or finite and greater than or equal to 1.0",
            ));
        }
        if !(emissive.is_finite() && emissive >= 0.0) {
            return Err(PyValueError::new_err(
                "emissive must be finite and greater than or equal to 0.0",
            ));
        }
        let mut slf_brw = slf.borrow_mut();
        let index = slf_brw.materials.len();
        if index == 255 {
            return Err(PyValueError::new_err(
                "palette already contains 255 materials, which is the maximum permitted",
            ));
        }
        let material = Py::new(
            slf.py(),
            Material {
                code: MaterialCode::new(index),
                color,
                roughness,
                metallic,
                ior,
                transmission,
                emissive,
                volume,
                palette: slf.clone().unbind(),
            },
        )?;
        slf_brw.materials.push(material.clone_ref(slf.py()));
        Ok(material)
    }

    /// Return the number of materials in the palette.
    pub fn __len__(&self) -> usize {
        self.materials.len()
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.materials.iter().try_for_each(|color| visit.call(color))
    }

    fn __clear__(&mut self) {
        self.materials.clear();
    }
}

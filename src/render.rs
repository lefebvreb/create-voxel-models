use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::{PyResult, pyclass, pymethods};

/// A camera position for rendering, orbiting the framed subject at a fitted distance.
#[pyclass(get_all, from_py_object, frozen)]
#[derive(Copy, Clone)]
pub struct CameraAngle {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: Option<f64>,
}

#[pymethods]
impl CameraAngle {
    /// Define a camera angle.
    ///
    /// Args:
    ///     yaw: Angle around the vertical axis, in degrees.
    ///     pitch: Angle above the horizon, in degrees; clamped to just under ±90.
    ///     zoom: Magnification relative to the automatic framing; above 1.0 zooms in. The
    ///         subject is fitted to the frame when omitted.
    #[new]
    #[pyo3(signature = (yaw, pitch, *, zoom = None))]
    fn new(yaw: f64, pitch: f64, zoom: Option<f64>) -> PyResult<Self> {
        if !yaw.is_finite() {
            return Err(PyValueError::new_err("yaw must be finite"));
        }
        if !pitch.is_finite() {
            return Err(PyValueError::new_err("pitch must be finite"));
        }
        if let Some(zoom) = zoom
            && !zoom.is_finite()
        {
            return Err(PyValueError::new_err("zoom must be finite"));
        }
        Ok(Self { yaw, pitch, zoom })
    }
}

/// The output of a render.
#[pyclass(get_all, frozen)]
pub struct RenderOutput {
    /// Paths of the written PNG files, ordered by keyframe time then by camera angle.
    pub files: Vec<PathBuf>,
}

#[pymethods]
impl RenderOutput {
    /// Print all file paths one after the other, one per line.
    pub fn __str__(&self) -> String {
        self.to_string()
    }
}

impl Display for RenderOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if let Some((last, head)) = self.files.split_last() {
            for file in head {
                writeln!(f, "{}", file.display())?;
            }
            write!(f, "{}", last.display())?;
        }
        Ok(())
    }
}

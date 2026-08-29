use std::path::PathBuf;

use clap::Parser;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::types::PyAnyMethods;
use pyo3::{PyResult, Python, pyfunction};

use crate::tools::{self, RenderError};

#[derive(Parser)]
#[command(name = "voxels.preview")]
pub struct Args {
    /// Path to the .glb file to render.
    pub glb: PathBuf,

    /// Camera position as YAW,PITCH or YAW,PITCH,ZOOM (degrees). Repeatable; a single
    /// three-quarter view (45,25) is used when omitted entirely.
    #[arg(short = 'a', long = "angle", value_name = "YAW,PITCH[,ZOOM]", value_parser = parse_angle)]
    pub angles: Vec<Angle>,

    /// Time, in seconds, to sample --animation at. Repeatable; ignored without --animation.
    #[arg(short = 't', long = "time", value_name = "SECONDS")]
    pub times: Vec<f64>,

    /// Name of the animation to pose the scene with before rendering.
    #[arg(long = "anim")]
    pub anim: Option<String>,

    /// Only show nodes/meshes with this name (repeatable); everything else starts hidden.
    #[arg(short = 'i', long = "include", value_name = "NAME")]
    pub include: Vec<String>,

    /// Hide nodes/meshes with this name (repeatable).
    #[arg(short = 'e', long = "exclude", value_name = "NAME")]
    pub exclude: Vec<String>,

    /// Output directory for the rendered PNGs (defaults to a fresh temp directory).
    #[arg(short = 'o', long = "out")]
    pub out: Option<PathBuf>,
}

#[derive(Clone, Copy)]
pub struct Angle {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: Option<f64>,
}

fn parse_angle(s: &str) -> Result<Angle, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err("expected YAW,PITCH or YAW,PITCH,ZOOM".to_string());
    }
    let number = |raw: &str| raw.trim().parse::<f64>().map_err(|_| format!("{raw:?} is not a number"));
    Ok(Angle {
        yaw: number(parts[0])?,
        pitch: number(parts[1])?,
        zoom: parts.get(2).map(|z| number(z)).transpose()?,
    })
}

/// Entry point for `python -m voxels.preview`.
#[pyfunction]
pub fn _preview(py: Python<'_>) -> PyResult<()> {
    let mut argv: Vec<String> = py.import("sys")?.getattr("argv")?.extract()?;
    if let Some(argv0) = argv.first_mut() {
        *argv0 = "voxels.preview".to_string();
    }
    let args = Args::parse_from(argv);
    match tools::run_cli(args) {
        Ok(files) => {
            for file in files {
                println!("{}", file.display());
            }
            Ok(())
        }
        Err(RenderError::InvalidInput(message)) => Err(PyValueError::new_err(message)),
        Err(RenderError::Io(message)) => Err(PyRuntimeError::new_err(message)),
    }
}

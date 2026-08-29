use std::num::ParseFloatError;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;

use clap::Parser;
use pyo3::types::PyAnyMethods;
use pyo3::{PyResult, Python, pyfunction};
use thiserror::Error;

use crate::tools::preview_glb;

#[derive(Parser)]
#[command(name = "voxels.preview")]
pub struct Args {
    /// Path to the .glb file to render.
    pub glb: PathBuf,
    /// Camera positions (repeatable). A single three-quarter view (45,25) is used when omitted entirely.
    #[arg(short = 'a', long = "angle", value_name = "YAW,PITCH[,ZOOM]")]
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

#[derive(Error, Debug)]
pub enum ParseAngleError {
    #[error("invalid camera angle format, expected YAW,PITCH[,ZOOM]")]
    InvalidFormat,
    #[error("failed to parse number: {0}")]
    ParseFloatError(#[from] ParseFloatError),
}

#[derive(Clone, Copy)]
pub struct Angle {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: Option<f64>,
}

impl FromStr for Angle {
    type Err = ParseAngleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split(',');
        Ok(Self {
            yaw: split.next().ok_or(ParseAngleError::InvalidFormat)?.parse()?,
            pitch: split.next().ok_or(ParseAngleError::InvalidFormat)?.parse()?,
            zoom: split.next().map(|lit| lit.parse()).transpose()?,
        })
    }
}

#[derive(Error, Debug)]
#[error("{0}")]
pub struct PreviewError(pub String);

/// Entry point for `python -m voxels.preview`.
#[pyfunction]
pub fn _preview(py: Python<'_>) -> PyResult<()> {
    let mut argv = py.import("sys")?.getattr("argv")?.extract::<Vec<String>>()?;
    if let Some(argv0) = argv.first_mut() {
        *argv0 = "voxels.preview".to_string();
    }
    let args = Args::parse_from(argv);
    match preview_glb(args) {
        Ok(files) => {
            for file in files {
                println!("{}", file.display());
            }
            Ok(())
        }
        Err(err) => {
            eprintln!("error: {err}");
            exit(1);
        }
    }
}

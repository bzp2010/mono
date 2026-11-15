//! Discovers projects from ecosystem workspace signals.

use std::path::{Path, PathBuf};

use mono_core::{DiscoveredProject, Ecosystem};
use thiserror::Error;

mod cargo;
mod npm;
mod package_json;
mod pnpm;

/// Errors raised during discovery.
#[derive(Debug, Error)]
pub enum DiscoverError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cargo metadata failed: {stderr}")]
    CargoMetadata { stderr: String },
    #[error("invalid metadata output: {0}")]
    InvalidMetadata(#[source] serde_json::Error),
    #[error("workspace member {dir} has no package.json")]
    MissingManifest { dir: PathBuf },
    #[error("invalid manifest {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },
    #[error("invalid workspace pattern {pattern}: {message}")]
    InvalidPattern { pattern: String, message: String },
}

/// Runs discovery for each enabled ecosystem under `root`.
pub fn discover(
    root: &Path,
    ecosystems: &[Ecosystem],
) -> Result<Vec<DiscoveredProject>, DiscoverError> {
    let mut projects = Vec::new();
    for ecosystem in ecosystems {
        match ecosystem {
            Ecosystem::Cargo => projects.extend(cargo::discover_cargo(root)?),
            Ecosystem::Pnpm => projects.extend(pnpm::discover_pnpm(root)?),
            Ecosystem::Npm => projects.extend(npm::discover_npm(root)?),
        }
    }
    Ok(projects)
}

pub use cargo::discover_cargo;
pub use npm::discover_npm;
pub use pnpm::discover_pnpm;

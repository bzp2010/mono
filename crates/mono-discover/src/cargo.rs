//! Cargo workspace discovery through `cargo metadata`.

use std::path::Path;
use std::process::Command;

use mono_core::{DiscoveredProject, Ecosystem};

use crate::DiscoverError;

/// Returns the single project for a Cargo workspace rooted at `root`.
///
/// The whole workspace is one project. Crate-level path dependencies are not
/// recorded because nothing downstream consumes them.
pub fn discover_cargo(root: &Path) -> Result<Vec<DiscoveredProject>, DiscoverError> {
    let manifest = root.join("Cargo.toml");
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--manifest-path"])
        .arg(&manifest)
        .output()
        .map_err(|source| DiscoverError::Io { path: manifest.clone(), source })?;
    if !output.status.success() {
        return Err(DiscoverError::CargoMetadata {
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(DiscoverError::InvalidMetadata)?;
    let workspace_root = meta["workspace_root"]
        .as_str()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| DiscoverError::CargoMetadata {
            stderr: "cargo metadata output has no workspace_root".to_string(),
        })?;
    let name = workspace_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(vec![DiscoveredProject {
        name,
        root: workspace_root,
        ecosystem: Ecosystem::Cargo,
        dependencies: Vec::new(),
    }])
}

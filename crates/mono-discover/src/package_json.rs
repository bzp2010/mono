//! Shared helpers for Node.js workspaces: manifests and member expansion.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::DiscoverError;

/// The fields of `package.json` that discovery needs.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PackageJson {
    pub name: Option<String>,
    pub version: Option<String>,
    /// Raw `workspaces` field: an array of patterns, or `{ packages: [...] }`.
    pub workspaces: Option<serde_json::Value>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: BTreeMap<String, String>,
    #[serde(rename = "peerDependencies", default)]
    pub peer_dependencies: BTreeMap<String, String>,
    #[serde(rename = "optionalDependencies", default)]
    pub optional_dependencies: BTreeMap<String, String>,
}

impl PackageJson {
    /// Every declared dependency as `(name, spec)` pairs.
    pub(crate) fn all_dependencies(&self) -> impl Iterator<Item = (&String, &String)> {
        self.dependencies
            .iter()
            .chain(&self.dev_dependencies)
            .chain(&self.peer_dependencies)
            .chain(&self.optional_dependencies)
    }
}

/// Reads `dir/package.json`. A missing file is an error, not a skip.
pub(crate) fn read_package_json(dir: &Path) -> Result<PackageJson, DiscoverError> {
    let path = dir.join("package.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(DiscoverError::MissingManifest { dir: dir.to_path_buf() })
        }
        Err(source) => return Err(DiscoverError::Io { path, source }),
    };
    serde_json::from_str(&text).map_err(|e| DiscoverError::InvalidManifest {
        path,
        message: e.to_string(),
    })
}

/// Expands workspace glob patterns into member directories.
///
/// Every matched directory must contain a `package.json`, otherwise the
/// result is `MissingManifest`.
pub(crate) fn expand_members(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, DiscoverError> {
    let mut dirs = Vec::new();
    for pattern in patterns {
        let full = root.join(pattern);
        let entries = glob::glob(&full.to_string_lossy()).map_err(|e| {
            DiscoverError::InvalidPattern { pattern: pattern.clone(), message: e.to_string() }
        })?;
        for entry in entries {
            let path = entry.map_err(|e| DiscoverError::Io {
                path: e.path().to_path_buf(),
                source: e.into(),
            })?;
            if !path.is_dir() {
                continue;
            }
            if !path.join("package.json").is_file() {
                return Err(DiscoverError::MissingManifest { dir: path });
            }
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs.dedup();
    Ok(dirs)
}

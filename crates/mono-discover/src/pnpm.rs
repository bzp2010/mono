//! pnpm workspace discovery from `pnpm-workspace.yaml`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mono_core::{DiscoveredProject, Ecosystem};
use serde::Deserialize;

use crate::package_json::{expand_members, read_package_json, PackageJson};
use crate::DiscoverError;

#[derive(Debug, Deserialize)]
struct PnpmWorkspace {
    #[serde(default)]
    packages: Vec<String>,
}

/// Discovers pnpm packages: the root package (when the root has a
/// `package.json`) and every member. Each is a project of its own. A
/// dependency is an edge only when its spec uses the `workspace:` protocol and
/// names another package.
pub fn discover_pnpm(root: &Path) -> Result<Vec<DiscoveredProject>, DiscoverError> {
    let path = root.join("pnpm-workspace.yaml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| DiscoverError::Io { path: path.clone(), source })?;
    let ws: PnpmWorkspace = serde_yaml_ng::from_str(&text).map_err(|e| {
        DiscoverError::InvalidManifest { path: path.clone(), message: e.to_string() }
    })?;

    let mut packages: Vec<(PathBuf, PackageJson)> = Vec::new();
    if root.join("package.json").is_file() {
        packages.push((root.to_path_buf(), read_package_json(root)?));
    }
    for dir in expand_members(root, &ws.packages)? {
        if dir == root {
            continue;
        }
        packages.push((dir.clone(), read_package_json(&dir)?));
    }

    let mut named = Vec::new();
    for (dir, pkg) in packages {
        let name = match pkg.name.clone() {
            Some(name) => name,
            None if dir == root => dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => {
                return Err(DiscoverError::InvalidManifest {
                    path: dir.join("package.json"),
                    message: "missing \"name\"".to_string(),
                })
            }
        };
        named.push((dir, name, pkg));
    }
    let names: BTreeSet<String> = named.iter().map(|(_, n, _)| n.clone()).collect();

    let mut projects = Vec::new();
    for (dir, name, pkg) in &named {
        let dependencies = pkg
            .all_dependencies()
            .filter(|(dep, spec)| spec.starts_with("workspace:") && names.contains(*dep))
            .map(|(dep, _)| dep.clone())
            .collect();
        projects.push(DiscoveredProject {
            name: name.clone(),
            root: dir.clone(),
            ecosystem: Ecosystem::Pnpm,
            dependencies,
        });
    }
    Ok(projects)
}

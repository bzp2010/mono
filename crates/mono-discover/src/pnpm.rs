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

/// Discovers pnpm packages: the root package (when the root has a `package.json`)
/// and every member. Each is a project of its own. A dependency is an edge only
/// when its spec uses the `workspace:` protocol and names another package.
///
/// A root without a `name` is discovered unnamed. The graph reports it unless
/// `project.json` names it.
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

    for (dir, pkg) in &packages {
        if dir != root && pkg.name.is_none() {
            return Err(DiscoverError::InvalidManifest {
                path: dir.join("package.json"),
                message: "missing \"name\"".to_string(),
            });
        }
    }
    let names: BTreeSet<String> = packages.iter().filter_map(|(_, pkg)| pkg.name.clone()).collect();

    let mut projects = Vec::new();
    for (dir, pkg) in &packages {
        let dependencies = pkg
            .all_dependencies()
            .filter_map(|(dep, spec)| workspace_target(dep, spec))
            .filter(|target| names.contains(target))
            .collect();
        projects.push(DiscoveredProject {
            name: pkg.name.clone(),
            package_name: pkg.name.clone(),
            root: dir.clone(),
            ecosystem: Ecosystem::Pnpm,
            dependencies,
            version: pkg.version.clone(),
        });
    }
    Ok(projects)
}

/// The package a `workspace:` spec refers to. `workspace:*` and `workspace:^1.0.0`
/// refer to the dependency's own name, and so do path references such as
/// `workspace:../foo`. `workspace:foo@*` is an alias, so it refers to `foo`, and
/// a scoped name such as `@scope/foo@*` keeps its scope.
fn workspace_target(dep: &str, spec: &str) -> Option<String> {
    let rest = spec.strip_prefix("workspace:")?;
    // A path reference names the package by the dependency key, not by the path.
    if rest.starts_with('.') || rest.starts_with('/') {
        return Some(dep.to_string());
    }
    match rest.rfind('@') {
        Some(i) if i > 0 => Some(rest[..i].to_string()),
        _ => Some(dep.to_string()),
    }
}

//! pnpm workspace discovery from `pnpm-workspace.yaml`.

use std::collections::BTreeSet;
use std::path::Path;

use mono_core::{DiscoveredProject, Ecosystem};
use serde::Deserialize;

use crate::package_json::{expand_members, read_package_json};
use crate::DiscoverError;

#[derive(Debug, Deserialize)]
struct PnpmWorkspace {
    #[serde(default)]
    packages: Vec<String>,
}

/// Discovers pnpm workspace members. A dependency is an edge only when its
/// spec uses the `workspace:` protocol and names another member.
pub fn discover_pnpm(root: &Path) -> Result<Vec<DiscoveredProject>, DiscoverError> {
    let path = root.join("pnpm-workspace.yaml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| DiscoverError::Io { path: path.clone(), source })?;
    let ws: PnpmWorkspace = serde_yaml_ng::from_str(&text).map_err(|e| {
        DiscoverError::InvalidManifest { path: path.clone(), message: e.to_string() }
    })?;

    let members: Vec<_> = expand_members(root, &ws.packages)?
        .into_iter()
        .map(|dir| read_package_json(&dir).map(|pkg| (dir, pkg)))
        .collect::<Result<_, _>>()?;
    let names: BTreeSet<String> = members.iter().filter_map(|(_, p)| p.name.clone()).collect();

    let mut projects = Vec::new();
    for (dir, pkg) in &members {
        let name = pkg.name.clone().ok_or_else(|| DiscoverError::InvalidManifest {
            path: dir.join("package.json"),
            message: "missing \"name\"".to_string(),
        })?;
        let dependencies = pkg
            .all_dependencies()
            .filter(|(dep, spec)| spec.starts_with("workspace:") && names.contains(*dep))
            .map(|(dep, _)| dep.clone())
            .collect();
        projects.push(DiscoveredProject {
            name,
            root: dir.clone(),
            ecosystem: Ecosystem::Pnpm,
            dependencies,
        });
    }
    Ok(projects)
}

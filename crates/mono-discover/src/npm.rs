//! npm and yarn workspace discovery from the root `package.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mono_core::{DiscoveredProject, Ecosystem};

use crate::package_json::{expand_members, read_package_json};
use crate::DiscoverError;

/// Discovers npm or yarn workspace members.
///
/// A dependency is an edge when its spec is exactly a member's version, or a
/// `file:` path that resolves to the member's directory. Other specs are not
/// workspace edges.
pub fn discover_npm(root: &Path) -> Result<Vec<DiscoveredProject>, DiscoverError> {
    let root_pkg = read_package_json(root)?;
    let patterns = workspace_patterns(&root_pkg);

    let members: Vec<_> = expand_members(root, &patterns)?
        .into_iter()
        .map(|dir| read_package_json(&dir).map(|pkg| (dir, pkg)))
        .collect::<Result<_, _>>()?;
    let by_name: BTreeMap<String, (PathBuf, Option<String>)> = members
        .iter()
        .filter_map(|(dir, pkg)| {
            pkg.name.clone().map(|n| (n, (dir.clone(), pkg.version.clone())))
        })
        .collect();

    let mut projects = Vec::new();
    for (dir, pkg) in &members {
        let name = pkg.name.clone().ok_or_else(|| DiscoverError::InvalidManifest {
            path: dir.join("package.json"),
            message: "missing \"name\"".to_string(),
        })?;
        let mut dependencies = Vec::new();
        for (dep, spec) in pkg.all_dependencies() {
            let Some((target_dir, target_version)) = by_name.get(dep) else { continue };
            let by_version = target_version.as_deref() == Some(spec.as_str());
            let by_path = spec
                .strip_prefix("file:")
                .map(|rel| dir.join(rel))
                .and_then(|p| p.canonicalize().ok())
                .zip(target_dir.canonicalize().ok())
                .is_some_and(|(a, b)| a == b);
            if by_version || by_path {
                dependencies.push(dep.clone());
            }
        }
        projects.push(DiscoveredProject {
            name: Some(name.clone()),
            package_name: Some(name),
            root: dir.clone(),
            ecosystem: Ecosystem::Npm,
            dependencies,
            version: pkg.version.clone(),
        });
    }
    Ok(projects)
}

/// Reads the `workspaces` field, in array or `{ packages }` form.
fn workspace_patterns(root_pkg: &crate::package_json::PackageJson) -> Vec<String> {
    let raw = match &root_pkg.workspaces {
        Some(serde_json::Value::Array(items)) => items.clone(),
        Some(serde_json::Value::Object(obj)) => {
            obj.get("packages").and_then(|p| p.as_array()).cloned().unwrap_or_default()
        }
        _ => Vec::new(),
    };
    raw.into_iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
}

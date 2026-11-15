//! Affected projects: the projects a set of changed files touches, and everything
//! that depends on them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::ProjectGraph;

/// The projects affected by changes to `changed` files.
///
/// A file belongs to the project whose root is its most specific ancestor, that is
/// the one with the most path components. Files outside every project belong to
/// none. Each owning project is affected, and so is every project that depends on
/// it, directly or through other projects.
///
/// Paths must be absolute, in the same form as the project roots.
pub fn affected(graph: &ProjectGraph, changed: &[PathBuf]) -> BTreeSet<String> {
    let mut direct: BTreeSet<String> = BTreeSet::new();
    for file in changed {
        direct.extend(owners_of(graph, file));
    }

    // Reverse edges: from a project to the projects that depend on it.
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for project in graph.projects() {
        for dep in graph.deps_of(&project.name) {
            dependents.entry(dep.clone()).or_default().push(project.name.clone());
        }
    }

    let mut affected = direct.clone();
    let mut queue: Vec<String> = direct.into_iter().collect();
    while let Some(name) = queue.pop() {
        for dependent in dependents.get(&name).into_iter().flatten() {
            if affected.insert(dependent.clone()) {
                queue.push(dependent.clone());
            }
        }
    }
    affected
}

/// The projects that own `file`: every project whose root is the most specific
/// ancestor of it. Projects that share that root all own the file.
fn owners_of(graph: &ProjectGraph, file: &Path) -> Vec<String> {
    let containing: Vec<&crate::Project> =
        graph.projects().filter(|p| file.starts_with(&p.root)).collect();
    let Some(depth) = containing.iter().map(|p| p.root.components().count()).max() else {
        return Vec::new();
    };
    containing
        .into_iter()
        .filter(|p| p.root.components().count() == depth)
        .map(|p| p.name.clone())
        .collect()
}

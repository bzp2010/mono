//! Affected projects: the projects a set of changed files touches, and everything
//! that depends on them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mono_core::Ecosystem;

use crate::ProjectGraph;

/// Files at the workspace root that are inputs of every project of an ecosystem.
/// A change to one of them can change every build of that ecosystem, so it marks
/// all of its projects affected. Other ecosystems are not affected.
const ECOSYSTEM_INPUTS: &[(Ecosystem, &str)] = &[
    (Ecosystem::Cargo, "Cargo.lock"),
    (Ecosystem::Cargo, "rust-toolchain"),
    (Ecosystem::Cargo, "rust-toolchain.toml"),
    (Ecosystem::Pnpm, "pnpm-lock.yaml"),
    (Ecosystem::Npm, "package-lock.json"),
    (Ecosystem::Npm, "npm-shrinkwrap.json"),
    (Ecosystem::Npm, "yarn.lock"),
];

/// Files at the workspace root that are inputs of every project, whatever its
/// ecosystem.
const WORKSPACE_INPUTS: &[&str] = &["mono.json"];

/// The workspace-level files that can change the outputs of projects of `ecosystem`:
/// its lockfiles and toolchain files, and `mono.json`, whether or not they exist.
pub fn workspace_input_files(workspace_root: &Path, ecosystem: Ecosystem) -> Vec<PathBuf> {
    let mut names: Vec<&str> = WORKSPACE_INPUTS.to_vec();
    names.extend(
        ECOSYSTEM_INPUTS
            .iter()
            .filter(|(e, _)| *e == ecosystem)
            .map(|(_, name)| *name),
    );
    names.into_iter().map(|n| workspace_root.join(n)).collect()
}

/// The projects affected by changes to `changed` files.
///
/// A file belongs to the projects whose root is its most specific ancestor. Files
/// outside every project belong to none, unless they are a workspace input:
///
/// - a file in `ECOSYSTEM_INPUTS` at the workspace root affects every project of
///   its ecosystem;
/// - a file in `WORKSPACE_INPUTS` at the workspace root affects every project.
///
/// Each affected project is affected, and so is every project that depends on it,
/// directly or through other projects. Paths must be absolute, in the same form as
/// the project roots.
pub fn affected(graph: &ProjectGraph, workspace_root: &Path, changed: &[PathBuf]) -> BTreeSet<String> {
    let mut direct: BTreeSet<String> = BTreeSet::new();
    for file in changed {
        if is_workspace_input(workspace_root, file) {
            // A workspace input belongs to no project, even when a project's root
            // contains it. Only the global rule decides what it affects.
            direct.extend(global_affected(graph, workspace_root, file));
        } else {
            direct.extend(owners_of(graph, file));
        }
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

/// The name of `file` if it is a workspace input: a known file directly under the
/// workspace root.
fn workspace_input_name<'a>(workspace_root: &Path, file: &'a Path) -> Option<&'a str> {
    if file.parent() != Some(workspace_root) {
        return None;
    }
    let name = file.file_name()?.to_str()?;
    let known = WORKSPACE_INPUTS.contains(&name)
        || ECOSYSTEM_INPUTS.iter().any(|(_, input)| *input == name);
    known.then_some(name)
}

fn is_workspace_input(workspace_root: &Path, file: &Path) -> bool {
    workspace_input_name(workspace_root, file).is_some()
}

/// The projects a workspace input affects, if `file` is one.
fn global_affected(graph: &ProjectGraph, workspace_root: &Path, file: &Path) -> Vec<String> {
    let Some(name) = workspace_input_name(workspace_root, file) else {
        return Vec::new();
    };
    if WORKSPACE_INPUTS.contains(&name) {
        return graph.projects().map(|p| p.name.clone()).collect();
    }
    let ecosystems: Vec<Ecosystem> = ECOSYSTEM_INPUTS
        .iter()
        .filter(|(_, input)| *input == name)
        .map(|(ecosystem, _)| *ecosystem)
        .collect();
    graph
        .projects()
        .filter(|p| ecosystems.contains(&p.ecosystem))
        .map(|p| p.name.clone())
        .collect()
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

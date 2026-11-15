//! Project graph: merged dependencies, validation, and topological order.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use mono_core::{DiscoveredProject, Ecosystem, ProjectConfig, TargetConfig};
use thiserror::Error;

/// One project in the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub root: PathBuf,
    pub ecosystem: Ecosystem,
    pub targets: BTreeMap<String, TargetConfig>,
}

/// Errors raised while building the graph.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphError {
    #[error("project {from} depends on unknown project {to}")]
    UnknownDependency { from: String, to: String },
    #[error("dependency cycle: {}", .path.join(" -> "))]
    Cycle { path: Vec<String> },
    #[error("duplicate project name {name}")]
    DuplicateName { name: String },
    #[error("project at {} has no name. Add a name to its project.json, or to its package.json", root.display())]
    UnnamedProject { root: PathBuf },
    #[error("project {project} depends on target {target}, which it does not define")]
    MissingTarget { project: String, target: String },
    #[error("task cycle: {}", .path.join(" -> "))]
    TaskCycle { path: Vec<String> },
}

/// Validated project graph with dependencies merged and ordered.
#[derive(Debug, Clone)]
pub struct ProjectGraph {
    projects: BTreeMap<String, Project>,
    deps: BTreeMap<String, Vec<String>>,
    order: Vec<String>,
}

impl ProjectGraph {
    pub fn projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.values()
    }

    pub fn project(&self, name: &str) -> Option<&Project> {
        self.projects.get(name)
    }

    /// Direct dependencies of a project, inferred first, then explicit.
    pub fn deps_of(&self, name: &str) -> &[String] {
        self.deps.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Projects ordered so that every project appears after its dependencies.
    pub fn topo_order(&self) -> Vec<&str> {
        self.order.iter().map(String::as_str).collect()
    }
}

/// Builds the graph from discovered projects and explicit `project.json` configs.
///
/// `configs` is keyed by project name, which the caller has already resolved from
/// `project.json` or the ecosystem. A discovered project without a name is an error.
///
/// Dependencies from discovery are package names. Each one resolves to the project
/// that has that package name, or else to the project of that name. An explicit
/// dependency in `project.json` names a project directly.
pub fn build(
    discovered: Vec<DiscoveredProject>,
    configs: &BTreeMap<String, ProjectConfig>,
) -> Result<ProjectGraph, GraphError> {
    let mut names: Vec<String> = Vec::new();
    for d in &discovered {
        let name = d
            .name
            .clone()
            .ok_or_else(|| GraphError::UnnamedProject { root: d.root.clone() })?;
        if names.contains(&name) {
            return Err(GraphError::DuplicateName { name });
        }
        names.push(name);
    }

    // Package name to project name, for resolving discovered dependencies.
    let by_package: BTreeMap<String, String> = discovered
        .iter()
        .zip(&names)
        .filter_map(|(d, name)| d.package_name.clone().map(|p| (p, name.clone())))
        .collect();

    let mut projects = BTreeMap::new();
    let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (d, name) in discovered.iter().zip(&names) {
        let mut merged: Vec<String> = Vec::new();
        for dep in &d.dependencies {
            let target = by_package
                .get(dep)
                .cloned()
                .or_else(|| names.iter().find(|n| *n == dep).cloned())
                .ok_or_else(|| GraphError::UnknownDependency {
                    from: name.clone(),
                    to: dep.clone(),
                })?;
            if !merged.contains(&target) {
                merged.push(target);
            }
        }
        if let Some(cfg) = configs.get(name) {
            for dep in &cfg.dependencies {
                if !names.contains(dep) {
                    return Err(GraphError::UnknownDependency { from: name.clone(), to: dep.clone() });
                }
                if !merged.contains(dep) {
                    merged.push(dep.clone());
                }
            }
        }
        deps.insert(name.clone(), merged);
        projects.insert(
            name.clone(),
            Project {
                name: name.clone(),
                root: d.root.clone(),
                ecosystem: d.ecosystem,
                targets: configs.get(name).map(|c| c.targets.clone()).unwrap_or_default(),
            },
        );
    }

    let order = topo_sort(&deps)?;
    Ok(ProjectGraph { projects, deps, order })
}

/// Repeatedly emits every project whose dependencies are all emitted. When no
/// project is ready but some remain, they contain a cycle.
fn topo_sort(deps: &BTreeMap<String, Vec<String>>) -> Result<Vec<String>, GraphError> {
    let mut done: BTreeSet<&str> = BTreeSet::new();
    let mut order = Vec::new();
    while done.len() < deps.len() {
        let ready: Vec<&str> = deps
            .iter()
            .filter(|(name, list)| {
                !done.contains(name.as_str()) && list.iter().all(|d| done.contains(d.as_str()))
            })
            .map(|(name, _)| name.as_str())
            .collect();
        if ready.is_empty() {
            let unresolved: BTreeSet<&str> = deps
                .keys()
                .map(String::as_str)
                .filter(|n| !done.contains(n))
                .collect();
            return Err(GraphError::Cycle { path: find_cycle(deps, &unresolved) });
        }
        for name in ready {
            done.insert(name);
            order.push(name.to_string());
        }
    }
    Ok(order)
}

/// Walks dependency edges among unresolved nodes until a node repeats,
/// then returns the closed loop.
fn find_cycle(deps: &BTreeMap<String, Vec<String>>, unresolved: &BTreeSet<&str>) -> Vec<String> {
    let start = *unresolved.iter().next().expect("cycle implies unresolved nodes");
    let mut path: Vec<String> = Vec::new();
    let mut cur = start.to_string();
    loop {
        if let Some(i) = path.iter().position(|p| *p == cur) {
            let mut cycle = path[i..].to_vec();
            cycle.push(cur);
            return cycle;
        }
        path.push(cur.clone());
        let next = deps
            .get(&cur)
            .and_then(|list| list.iter().find(|d| unresolved.contains(d.as_str())))
            .expect("every unresolved node has an unresolved dependency");
        cur = next.clone();
    }
}

use std::collections::BTreeMap;
use std::path::PathBuf;

use mono_core::{DiscoveredProject, Ecosystem, ProjectConfig};
use mono_discover::discover;
use mono_graph::{build, GraphError};

fn proj(name: &str, deps: &[&str]) -> DiscoveredProject {
    DiscoveredProject {
        name: name.to_string(),
        root: PathBuf::from(format!("/ws/{name}")),
        ecosystem: Ecosystem::Npm,
        dependencies: deps.iter().map(|d| d.to_string()).collect(),
    }
}

fn explicit(name: &str, deps: &[&str]) -> (String, ProjectConfig) {
    let cfg = ProjectConfig {
        dependencies: deps.iter().map(|d| d.to_string()).collect(),
        ..Default::default()
    };
    (name.to_string(), cfg)
}

#[test]
fn inferred_and_explicit_dependencies_are_merged() {
    let projects = vec![proj("a", &["b"]), proj("b", &[]), proj("c", &[])];
    let configs: BTreeMap<_, _> = [explicit("a", &["c"])].into_iter().collect();
    let graph = build(projects, &configs).unwrap();
    let mut deps = graph.deps_of("a").to_vec();
    deps.sort();
    assert_eq!(deps, vec!["b".to_string(), "c".to_string()]);
}

#[test]
fn unknown_explicit_dependency_is_error() {
    let projects = vec![proj("a", &[])];
    let configs: BTreeMap<_, _> = [explicit("a", &["ghost"])].into_iter().collect();
    match build(projects, &configs).unwrap_err() {
        GraphError::UnknownDependency { from, to } => {
            assert_eq!((from.as_str(), to.as_str()), ("a", "ghost"));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn duplicate_project_names_are_error() {
    let projects = vec![proj("a", &[]), proj("a", &[])];
    let err = build(projects, &BTreeMap::new()).unwrap_err();
    assert!(matches!(err, GraphError::DuplicateName { .. }));
}

#[test]
fn cycle_reports_closed_path() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cycle");
    let projects = discover(&root, &[Ecosystem::Pnpm]).unwrap();
    match build(projects, &BTreeMap::new()).unwrap_err() {
        GraphError::Cycle { path } => assert_eq!(path.first(), path.last()),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn topo_order_puts_dependencies_first() {
    let projects = vec![proj("app", &["lib"]), proj("lib", &["core"]), proj("core", &[])];
    let graph = build(projects, &BTreeMap::new()).unwrap();
    let order = graph.topo_order();
    let pos = |n: &str| order.iter().position(|x| *x == n).unwrap();
    assert!(pos("core") < pos("lib") && pos("lib") < pos("app"));
}

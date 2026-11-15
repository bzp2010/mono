use std::collections::BTreeMap;
use std::path::PathBuf;

use mono_core::{DiscoveredProject, Ecosystem, ProjectConfig, TargetConfig};
use mono_graph::{build, plan, GraphError, ProjectGraph, TaskId};

fn target(command: &str, depends_on: &[&str]) -> TargetConfig {
    TargetConfig {
        command: command.to_string(),
        inputs: vec![],
        outputs: vec![],
        depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
    }
}

/// `b` depends on `a`. Both define `build`.
fn two_projects() -> ProjectGraph {
    let discovered = vec![
        DiscoveredProject {
            name: Some("a".into()),
            package_name: None,
            root: PathBuf::from("/ws/a"),
            ecosystem: Ecosystem::Npm,
            dependencies: vec![],
        },
        DiscoveredProject {
            name: Some("b".into()),
            package_name: None,
            root: PathBuf::from("/ws/b"),
            ecosystem: Ecosystem::Npm,
            dependencies: vec!["a".into()],
        },
    ];
    let mut configs = BTreeMap::new();
    for name in ["a", "b"] {
        let cfg = ProjectConfig {
            targets: [("build".to_string(), target("true", &["^build"]))].into_iter().collect(),
            ..Default::default()
        };
        configs.insert(name.to_string(), cfg);
    }
    build(discovered, &configs).unwrap()
}

fn ids(tasks: &[TaskId]) -> Vec<(String, String)> {
    tasks.iter().map(|t| (t.project.clone(), t.target.clone())).collect()
}

#[test]
fn dependencies_run_first() {
    let graph = two_projects();
    let tasks = plan(&graph, "build").unwrap();
    assert_eq!(
        ids(&tasks),
        vec![("a".into(), "build".into()), ("b".into(), "build".into())]
    );
}

#[test]
fn projects_without_the_target_are_skipped() {
    let graph = two_projects();
    assert!(plan(&graph, "lint").unwrap().is_empty());
}

#[test]
fn missing_dependency_target_is_skipped() {
    // `a` has no `test`, so `^test` on `b` contributes nothing.
    let discovered = vec![
        DiscoveredProject {
            name: Some("a".into()),
            package_name: None,
            root: PathBuf::from("/ws/a"),
            ecosystem: Ecosystem::Npm,
            dependencies: vec![],
        },
        DiscoveredProject {
            name: Some("b".into()),
            package_name: None,
            root: PathBuf::from("/ws/b"),
            ecosystem: Ecosystem::Npm,
            dependencies: vec!["a".into()],
        },
    ];
    let configs: BTreeMap<_, _> = [(
        "b".to_string(),
        ProjectConfig {
            targets: [("test".to_string(), target("true", &["^test"]))].into_iter().collect(),
            ..Default::default()
        },
    )]
    .into_iter()
    .collect();
    let graph = build(discovered, &configs).unwrap();
    assert_eq!(ids(&plan(&graph, "test").unwrap()), vec![("b".into(), "test".into())]);
}

#[test]
fn same_project_dependency_on_missing_target_is_error() {
    let discovered = vec![DiscoveredProject {
        name: Some("a".into()),
        package_name: None,
        root: PathBuf::from("/ws/a"),
        ecosystem: Ecosystem::Npm,
        dependencies: vec![],
    }];
    let configs: BTreeMap<_, _> = [(
        "a".to_string(),
        ProjectConfig {
            targets: [("build".to_string(), target("true", &["gen"]))].into_iter().collect(),
            ..Default::default()
        },
    )]
    .into_iter()
    .collect();
    let graph = build(discovered, &configs).unwrap();
    assert!(matches!(
        plan(&graph, "build").unwrap_err(),
        GraphError::MissingTarget { .. }
    ));
}

#[test]
fn task_level_cycle_is_error() {
    let discovered = vec![DiscoveredProject {
        name: Some("a".into()),
        package_name: None,
        root: PathBuf::from("/ws/a"),
        ecosystem: Ecosystem::Npm,
        dependencies: vec![],
    }];
    let configs: BTreeMap<_, _> = [(
        "a".to_string(),
        ProjectConfig {
            targets: [
                ("build".to_string(), target("true", &["test"])),
                ("test".to_string(), target("true", &["build"])),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        },
    )]
    .into_iter()
    .collect();
    let graph = build(discovered, &configs).unwrap();
    assert!(matches!(plan(&graph, "build").unwrap_err(), GraphError::TaskCycle { .. }));
}

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mono_cache::LocalBackend;
use mono_core::{DiscoveredProject, Ecosystem, ProjectConfig, TargetConfig};
use mono_graph::{build, plan, ProjectGraph};
use mono_task::{run_plan_in_workspace, TaskOutcome};

/// Two Cargo projects under one workspace. `p1` rewrites the workspace lockfile from
/// `A` to `B`, as a dependency install would. `p2` has an output and must be keyed on
/// the lockfile as it is when `p2` runs, not as it was when the plan started.
fn graph(ws: &Path) -> ProjectGraph {
    let target = |command: String, outputs: Vec<String>| TargetConfig {
        command,
        inputs: vec![],
        outputs,
        depends_on: vec![],
    };
    let lock = ws.join("Cargo.lock");
    let discovered = vec![
        DiscoveredProject {
            name: Some("p1".into()),
            package_name: None,
            root: ws.join("p1"),
            ecosystem: Ecosystem::Cargo,
            dependencies: vec![],
        },
        DiscoveredProject {
            name: Some("p2".into()),
            package_name: None,
            root: ws.join("p2"),
            ecosystem: Ecosystem::Cargo,
            dependencies: vec![],
        },
    ];
    let mut configs = BTreeMap::new();
    configs.insert(
        "p1".to_string(),
        ProjectConfig {
            name: None,
            dependencies: vec![],
            targets: [(
                "build".to_string(),
                target(format!("echo B > '{}'", lock.display()), vec![]),
            )]
            .into_iter()
            .collect(),
        },
    );
    configs.insert(
        "p2".to_string(),
        ProjectConfig {
            name: None,
            dependencies: vec![],
            targets: [(
                "build".to_string(),
                target(
                    format!("echo built >> '{}' && echo x > out.txt", ws.join("log.txt").display()),
                    vec!["out.txt".into()],
                ),
            )]
            .into_iter()
            .collect(),
        },
    );
    build(discovered, &configs).unwrap()
}

#[test]
fn a_task_is_keyed_on_the_lockfile_as_it_is_when_it_runs() {
    let ws = tempfile::tempdir().unwrap();
    let ws_path = ws.path();
    fs::create_dir_all(ws_path.join("p1")).unwrap();
    fs::create_dir_all(ws_path.join("p2")).unwrap();
    fs::write(ws_path.join("Cargo.lock"), "A").unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());

    // First run: p1 turns the lockfile into B, then p2 runs and is stored under B.
    let g = graph(ws_path);
    let tasks = plan(&g, "build").unwrap();
    run_plan_in_workspace(ws_path, &g, &tasks, Some(&backend)).unwrap();

    // Second run: the lockfile is already B, so p2 must hit the entry it stored.
    fs::write(ws_path.join("out.txt"), "stale").unwrap();
    let results = run_plan_in_workspace(ws_path, &g, &tasks, Some(&backend)).unwrap();
    let p2 = results.iter().find(|(t, _)| t.project == "p2").unwrap();
    assert_eq!(p2.1, TaskOutcome::Cached);
    assert_eq!(fs::read_to_string(ws_path.join("log.txt")).unwrap().lines().count(), 1);
}

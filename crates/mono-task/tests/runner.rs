use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mono_core::{DiscoveredProject, Ecosystem, ProjectConfig, TargetConfig};
use mono_graph::{build, plan, ProjectGraph};
use mono_task::{run_plan, run_task, TaskOutcome};

fn target(command: &str, depends_on: &[&str]) -> TargetConfig {
    TargetConfig {
        command: command.to_string(),
        inputs: vec![],
        outputs: vec![],
        depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
        cache_dirs: vec![],
    }
}

/// `b` depends on `a`. Each defines `build` with the given commands.
fn graph_in(root: &Path, a_cmd: &str, b_cmd: &str) -> ProjectGraph {
    let dir_a = root.join("a");
    let dir_b = root.join("b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    let discovered = vec![
        DiscoveredProject {
            name: "a".into(),
            root: dir_a,
            ecosystem: Ecosystem::Npm,
            dependencies: vec![],
        },
        DiscoveredProject {
            name: "b".into(),
            root: dir_b,
            ecosystem: Ecosystem::Npm,
            dependencies: vec!["a".into()],
        },
    ];
    let mut configs = BTreeMap::new();
    configs.insert(
        "a".to_string(),
        ProjectConfig {
            targets: [("build".to_string(), target(a_cmd, &[]))].into_iter().collect(),
            ..Default::default()
        },
    );
    configs.insert(
        "b".to_string(),
        ProjectConfig {
            targets: [("build".to_string(), target(b_cmd, &["^build"]))].into_iter().collect(),
            ..Default::default()
        },
    );
    build(discovered, &configs).unwrap()
}

fn log_path(root: &Path) -> PathBuf {
    root.join("log.txt")
}

#[test]
fn successful_command_reports_success() {
    let dir = tempfile::tempdir().unwrap();
    let task = mono_graph::TaskId { project: "a".into(), target: "build".into() };
    let outcome = run_task(&task, dir.path(), "true").unwrap();
    assert_eq!(outcome, TaskOutcome::Success);
}

#[test]
fn failing_command_reports_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let task = mono_graph::TaskId { project: "a".into(), target: "build".into() };
    let outcome = run_task(&task, dir.path(), "exit 3").unwrap();
    assert_eq!(outcome, TaskOutcome::Failed { code: 3 });
}

#[test]
fn plan_runs_in_order_and_stops_at_first_failure() {
    let dir = tempfile::tempdir().unwrap();
    let log = log_path(dir.path());
    let a_cmd = format!("echo a >> '{}' && exit 3", log.display());
    let b_cmd = format!("echo b >> '{}'", log.display());
    let graph = graph_in(dir.path(), &a_cmd, &b_cmd);
    let tasks = plan(&graph, "build").unwrap();

    let results = run_plan(&graph, &tasks, None).unwrap();

    assert_eq!(results.len(), 1, "execution must stop after the failing task");
    assert_eq!(results[0].1, TaskOutcome::Failed { code: 3 });
    assert_eq!(fs::read_to_string(&log).unwrap(), "a\n");
}

#[test]
fn plan_runs_every_task_when_all_succeed() {
    let dir = tempfile::tempdir().unwrap();
    let log = log_path(dir.path());
    let a_cmd = format!("echo a >> '{}'", log.display());
    let b_cmd = format!("echo b >> '{}'", log.display());
    let graph = graph_in(dir.path(), &a_cmd, &b_cmd);
    let tasks = plan(&graph, "build").unwrap();

    let results = run_plan(&graph, &tasks, None).unwrap();

    assert!(results.iter().all(|(_, o)| *o == TaskOutcome::Success));
    assert_eq!(fs::read_to_string(&log).unwrap(), "a\nb\n");
}

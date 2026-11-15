use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mono_cache::LocalBackend;
use mono_core::{DiscoveredProject, Ecosystem, ProjectConfig, TargetConfig};
use mono_graph::{build, plan, ProjectGraph};
use mono_task::{run_plan, TaskOutcome};

/// One project `a` whose `build` appends to `log.txt` and writes `dist/out.txt`.
fn graph_with_output(root: &Path) -> ProjectGraph {
    let dir = root.join("a");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/input.txt"), "v1").unwrap();
    let discovered = vec![DiscoveredProject {
        name: "a".into(),
        root: dir.clone(),
        ecosystem: Ecosystem::Npm,
        dependencies: vec![],
    }];
    let command = format!(
        "mkdir -p dist && cat src/input.txt > dist/out.txt && echo run >> '{}'",
        root.join("log.txt").display()
    );
    let target = TargetConfig {
        command,
        inputs: vec!["src/**/*.txt".into()],
        outputs: vec!["dist/**".into()],
        depends_on: vec![],
        cache_dirs: vec![],
    };
    let configs: BTreeMap<_, _> = [(
        "a".to_string(),
        ProjectConfig {
            targets: [("build".to_string(), target)].into_iter().collect(),
            ..Default::default()
        },
    )]
    .into_iter()
    .collect();
    build(discovered, &configs).unwrap()
}

#[test]
fn second_run_with_same_inputs_is_cached_and_skips_command() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    let graph = graph_with_output(root.path());
    let tasks = plan(&graph, "build").unwrap();

    let first = run_plan(&graph, &tasks, Some(&backend)).unwrap();
    assert_eq!(first[0].1, TaskOutcome::Success);

    fs::remove_dir_all(root.path().join("a/dist")).unwrap();
    let second = run_plan(&graph, &tasks, Some(&backend)).unwrap();

    assert_eq!(second[0].1, TaskOutcome::Cached);
    assert_eq!(fs::read_to_string(root.path().join("log.txt")).unwrap(), "run\n");
    assert!(root.path().join("a/dist/out.txt").exists());
}

#[test]
fn changed_input_misses_cache() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    let graph = graph_with_output(root.path());
    let tasks = plan(&graph, "build").unwrap();

    run_plan(&graph, &tasks, Some(&backend)).unwrap();
    fs::write(root.path().join("a/src/input.txt"), "v2").unwrap();
    let second = run_plan(&graph, &tasks, Some(&backend)).unwrap();

    assert_eq!(second[0].1, TaskOutcome::Success);
    assert_eq!(fs::read_to_string(root.path().join("log.txt")).unwrap(), "run\nrun\n");
}

#[test]
fn restore_leaves_exactly_the_snapshot_outputs() {
    // Version 1 produces dist/a.js, version 2 produces dist/b.js. Switching
    // back to version 1 must restore dist/a.js without leaving dist/b.js behind.
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    let dir = root.path().join("a");
    fs::create_dir_all(&dir).unwrap();
    let command = "case $(cat version.txt) in \
        1) rm -rf dist && mkdir dist && echo a > dist/a.js ;; \
        2) rm -rf dist && mkdir dist && echo b > dist/b.js ;; esac";
    let target = TargetConfig {
        command: command.into(),
        inputs: vec!["version.txt".into()],
        outputs: vec!["dist/**".into()],
        depends_on: vec![],
        cache_dirs: vec![],
    };
    let discovered = vec![DiscoveredProject {
        name: "a".into(),
        root: dir.clone(),
        ecosystem: Ecosystem::Npm,
        dependencies: vec![],
    }];
    let configs: BTreeMap<_, _> = [(
        "a".to_string(),
        ProjectConfig {
            targets: [("build".to_string(), target)].into_iter().collect(),
            ..Default::default()
        },
    )]
    .into_iter()
    .collect();
    let graph = build(discovered, &configs).unwrap();
    let tasks = plan(&graph, "build").unwrap();

    fs::write(dir.join("version.txt"), "1").unwrap();
    run_plan(&graph, &tasks, Some(&backend)).unwrap();
    fs::write(dir.join("version.txt"), "2").unwrap();
    run_plan(&graph, &tasks, Some(&backend)).unwrap();
    fs::write(dir.join("version.txt"), "1").unwrap();
    let back = run_plan(&graph, &tasks, Some(&backend)).unwrap();

    assert_eq!(back[0].1, TaskOutcome::Cached);
    assert!(dir.join("dist/a.js").exists());
    assert!(!dir.join("dist/b.js").exists(), "output from the other version must be removed");
}

#[test]
fn cache_miss_keeps_existing_outputs() {
    // The command is incremental: it does not recreate dist/keep.txt, and the
    // cache is empty. The miss must not remove the output the tool still needs.
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    let dir = root.path().join("a");
    fs::create_dir_all(dir.join("dist")).unwrap();
    fs::write(dir.join("dist/keep.txt"), "built earlier").unwrap();
    let target = TargetConfig {
        command: "echo ran > ran.txt".into(),
        inputs: vec![],
        outputs: vec!["dist/**".into()],
        depends_on: vec![],
        cache_dirs: vec![],
    };
    let discovered = vec![DiscoveredProject {
        name: "a".into(),
        root: dir.clone(),
        ecosystem: Ecosystem::Npm,
        dependencies: vec![],
    }];
    let configs: BTreeMap<_, _> = [(
        "a".to_string(),
        ProjectConfig {
            targets: [("build".to_string(), target)].into_iter().collect(),
            ..Default::default()
        },
    )]
    .into_iter()
    .collect();
    let graph = build(discovered, &configs).unwrap();
    let tasks = plan(&graph, "build").unwrap();

    let results = run_plan(&graph, &tasks, Some(&backend)).unwrap();

    assert_eq!(results[0].1, TaskOutcome::Success);
    assert!(dir.join("dist/keep.txt").exists(), "a miss must not remove existing outputs");
}

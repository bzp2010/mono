use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mono_cache::LocalBackend;
use mono_core::{DiscoveredProject, Ecosystem, ProjectConfig, TargetConfig};
use mono_graph::{build, plan, ProjectGraph};
use mono_task::{run_plan, TaskOutcome};

/// One project `a`. Its `build` appends to `target/count.txt` and to `log.txt`,
/// so the file contents show whether the directory was restored before the run.
fn graph_with_cache_dir(root: &Path) -> ProjectGraph {
    let dir = root.join("a");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("Cargo.lock"), "lock v1").unwrap();
    let discovered = vec![DiscoveredProject {
        name: "a".into(),
        root: dir,
        ecosystem: Ecosystem::Npm,
        dependencies: vec![],
    }];
    let command = format!(
        "mkdir -p target && echo x >> target/count.txt && echo run >> '{}'",
        root.join("log.txt").display()
    );
    let target = TargetConfig {
        command,
        inputs: vec!["Cargo.lock".into()],
        outputs: vec![],
        depends_on: vec![],
        cache_dirs: vec!["target".into()],
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
fn cache_dir_is_restored_before_the_command_runs() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    let graph = graph_with_cache_dir(root.path());
    let tasks = plan(&graph, "build").unwrap();
    let count = root.path().join("a/target/count.txt");

    let first = run_plan(&graph, &tasks, Some(&backend)).unwrap();
    assert_eq!(first[0].1, TaskOutcome::Success);
    assert_eq!(fs::read_to_string(&count).unwrap(), "x\n");

    fs::remove_dir_all(root.path().join("a/target")).unwrap();
    let second = run_plan(&graph, &tasks, Some(&backend)).unwrap();

    // The command still runs (the tool owns incremental state), but it appends
    // to the restored file, so the first line survived.
    assert_eq!(second[0].1, TaskOutcome::Success);
    assert_eq!(fs::read_to_string(&count).unwrap(), "x\nx\n");
    assert_eq!(fs::read_to_string(root.path().join("log.txt")).unwrap(), "run\nrun\n");
}

#[test]
fn cache_dir_is_not_restored_without_a_backend() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    let graph = graph_with_cache_dir(root.path());
    let tasks = plan(&graph, "build").unwrap();
    run_plan(&graph, &tasks, Some(&backend)).unwrap();

    fs::remove_dir_all(root.path().join("a/target")).unwrap();
    run_plan(&graph, &tasks, None).unwrap();

    assert_eq!(fs::read_to_string(root.path().join("a/target/count.txt")).unwrap(), "x\n");
}

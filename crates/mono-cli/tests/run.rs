use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::str::contains;

/// Writes a pnpm workspace with `a` (defines `build`) and `b` (defines nothing).
fn workspace(root: &Path, build_command: &str) {
    fs::write(root.join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();
    fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    let a = root.join("packages/a");
    let b = root.join("packages/b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("package.json"), r#"{"name":"a","version":"1.0.0"}"#).unwrap();
    fs::write(b.join("package.json"), r#"{"name":"b","version":"1.0.0"}"#).unwrap();
    let project = format!(r#"{{"targets":{{"build":{{"command":"{build_command}"}}}}}}"#);
    fs::write(a.join("project.json"), project).unwrap();
}

#[test]
fn run_executes_target_and_skips_projects_without_it() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path(), "echo built > built.txt");
    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .args(["run", "build"])
        .assert()
        .success();
    assert!(dir.path().join("packages/a/built.txt").exists());
    assert!(!dir.path().join("packages/b/built.txt").exists());
}

#[test]
fn run_works_from_a_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path(), "echo built > built.txt");
    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path().join("packages/b"))
        .args(["run", "build"])
        .assert()
        .success();
    assert!(dir.path().join("packages/a/built.txt").exists());
}

#[test]
fn failing_task_makes_run_fail() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path(), "exit 4");
    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .args(["run", "build"])
        .assert()
        .code(1);
}

#[test]
fn run_outside_a_workspace_is_error() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .args(["run", "build"])
        .assert()
        .code(2)
        .stderr(contains("mono.json"));
}

#[test]
fn second_run_reports_cache_hit() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path(), "echo built > built.txt");
    fs::write(
        dir.path().join("packages/a/project.json"),
        r#"{"targets":{"build":{"command":"echo built > built.txt","outputs":["built.txt"]}}}"#,
    )
    .unwrap();
    let run = || {
        Command::cargo_bin("mono")
            .unwrap()
            .current_dir(dir.path())
            .args(["run", "build"])
            .assert()
            .success()
    };
    run().stdout(contains("a:build: ok"));
    fs::remove_file(dir.path().join("packages/a/built.txt")).unwrap();
    run().stdout(contains("a:build: cached"));
    assert!(dir.path().join("packages/a/built.txt").exists());
}

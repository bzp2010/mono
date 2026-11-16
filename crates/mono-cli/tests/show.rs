use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// A pnpm workspace: `lib` (1.2.3) has a build target, and `app` depends on it.
fn workspace(dir: &Path) {
    fs::write(dir.join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();
    fs::write(dir.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    fs::create_dir_all(dir.join("packages/lib")).unwrap();
    fs::create_dir_all(dir.join("packages/app")).unwrap();
    fs::write(dir.join("packages/lib/package.json"), r#"{"name":"@acme/lib","version":"1.2.3"}"#).unwrap();
    fs::write(
        dir.join("packages/lib/project.json"),
        r#"{"name":"lib","targets":{"build":{"command":"echo hi","inputs":["src/**"],"outputs":["dist/**"],"dependsOn":["^build"]}}}"#,
    )
    .unwrap();
    fs::write(
        dir.join("packages/app/package.json"),
        r#"{"name":"app","version":"0.1.0","dependencies":{"@acme/lib":"workspace:*"}}"#,
    )
    .unwrap();
}

fn mono(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("mono").unwrap();
    cmd.current_dir(dir);
    cmd
}

#[test]
fn show_projects_lists_names_one_per_line() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path());
    mono(dir.path()).args(["show", "projects"]).assert().success().stdout("app\nlib\n");
}

#[test]
fn show_projects_json_is_an_array_of_names() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path());
    let out = mono(dir.path()).args(["show", "projects", "--json"]).output().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value, serde_json::json!(["app", "lib"]));
}

#[test]
fn show_project_prints_the_metadata_of_one_project() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path());
    mono(dir.path())
        .args(["show", "project", "lib"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name: lib"))
        .stdout(predicate::str::contains("package: @acme/lib"))
        .stdout(predicate::str::contains("version: 1.2.3"))
        .stdout(predicate::str::contains("ecosystem: pnpm"))
        .stdout(predicate::str::contains("root: packages/lib"))
        .stdout(predicate::str::contains("dependents: app"))
        .stdout(predicate::str::contains("build"))
        .stdout(predicate::str::contains("echo hi"));
}

#[test]
fn show_project_json_has_the_full_structure() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path());
    let out = mono(dir.path()).args(["show", "project", "app", "--json"]).output().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["name"], "app");
    assert_eq!(value["package"], "app");
    assert_eq!(value["version"], "0.1.0");
    assert_eq!(value["ecosystem"], "pnpm");
    assert_eq!(value["root"], "packages/app");
    assert_eq!(value["dependencies"], serde_json::json!(["lib"]));
    assert_eq!(value["dependents"], serde_json::json!([]));
    assert_eq!(value["targets"], serde_json::json!({}));

    let out = mono(dir.path()).args(["show", "project", "lib", "--json"]).output().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        value["targets"]["build"],
        serde_json::json!({
            "command": "echo hi",
            "inputs": ["src/**"],
            "outputs": ["dist/**"],
            "dependsOn": ["^build"],
        })
    );
}

#[test]
fn show_unknown_project_fails_and_lists_the_known_names() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path());
    mono(dir.path())
        .args(["show", "project", "ghost"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown project ghost"))
        .stderr(predicate::str::contains("app, lib"));
}

#[test]
fn show_works_from_a_subdirectory_with_workspace_relative_roots() {
    let dir = tempfile::tempdir().unwrap();
    workspace(dir.path());
    mono(&dir.path().join("packages/app"))
        .args(["show", "project", "lib", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"root\":\"packages/lib\""));
}

#[test]
fn a_project_at_the_workspace_root_has_root_dot() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();
    fs::write(dir.path().join("pnpm-workspace.yaml"), "packages: []\n").unwrap();
    fs::write(dir.path().join("package.json"), r#"{"name":"top","version":"3.0.0"}"#).unwrap();
    let out = mono(dir.path()).args(["show", "project", "top", "--json"]).output().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["root"], ".");
}

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures").join(name)
}

/// Copies a fixture into a fresh temporary workspace.
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let rel = entry.path().strip_prefix(from).unwrap();
        let dst = to.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dst).unwrap();
        } else {
            fs::copy(entry.path(), &dst).unwrap();
        }
    }
}

#[test]
fn graph_lists_projects_in_dependency_order() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("pnpm-ws"), dir.path());
    fs::write(dir.path().join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();

    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .arg("graph")
        .assert()
        .success()
        .stdout("x\ny -> x\n");
}

#[test]
fn project_json_name_overrides_the_package_name_in_the_graph() {
    // `app` depends on the package `x`, whose project is renamed to `renamed` by its
    // project.json. The graph must show the rename and the edge to it.
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();
    fs::write(dir.path().join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    fs::create_dir_all(dir.path().join("packages/x")).unwrap();
    fs::create_dir_all(dir.path().join("packages/app")).unwrap();
    fs::write(dir.path().join("packages/x/package.json"), r#"{"name":"x","version":"1.0.0"}"#).unwrap();
    fs::write(dir.path().join("packages/x/project.json"), r#"{"name":"renamed"}"#).unwrap();
    fs::write(
        dir.path().join("packages/app/package.json"),
        r#"{"name":"app","version":"1.0.0","dependencies":{"x":"workspace:*"}}"#,
    )
    .unwrap();

    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .arg("graph")
        .assert()
        .success()
        .stdout("renamed\napp -> renamed\n");
}

#[test]
fn unnamed_root_without_a_project_json_name_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();
    fs::write(dir.path().join("pnpm-workspace.yaml"), "packages: []\n").unwrap();
    fs::write(dir.path().join("package.json"), r#"{"private":true}"#).unwrap();

    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .arg("graph")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("project.json"));
}

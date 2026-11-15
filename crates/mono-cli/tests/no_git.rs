use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::str::contains;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures").join(name)
}

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

/// A temporary workspace with no `.git` directory anywhere above it.
#[test]
fn run_works_without_git() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("cargo-ws"), dir.path());
    fs::write(dir.path().join("mono.json"), r#"{"ecosystems":["cargo"]}"#).unwrap();
    fs::write(
        dir.path().join("project.json"),
        r#"{"targets":{"build":{"command":"true"}}}"#,
    )
    .unwrap();
    assert!(!dir.path().join(".git").exists());
    // A project is named after its workspace root directory.
    let project = dir.path().file_name().unwrap().to_string_lossy().into_owned();

    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .args(["run", "build"])
        .assert()
        .success()
        .stdout(contains(format!("{project}:build: ok")));
}

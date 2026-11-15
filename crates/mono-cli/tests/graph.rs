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

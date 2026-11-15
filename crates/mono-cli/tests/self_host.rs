use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

/// The repository root, two levels above this crate.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn graph_discovers_this_repository_as_a_project() {
    // A project is named after its workspace root directory, whatever the checkout is called.
    let root = repo_root().canonicalize().unwrap();
    let name = root.file_name().unwrap().to_string_lossy().into_owned();
    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(&root)
        .arg("graph")
        .assert()
        .success()
        .stdout(contains(name));
}

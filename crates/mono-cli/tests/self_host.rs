use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

/// The repository root, two levels above this crate.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn graph_discovers_this_repository_as_a_project() {
    Command::cargo_bin("mono")
        .unwrap()
        .current_dir(repo_root())
        .arg("graph")
        .assert()
        .success()
        .stdout(contains("monorepo"));
}

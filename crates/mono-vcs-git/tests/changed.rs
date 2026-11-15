use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mono_vcs_git::changed_files;

/// Runs git in `dir` with a fixed identity, so commits work on any machine.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.invalid"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// A repository with `a.txt` and `sub/b.txt` committed.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    fs::create_dir_all(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/b.txt"), "b").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "initial"]);
    dir
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap()
}

#[test]
fn modified_and_untracked_files_are_changed_absolute_paths() {
    let dir = repo();
    let top = canonical(dir.path());
    fs::write(dir.path().join("a.txt"), "changed").unwrap();
    fs::write(dir.path().join("new.txt"), "new").unwrap();

    let mut changed = changed_files(dir.path(), "HEAD").unwrap();
    changed.sort();
    assert_eq!(changed, vec![top.join("a.txt"), top.join("new.txt")]);
}

#[test]
fn committed_changes_since_the_base_are_included() {
    let dir = repo();
    let top = canonical(dir.path());
    fs::write(dir.path().join("sub/b.txt"), "b2").unwrap();
    git(dir.path(), &["commit", "-q", "-am", "change b"]);

    let changed = changed_files(dir.path(), "HEAD~1").unwrap();
    assert_eq!(changed, vec![top.join("sub/b.txt")]);
}

#[test]
fn a_directory_outside_a_repository_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(changed_files(dir.path(), "HEAD").is_err());
}

#[test]
fn an_unknown_base_is_an_error() {
    let dir = repo();
    assert!(changed_files(dir.path(), "no-such-revision").is_err());
}

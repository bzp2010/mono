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

#[test]
fn a_base_that_names_a_path_is_not_read_as_a_path_filter() {
    // `ws` is a directory at the top level, not a revision. It must be an error,
    // never a silent filter that reports nothing.
    let dir = repo();
    fs::create_dir_all(dir.path().join("ws")).unwrap();
    fs::write(dir.path().join("ws/x.txt"), "x").unwrap();
    assert!(changed_files(dir.path(), "ws").is_err());
}

#[test]
fn a_base_option_is_not_read_as_a_git_option() {
    let dir = repo();
    let out = dir.path().join("out.txt");
    let base = format!("--output={}", out.display());
    assert!(changed_files(dir.path(), &base).is_err());
    assert!(!out.exists(), "git must not be asked to write a file");
}

#[test]
fn renamed_files_report_both_paths() {
    // A rename is a deletion of the old path and an addition of the new one. The
    // old path's owner must be reported too, so both projects are affected.
    let dir = repo();
    let top = canonical(dir.path());
    git(dir.path(), &["mv", "sub/b.txt", "a2.txt"]);
    let mut changed = changed_files(dir.path(), "HEAD").unwrap();
    changed.sort();
    assert_eq!(changed, vec![top.join("a2.txt"), top.join("sub/b.txt")]);
}

#[test]
fn a_branch_is_compared_against_its_merge_base_not_the_moving_base() {
    // `main` moves on after the feature branch is cut. The change `main` made to
    // `sub/b.txt` is not a change of this branch, so only `a.txt` is reported.
    let dir = repo();
    let top = canonical(dir.path());
    git(dir.path(), &["branch", "-q", "-M", "main"]);
    git(dir.path(), &["checkout", "-q", "-b", "feature"]);
    fs::write(dir.path().join("a.txt"), "on feature").unwrap();
    git(dir.path(), &["commit", "-q", "-am", "feature change"]);
    git(dir.path(), &["checkout", "-q", "main"]);
    fs::write(dir.path().join("sub/b.txt"), "on main").unwrap();
    git(dir.path(), &["commit", "-q", "-am", "main moves"]);
    git(dir.path(), &["checkout", "-q", "feature"]);

    let changed = changed_files(dir.path(), "main").unwrap();
    assert_eq!(changed, vec![top.join("a.txt")]);
}

#[test]
fn a_directory_name_with_a_trailing_space_works() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("ws ");
    fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    fs::write(dir.join("a.txt"), "a").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "init"]);
    fs::write(dir.join("a.txt"), "changed").unwrap();
    assert_eq!(changed_files(&dir, "HEAD").unwrap(), vec![canonical(&dir).join("a.txt")]);
}

#[test]
fn unrelated_histories_are_an_error_not_a_guess() {
    // With no common ancestor there is no merge base. Falling back to the base tip
    // could report no change where there is one, so the call must fail instead.
    let dir = repo();
    git(dir.path(), &["checkout", "-q", "--orphan", "other"]);
    fs::write(dir.path().join("unrelated.txt"), "x").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "unrelated"]);
    let base = Command::new("git")
        .current_dir(dir.path())
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .unwrap();
    let other = String::from_utf8_lossy(&base.stdout).trim().to_string();
    git(dir.path(), &["checkout", "-q", "--orphan", "third"]);
    fs::write(dir.path().join("third.txt"), "y").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "third"]);
    assert!(changed_files(dir.path(), &other).is_err());
}

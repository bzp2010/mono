use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::Command as Mono;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_empty};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.invalid"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// A pnpm workspace in a git repository. `b` depends on `a`. `c` is independent.
/// Each project's `build` appends its name to `log.txt` in the workspace root.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let log = root.join("log.txt");
    fs::write(root.join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();
    fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    for (name, deps) in [("a", ""), ("b", r#","dependencies":{"a":"workspace:*"}"#), ("c", "")] {
        let pkg = root.join("packages").join(name);
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("package.json"),
            format!(r#"{{"name":"{name}","version":"1.0.0"{deps}}}"#),
        )
        .unwrap();
        fs::write(
            pkg.join("project.json"),
            format!(r#"{{"targets":{{"build":{{"command":"echo {name} >> '{}'"}}}}}}"#, log.display()),
        )
        .unwrap();
    }
    git(root, &["init", "-q"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "initial"]);
    dir
}

fn log(root: &Path) -> String {
    fs::read_to_string(root.join("log.txt")).unwrap_or_default()
}

#[test]
fn affected_runs_only_changed_projects_and_their_dependents() {
    let dir = workspace();
    let root = dir.path();
    fs::write(root.join("packages/a/index.js"), "changed").unwrap();

    Mono::cargo_bin("mono")
        .unwrap()
        .current_dir(root)
        .args(["run", "build", "--affected"])
        .assert()
        .success()
        .stdout(contains("a:build: ok"))
        .stdout(contains("b:build: ok"))
        .stdout(contains("c:build").not());

    let text = log(root);
    let mut ran: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    ran.sort();
    assert_eq!(ran, vec!["a", "b"]);
}

#[test]
fn affected_with_no_changes_runs_nothing() {
    let dir = workspace();
    Mono::cargo_bin("mono")
        .unwrap()
        .current_dir(dir.path())
        .args(["run", "build", "--affected"])
        .assert()
        .success()
        .stdout(is_empty());
    assert!(log(dir.path()).is_empty());
}

#[test]
fn affected_with_a_base_compares_against_that_revision() {
    let dir = workspace();
    let root = dir.path();
    fs::write(root.join("packages/c/index.js"), "changed").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "change c"]);

    // Against the commit before the change, `c` is affected.
    Mono::cargo_bin("mono")
        .unwrap()
        .current_dir(root)
        .args(["run", "build", "--affected", "--base", "HEAD~1"])
        .assert()
        .success()
        .stdout(contains("c:build: ok"));
}

#[test]
fn affected_runs_the_upstream_tasks_that_its_outputs_depend_on() {
    // `b` reads the output of `a`. Only `b` changed, but a fresh checkout has no
    // output of `a` yet, so `a:build` must run before `b:build`.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("mono.json"), r#"{"ecosystems":["pnpm"]}"#).unwrap();
    fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    for name in ["a", "b"] {
        fs::create_dir_all(root.join("packages").join(name)).unwrap();
    }
    fs::write(root.join("packages/a/package.json"), r#"{"name":"a","version":"1.0.0"}"#).unwrap();
    fs::write(
        root.join("packages/b/package.json"),
        r#"{"name":"b","version":"1.0.0","dependencies":{"a":"workspace:*"}}"#,
    )
    .unwrap();
    fs::write(
        root.join("packages/a/project.json"),
        r#"{"targets":{"build":{"command":"mkdir -p dist && echo a > dist/out","outputs":["dist/**"]}}}"#,
    )
    .unwrap();
    fs::write(
        root.join("packages/b/project.json"),
        r#"{"targets":{"build":{"command":"mkdir -p dist && cat ../a/dist/out > dist/out","outputs":["dist/**"],"dependsOn":["^build"]}}}"#,
    )
    .unwrap();
    git(root, &["init", "-q"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "initial"]);
    fs::write(root.join("packages/b/index.js"), "changed").unwrap();

    Mono::cargo_bin("mono")
        .unwrap()
        .current_dir(root)
        .args(["run", "build", "--affected"])
        .assert()
        .success()
        .stdout(contains("a:build"))
        .stdout(contains("b:build"));
    assert_eq!(fs::read_to_string(root.join("packages/b/dist/out")).unwrap().trim(), "a");
}

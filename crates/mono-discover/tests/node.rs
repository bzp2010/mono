use std::path::PathBuf;

use mono_core::Ecosystem;
use mono_discover::{discover, DiscoverError};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn find<'a>(
    projects: &'a [mono_core::DiscoveredProject],
    name: &str,
) -> &'a mono_core::DiscoveredProject {
    projects.iter().find(|p| p.name == name).unwrap()
}

#[test]
fn pnpm_workspace_text_dependency_is_inferred() {
    let projects = discover(&fixture("pnpm-ws"), &[Ecosystem::Pnpm]).unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(find(&projects, "y").dependencies, vec!["x".to_string()]);
    assert!(find(&projects, "x").dependencies.is_empty());
}

#[test]
fn pnpm_member_without_manifest_is_error() {
    let err = discover(&fixture("pnpm-broken"), &[Ecosystem::Pnpm]).unwrap_err();
    match err {
        DiscoverError::MissingManifest { dir } => assert!(dir.ends_with("packages/x")),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn npm_dependency_matching_member_version_is_inferred() {
    let projects = discover(&fixture("npm-ws"), &[Ecosystem::Npm]).unwrap();
    assert_eq!(projects.len(), 2);
    assert_eq!(find(&projects, "two").dependencies, vec!["one".to_string()]);
    assert!(find(&projects, "one").dependencies.is_empty());
}

#[test]
fn glob_metacharacters_in_workspace_root_do_not_hide_members() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("ws[1]");
    let member = root.join("packages/x");
    std::fs::create_dir_all(&member).unwrap();
    std::fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    std::fs::write(member.join("package.json"), r#"{"name":"x","version":"1.0.0"}"#).unwrap();
    let projects = discover(&root, &[Ecosystem::Pnpm]).unwrap();
    assert_eq!(projects.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["x"]);
}

#[test]
fn pnpm_negative_pattern_removes_members_before_validation() {
    // packages/ignored has no package.json, but it is excluded, so it must
    // neither be a member nor make discovery fail.
    let projects = discover(&fixture("pnpm-exclude"), &[Ecosystem::Pnpm]).unwrap();
    assert_eq!(projects.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["x"]);
}

#[test]
fn pnpm_root_package_is_a_project_too() {
    // Every pnpm package, the root one included, is a project of its own.
    let projects = discover(&fixture("pnpm-root"), &[Ecosystem::Pnpm]).unwrap();
    let root = find(&projects, "root-pkg");
    assert_eq!(projects.len(), 2);
    assert_eq!(root.dependencies, vec!["x".to_string()]);
    assert!(find(&projects, "x").dependencies.is_empty());
}

#[test]
fn unnamed_pnpm_root_colliding_with_a_member_is_error() {
    // The root takes its directory name, "x", which a member already has.
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("x");
    let member = root.join("packages/x");
    std::fs::create_dir_all(&member).unwrap();
    std::fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    std::fs::write(root.join("package.json"), r#"{"private":true}"#).unwrap();
    std::fs::write(member.join("package.json"), r#"{"name":"x","version":"1.0.0"}"#).unwrap();
    let err = discover(&root, &[Ecosystem::Pnpm]).unwrap_err();
    assert!(matches!(err, DiscoverError::InvalidManifest { .. }));
}

#[test]
fn pnpm_workspace_alias_resolves_to_the_target_package() {
    // `bar` is an alias for the workspace package `foo`, so the edge points at `foo`.
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("ws");
    let foo = root.join("packages/foo");
    std::fs::create_dir_all(&foo).unwrap();
    std::fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    std::fs::write(root.join("package.json"), r#"{"name":"root","private":true,"devDependencies":{"bar":"workspace:foo@*"}}"#).unwrap();
    std::fs::write(foo.join("package.json"), r#"{"name":"foo","version":"1.0.0"}"#).unwrap();
    let projects = discover(&root, &[Ecosystem::Pnpm]).unwrap();
    assert_eq!(find(&projects, "root").dependencies, vec!["foo".to_string()]);
}

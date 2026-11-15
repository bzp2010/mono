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

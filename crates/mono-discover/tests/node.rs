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
    projects.iter().find(|p| p.name.as_deref() == Some(name)).unwrap()
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
    assert_eq!(projects.iter().map(|p| p.name.clone().unwrap()).collect::<Vec<_>>(), vec!["x".to_string()]);
}

#[test]
fn pnpm_negative_pattern_removes_members_before_validation() {
    // packages/ignored has no package.json, but it is excluded, so it must
    // neither be a member nor make discovery fail.
    let projects = discover(&fixture("pnpm-exclude"), &[Ecosystem::Pnpm]).unwrap();
    assert_eq!(projects.iter().map(|p| p.name.clone().unwrap()).collect::<Vec<_>>(), vec!["x".to_string()]);
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
fn unnamed_pnpm_root_is_discovered_without_a_name() {
    // The root has no name and nothing is guessed for it. The graph reports it
    // unless project.json names it, even when a member shares its directory name.
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("x");
    let member = root.join("packages/x");
    std::fs::create_dir_all(&member).unwrap();
    std::fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
    std::fs::write(root.join("package.json"), r#"{"private":true}"#).unwrap();
    std::fs::write(member.join("package.json"), r#"{"name":"x","version":"1.0.0"}"#).unwrap();
    let projects = discover(&root, &[Ecosystem::Pnpm]).unwrap();
    let unnamed = projects.iter().find(|p| p.root == root).unwrap();
    assert!(unnamed.name.is_none());
    assert!(unnamed.package_name.is_none());
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

#[test]
fn pnpm_path_workspace_spec_keeps_the_scoped_package_name() {
    // A path reference is a `workspace:` spec too. Its `@` belongs to a scope,
    // so the edge must still point at `@scope/foo`.
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("ws");
    let app = root.join("packages/app");
    let foo = root.join("packages/@scope/foo");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::create_dir_all(&foo).unwrap();
    std::fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - \"packages/app\"\n  - \"packages/@scope/foo\"\n",
    )
    .unwrap();
    std::fs::write(
        app.join("package.json"),
        r#"{"name":"app","version":"1.0.0","dependencies":{"@scope/foo":"workspace:../@scope/foo"}}"#,
    )
    .unwrap();
    std::fs::write(foo.join("package.json"), r#"{"name":"@scope/foo","version":"1.0.0"}"#).unwrap();
    let projects = discover(&root, &[Ecosystem::Pnpm]).unwrap();
    assert_eq!(find(&projects, "app").dependencies, vec!["@scope/foo".to_string()]);
}

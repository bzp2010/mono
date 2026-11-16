use std::path::PathBuf;

use mono_core::Ecosystem;
use mono_discover::discover;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn cargo_workspace_is_one_project() {
    let root = fixture("cargo-ws");
    let projects = discover(&root, &[Ecosystem::Cargo]).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name.as_deref(), Some("cargo-ws"));
    assert_eq!(projects[0].root, root.canonicalize().unwrap());
    assert!(projects[0].dependencies.is_empty());
}

#[test]
fn cargo_workspace_version_is_the_one_its_packages_share() {
    let projects = discover(&fixture("cargo-ws"), &[Ecosystem::Cargo]).unwrap();
    assert_eq!(projects[0].version.as_deref(), Some("0.1.0"));
}

#[test]
fn cargo_workspace_without_a_shared_version_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("Cargo.toml"), "[workspace]\nresolver = \"2\"\nmembers = [\"a\", \"b\"]\n").unwrap();
    for (name, version) in [("a", "0.1.0"), ("b", "0.2.0")] {
        std::fs::create_dir_all(root.join(name).join("src")).unwrap();
        std::fs::write(root.join(name).join("src/lib.rs"), "").unwrap();
        std::fs::write(
            root.join(name).join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n"),
        )
        .unwrap();
    }
    let projects = discover(root, &[Ecosystem::Cargo]).unwrap();
    assert_eq!(projects[0].version, None);
}

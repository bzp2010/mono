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
    assert_eq!(projects[0].name, "cargo-ws");
    assert_eq!(projects[0].root, root.canonicalize().unwrap());
    assert!(projects[0].dependencies.is_empty());
}

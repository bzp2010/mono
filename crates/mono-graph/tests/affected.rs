use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mono_core::{DiscoveredProject, Ecosystem};
use mono_graph::{affected, build};

fn proj(name: &str, root: &str, deps: &[&str]) -> DiscoveredProject {
    DiscoveredProject {
        name: Some(name.to_string()),
        package_name: None,
        root: PathBuf::from(root),
        ecosystem: Ecosystem::Npm,
        dependencies: deps.iter().map(|d| d.to_string()).collect(),
        version: None,
    }
}

/// `app` depends on `lib`, which depends on `core`. `other` is independent.
fn graph() -> mono_graph::ProjectGraph {
    build(
        vec![
            proj("core", "/ws/core", &[]),
            proj("lib", "/ws/lib", &["core"]),
            proj("app", "/ws/app", &["lib"]),
            proj("other", "/ws/other", &[]),
        ],
        &BTreeMap::new(),
    )
    .unwrap()
}

fn set(names: &[&str]) -> std::collections::BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_change_affects_its_project_and_everything_that_depends_on_it() {
    let g = graph();
    assert_eq!(affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/core/src/a.rs")]), set(&["core", "lib", "app"]));
}

#[test]
fn a_change_at_the_top_affects_only_itself() {
    let g = graph();
    assert_eq!(affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/app/index.ts")]), set(&["app"]));
}

#[test]
fn a_change_outside_every_project_affects_nothing() {
    let g = graph();
    assert_eq!(affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/README.md")]), set(&[]));
}

#[test]
fn a_file_belongs_to_the_most_specific_project() {
    // `inner` lives inside `outer`. A change in `inner` must not affect `outer` through
    // a containment rule, only through dependencies.
    let g = build(
        vec![proj("outer", "/ws/outer", &[]), proj("inner", "/ws/outer/inner", &[])],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/outer/inner/x.rs")]), set(&["inner"]));
    assert_eq!(affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/outer/y.rs")]), set(&["outer"]));
}

#[test]
fn several_changes_are_unioned() {
    let g = graph();
    let changed = [PathBuf::from("/ws/other/a"), PathBuf::from("/ws/core/b")];
    assert_eq!(affected(&g, Path::new("/ws"), &changed), set(&["core", "lib", "app", "other"]));
}

#[test]
fn every_project_sharing_the_owning_root_is_affected() {
    // A root can hold two projects, such as a Cargo workspace and a pnpm package. A
    // change under that root belongs to both, so both must be affected.
    let g = build(
        vec![proj("cargo-root", "/ws/root", &[]), proj("pnpm-root", "/ws/root", &[])],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/root/src/lib.rs")]),
        set(&["cargo-root", "pnpm-root"])
    );
}

fn proj_in(name: &str, root: &str, ecosystem: Ecosystem) -> DiscoveredProject {
    DiscoveredProject {
        name: Some(name.to_string()),
        package_name: None,
        root: PathBuf::from(root),
        ecosystem,
        dependencies: vec![],
        version: None,
    }
}

#[test]
fn a_cargo_lock_change_affects_only_cargo_projects() {
    // A lockfile can change every build of its ecosystem, but not the builds of
    // other ecosystems in the same workspace.
    let g = build(
        vec![
            proj_in("crate-a", "/ws/crates/a", Ecosystem::Cargo),
            proj_in("node-app", "/ws/packages/app", Ecosystem::Npm),
        ],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/Cargo.lock")]),
        set(&["crate-a"])
    );
}

#[test]
fn a_pnpm_lock_change_affects_only_node_projects_in_a_mixed_root() {
    // A Cargo workspace and a pnpm package share the root. The pnpm lockfile is a
    // Node input, so it affects the pnpm package and not the Cargo project.
    let g = build(
        vec![
            proj_in("crate-root", "/ws", Ecosystem::Cargo),
            proj_in("pkg-root", "/ws", Ecosystem::Pnpm),
            proj_in("pkg-other", "/ws/packages/other", Ecosystem::Pnpm),
        ],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/pnpm-lock.yaml")]),
        set(&["pkg-root", "pkg-other"])
    );
}

#[test]
fn mono_json_affects_every_project() {
    let g = build(
        vec![
            proj_in("crate-a", "/ws/crates/a", Ecosystem::Cargo),
            proj_in("node-app", "/ws/packages/app", Ecosystem::Npm),
        ],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/mono.json")]),
        set(&["crate-a", "node-app"])
    );
}

#[test]
fn a_lockfile_outside_the_workspace_root_is_not_global() {
    // A nested lockfile is not a workspace input. It belongs to the project it sits
    // in, and that project alone is affected.
    let g = build(
        vec![
            proj_in("crate-a", "/ws/crates/a", Ecosystem::Cargo),
            proj_in("node-app", "/ws/packages/app", Ecosystem::Npm),
        ],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        affected(&g, Path::new("/ws"), &[PathBuf::from("/ws/packages/app/Cargo.lock")]),
        set(&["node-app"])
    );
}

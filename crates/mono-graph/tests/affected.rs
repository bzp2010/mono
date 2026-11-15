use std::collections::BTreeMap;
use std::path::PathBuf;

use mono_core::{DiscoveredProject, Ecosystem};
use mono_graph::{affected, build};

fn proj(name: &str, root: &str, deps: &[&str]) -> DiscoveredProject {
    DiscoveredProject {
        name: Some(name.to_string()),
        package_name: None,
        root: PathBuf::from(root),
        ecosystem: Ecosystem::Npm,
        dependencies: deps.iter().map(|d| d.to_string()).collect(),
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
    assert_eq!(affected(&g, &[PathBuf::from("/ws/core/src/a.rs")]), set(&["core", "lib", "app"]));
}

#[test]
fn a_change_at_the_top_affects_only_itself() {
    let g = graph();
    assert_eq!(affected(&g, &[PathBuf::from("/ws/app/index.ts")]), set(&["app"]));
}

#[test]
fn a_change_outside_every_project_affects_nothing() {
    let g = graph();
    assert_eq!(affected(&g, &[PathBuf::from("/ws/README.md")]), set(&[]));
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
    assert_eq!(affected(&g, &[PathBuf::from("/ws/outer/inner/x.rs")]), set(&["inner"]));
    assert_eq!(affected(&g, &[PathBuf::from("/ws/outer/y.rs")]), set(&["outer"]));
}

#[test]
fn several_changes_are_unioned() {
    let g = graph();
    let changed = [PathBuf::from("/ws/other/a"), PathBuf::from("/ws/core/b")];
    assert_eq!(affected(&g, &changed), set(&["core", "lib", "app", "other"]));
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
        affected(&g, &[PathBuf::from("/ws/root/src/lib.rs")]),
        set(&["cargo-root", "pnpm-root"])
    );
}

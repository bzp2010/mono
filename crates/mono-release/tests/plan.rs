use std::collections::BTreeMap;

use mono_release::{compute_plan, parse_record, Bump, ReleaseError};
use semver::Version;

fn versions(pairs: &[(&str, &str)]) -> BTreeMap<String, Version> {
    pairs.iter().map(|(n, v)| (n.to_string(), Version::parse(v).unwrap())).collect()
}

/// `app` depends on `lib`. `dependents` maps each project to the projects that depend on it.
fn dependents(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    pairs
        .iter()
        .map(|(n, ds)| (n.to_string(), ds.iter().map(|d| d.to_string()).collect()))
        .collect()
}

#[test]
fn a_minor_bump_moves_the_project_and_patches_its_dependents() {
    let record = parse_record("---\n\"lib\": minor\n---\n\nx\n").unwrap();
    let plan = compute_plan(
        &[record],
        &versions(&[("lib", "1.2.0"), ("app", "2.0.0")]),
        &dependents(&[("lib", &["app"])]),
    )
    .unwrap();
    assert_eq!(plan["lib"].next, Version::parse("1.3.0").unwrap());
    assert_eq!(plan["lib"].bump, Bump::Minor);
    assert_eq!(plan["app"].next, Version::parse("2.0.1").unwrap());
    assert_eq!(plan["app"].bump, Bump::Patch);
}

#[test]
fn the_largest_bump_across_records_wins() {
    let a = parse_record("---\n\"lib\": patch\n---\n\nx\n").unwrap();
    let b = parse_record("---\n\"lib\": major\n---\n\ny\n").unwrap();
    let plan = compute_plan(&[a, b], &versions(&[("lib", "1.2.3")]), &dependents(&[])).unwrap();
    assert_eq!(plan["lib"].next, Version::parse("2.0.0").unwrap());
}

#[test]
fn a_bumped_dependent_is_not_downgraded_by_propagation() {
    let record = parse_record("---\n\"lib\": patch\n\"app\": major\n---\n\nx\n").unwrap();
    let plan = compute_plan(
        &[record],
        &versions(&[("lib", "1.0.0"), ("app", "1.0.0")]),
        &dependents(&[("lib", &["app"])]),
    )
    .unwrap();
    assert_eq!(plan["app"].next, Version::parse("2.0.0").unwrap());
}

#[test]
fn propagation_is_transitive() {
    let record = parse_record("---\n\"core\": minor\n---\n\nx\n").unwrap();
    let plan = compute_plan(
        &[record],
        &versions(&[("core", "1.0.0"), ("lib", "1.0.0"), ("app", "1.0.0")]),
        &dependents(&[("core", &["lib"]), ("lib", &["app"])]),
    )
    .unwrap();
    assert!(plan.contains_key("app"));
    assert_eq!(plan["app"].bump, Bump::Patch);
}

#[test]
fn a_record_for_an_unknown_project_is_an_error() {
    let record = parse_record("---\n\"ghost\": patch\n---\n\nx\n").unwrap();
    match compute_plan(&[record], &versions(&[("lib", "1.0.0")]), &dependents(&[])).unwrap_err() {
        ReleaseError::UnknownProject { name } => assert_eq!(name, "ghost"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_project_without_a_version_is_an_error() {
    let record = parse_record("---\n\"lib\": patch\n---\n\nx\n").unwrap();
    assert!(compute_plan(&[record], &versions(&[]), &dependents(&[])).is_err());
}

#[test]
fn a_dependency_cycle_terminates() {
    let record = parse_record("---\n\"a\": minor\n---\n\nx\n").unwrap();
    let plan = compute_plan(
        &[record],
        &versions(&[("a", "1.0.0"), ("b", "1.0.0")]),
        &dependents(&[("a", &["b"]), ("b", &["a"])]),
    )
    .unwrap();
    assert_eq!(plan["a"].bump, Bump::Minor);
    assert_eq!(plan["b"].bump, Bump::Patch);
}

#[test]
fn a_self_dependency_terminates() {
    let record = parse_record("---\n\"a\": patch\n---\n\nx\n").unwrap();
    let plan = compute_plan(&[record], &versions(&[("a", "1.0.0")]), &dependents(&[("a", &["a"])])).unwrap();
    assert_eq!(plan["a"].bump, Bump::Patch);
}

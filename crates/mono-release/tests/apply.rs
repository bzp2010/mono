use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mono_release::{apply_plan, ApplyError, ApplyTarget, Bump, ManifestKind, Release};
use semver::Version;

fn release(current: &str, next: &str) -> Release {
    Release {
        current: Version::parse(current).unwrap(),
        next: Version::parse(next).unwrap(),
        bump: Bump::Minor,
    }
}

/// The kind follows the manifests' file name, and is `package.json` when there are none.
fn target(project: &str, names: &[&str], manifests: &[&Path]) -> ApplyTarget {
    let cargo = manifests.iter().any(|m| m.file_name().is_some_and(|n| n == "Cargo.toml"));
    let kind = if cargo { ManifestKind::CargoToml } else { ManifestKind::PackageJson };
    kind_target(kind, project, names, manifests)
}

fn kind_target(kind: ManifestKind, project: &str, names: &[&str], manifests: &[&Path]) -> ApplyTarget {
    ApplyTarget {
        kind,
        project: project.to_string(),
        package_names: names.iter().map(|n| n.to_string()).collect(),
        manifests: manifests.iter().map(|m| m.to_path_buf()).collect(),
    }
}

fn plan(entries: &[(&str, Release)]) -> BTreeMap<String, Release> {
    entries.iter().map(|(n, r)| (n.to_string(), r.clone())).collect()
}

#[test]
fn package_json_version_and_exact_internal_dependency_are_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib.json");
    let app = dir.path().join("app.json");
    fs::write(&lib, "{\n  \"name\": \"@s/lib\",\n  \"version\": \"1.2.0\"\n}\n").unwrap();
    fs::write(
        &app,
        "{\n  \"name\": \"app\",\n  \"version\": \"2.0.0\",\n  \"dependencies\": {\n    \"@s/lib\": \"1.2.0\",\n    \"other\": \"1.2.0\"\n  }\n}\n",
    )
    .unwrap();
    // The files must be named package.json for the kind to be recognised.
    let lib_dir = dir.path().join("lib");
    let app_dir = dir.path().join("app");
    fs::create_dir_all(&lib_dir).unwrap();
    fs::create_dir_all(&app_dir).unwrap();
    fs::rename(&lib, lib_dir.join("package.json")).unwrap();
    fs::rename(&app, app_dir.join("package.json")).unwrap();

    let changed = apply_plan(
        &plan(&[("lib", release("1.2.0", "1.3.0")), ("app", release("2.0.0", "2.0.1"))]),
        &[
            target("lib", &["@s/lib"], &[&lib_dir.join("package.json")]),
            target("app", &["app"], &[&app_dir.join("package.json")]),
        ],
    )
    .unwrap();
    assert_eq!(changed.len(), 2);
    assert_eq!(
        fs::read_to_string(lib_dir.join("package.json")).unwrap(),
        "{\n  \"name\": \"@s/lib\",\n  \"version\": \"1.3.0\"\n}\n"
    );
    assert_eq!(
        fs::read_to_string(app_dir.join("package.json")).unwrap(),
        "{\n  \"name\": \"app\",\n  \"version\": \"2.0.1\",\n  \"dependencies\": {\n    \"@s/lib\": \"1.3.0\",\n    \"other\": \"1.2.0\"\n  }\n}\n"
    );
}

fn write_pkg(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    let d = dir.join(name);
    fs::create_dir_all(&d).unwrap();
    let p = d.join("package.json");
    fs::write(&p, body).unwrap();
    p
}

#[test]
fn protocol_specs_ranges_peers_and_other_versions_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let lib = write_pkg(dir.path(), "lib", "{\"name\":\"lib\",\"version\":\"1.0.0\"}");
    let manifest = |optional: &str| {
        format!(
            "{{\n\t\"name\": \"app\",\n\t\"version\": \"1.0.0\",\n\t\"dependencies\": {{\n\t\t\"lib\": \"workspace:*\"\n\t}},\n\t\"devDependencies\": {{\n\t\t\"lib\": \"file:../lib\"\n\t}},\n\t\"optionalDependencies\": {{\n\t\t\"lib\": \"{optional}\"\n\t}},\n\t\"peerDependencies\": {{\n\t\t\"lib\": \"1.0.0\"\n\t}}\n}}"
        )
    };
    let app = write_pkg(dir.path(), "app", &manifest("^1.0.0"));
    apply_plan(
        &plan(&[("lib", release("1.0.0", "1.1.0"))]),
        &[target("lib", &["lib"], &[&lib]), target("app", &["app"], &[&app])],
    )
    .unwrap();
    // Tabs and the missing final newline are kept. Only the caret range moves.
    assert_eq!(fs::read_to_string(&app).unwrap(), manifest("^1.1.0"));
}

#[test]
fn key_order_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let lib = write_pkg(dir.path(), "lib", "{\n  \"version\": \"1.0.0\",\n  \"name\": \"lib\",\n  \"a\": 1\n}\n");
    apply_plan(&plan(&[("lib", release("1.0.0", "1.0.1"))]), &[target("lib", &["lib"], &[&lib])])
        .unwrap();
    assert_eq!(
        fs::read_to_string(&lib).unwrap(),
        "{\n  \"version\": \"1.0.1\",\n  \"name\": \"lib\",\n  \"a\": 1\n}\n"
    );
}

#[test]
fn cargo_workspace_versions_dependencies_and_inheritance_are_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Cargo.toml");
    fs::write(
        &root,
        "# workspace\n[workspace]\nmembers = [\"a\", \"b\"]\n\n[workspace.package]\nversion = \"0.1.0\"\n\n[workspace.dependencies]\na = { path = \"a\", version = \"0.1.0\" }\nserde = \"0.1.0\"\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("a")).unwrap();
    fs::create_dir_all(dir.path().join("b")).unwrap();
    let a = dir.path().join("a/Cargo.toml");
    let b = dir.path().join("b/Cargo.toml");
    fs::write(&a, "[package]\nname = \"a\"\nversion.workspace = true\n").unwrap();
    fs::write(
        &b,
        "[package]\nname = \"b\"\nversion = \"0.1.0\" # keep\n\n[dependencies]\na = { path = \"../a\", version = \"=0.1.0\" }\nz = \"0.1.0\"\n\n[dev-dependencies]\na2 = { package = \"a\", path = \"../a\", version = \"0.1.0\" }\n\n[target.'cfg(unix)'.build-dependencies]\na = { path = \"../a\", version = \"~0.1.0\" }\n",
    )
    .unwrap();
    apply_plan(
        &plan(&[("ws", release("0.1.0", "0.2.0"))]),
        &[target("ws", &["a", "b"], &[&root, &a, &b])],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(&root).unwrap(),
        "# workspace\n[workspace]\nmembers = [\"a\", \"b\"]\n\n[workspace.package]\nversion = \"0.2.0\"\n\n[workspace.dependencies]\na = { path = \"a\", version = \"0.2.0\" }\nserde = \"0.1.0\"\n"
    );
    assert_eq!(
        fs::read_to_string(&a).unwrap(),
        "[package]\nname = \"a\"\nversion.workspace = true\n"
    );
    assert_eq!(
        fs::read_to_string(&b).unwrap(),
        "[package]\nname = \"b\"\nversion = \"0.2.0\" # keep\n\n[dependencies]\na = { path = \"../a\", version = \"=0.2.0\" }\nz = \"0.1.0\"\n\n[dev-dependencies]\na2 = { package = \"a\", path = \"../a\", version = \"0.2.0\" }\n\n[target.'cfg(unix)'.build-dependencies]\na = { path = \"../a\", version = \"~0.2.0\" }\n"
    );
}

#[test]
fn unchanged_files_are_not_reported_or_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let other = write_pkg(dir.path(), "other", "{\"name\":\"other\",\"version\":\"3.0.0\"}");
    let before = fs::read_to_string(&other).unwrap();
    let changed = apply_plan(
        &plan(&[("lib", release("1.0.0", "1.1.0"))]),
        &[target("lib", &["lib"], &[]), target("other", &["other"], &[&other])],
    )
    .unwrap();
    assert!(changed.is_empty());
    assert_eq!(fs::read_to_string(&other).unwrap(), before);
}

#[test]
fn nothing_is_written_when_any_manifest_fails() {
    let dir = tempfile::tempdir().unwrap();
    let good = write_pkg(dir.path(), "good", "{\"name\":\"good\",\"version\":\"1.0.0\"}");
    let bad = write_pkg(dir.path(), "bad", "{not json");
    let before = fs::read_to_string(&good).unwrap();
    let err = apply_plan(
        &plan(&[("good", release("1.0.0", "1.0.1")), ("bad", release("1.0.0", "1.0.1"))]),
        &[target("good", &["good"], &[&good]), target("bad", &["bad"], &[&bad])],
    )
    .unwrap_err();
    assert!(matches!(err, ApplyError::Json { .. }));
    assert_eq!(fs::read_to_string(&good).unwrap(), before);
}

#[test]
fn a_released_project_whose_manifest_has_no_version_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let p = write_pkg(dir.path(), "p", "{\"name\":\"p\"}");
    let err = apply_plan(&plan(&[("p", release("1.0.0", "1.0.1"))]), &[target("p", &["p"], &[&p])])
        .unwrap_err();
    assert!(matches!(err, ApplyError::MissingVersion { .. }));
}

#[test]
fn an_unrecognised_manifest_name_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("pom.xml");
    fs::write(&p, "<x/>").unwrap();
    let err = apply_plan(&plan(&[("p", release("1.0.0", "1.0.1"))]), &[target("p", &["p"], &[&p])])
        .unwrap_err();
    assert!(matches!(err, ApplyError::UnsupportedManifest { .. }));
}

#[test]
fn a_plan_entry_without_a_target_is_an_error() {
    let err = apply_plan(&plan(&[("ghost", release("1.0.0", "1.0.1"))]), &[]).unwrap_err();
    assert!(matches!(err, ApplyError::UnknownProject { .. }));
}

#[test]
fn a_manifest_listed_under_two_targets_keeps_every_edit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Cargo.toml");
    fs::write(
        &root,
        "[workspace.package]\nversion = \"0.1.0\"\n\n[workspace.dependencies]\nc = { path = \"c\", version = \"1.0.0\" }\n",
    )
    .unwrap();
    apply_plan(
        &plan(&[("a", release("0.1.0", "0.2.0")), ("c", release("1.0.0", "1.1.0"))]),
        &[target("a", &["a"], &[&root]), target("b", &["b"], &[&root]), kind_target(ManifestKind::CargoToml, "c", &["c"], &[])],
    )
    .unwrap();
    let text = fs::read_to_string(&root).unwrap();
    assert!(text.contains("version = \"0.2.0\""), "{text}");
    assert!(text.contains("version = \"1.1.0\" }"), "{text}");
}

#[test]
fn crlf_line_endings_are_kept_in_package_json() {
    let dir = tempfile::tempdir().unwrap();
    let lib = write_pkg(dir.path(), "lib", "{\r\n  \"name\": \"lib\",\r\n  \"version\": \"1.0.0\"\r\n}\r\n");
    apply_plan(&plan(&[("lib", release("1.0.0", "1.0.1"))]), &[target("lib", &["lib"], &[&lib])])
        .unwrap();
    assert_eq!(
        fs::read_to_string(&lib).unwrap(),
        "{\r\n  \"name\": \"lib\",\r\n  \"version\": \"1.0.1\"\r\n}\r\n"
    );
}

#[test]
fn partial_cargo_requirements_follow_the_new_version() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("Cargo.toml");
    fs::write(
        &a,
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[dependencies]\nx = \"0.1\"\ny = { path = \"y\", version = \"^1\" }\nz = \"1.2\"\nw = \"0.2\"\n",
    )
    .unwrap();
    apply_plan(
        &plan(&[
            ("x", release("0.1.0", "0.2.0")),
            ("y", release("1.0.0", "2.0.0")),
            ("z", release("1.2.3", "1.3.0")),
        ]),
        &[
            target("x", &["x"], &[&a]),
            kind_target(ManifestKind::CargoToml, "y", &["y"], &[]),
            kind_target(ManifestKind::CargoToml, "z", &["z"], &[]),
        ],
    )
    .unwrap();
    // `0.1` and `^1` name the old version's leading components. `1.2` also covers
    // 1.3.0 only after the minor part moves, so it follows too. `w` is not released.
    assert_eq!(
        fs::read_to_string(&a).unwrap(),
        "[package]\nname = \"x\"\nversion = \"0.2.0\"\n\n[dependencies]\nx = \"0.2\"\ny = { path = \"y\", version = \"^2\" }\nz = \"1.3\"\nw = \"0.2\"\n"
    );
}

#[test]
fn equal_names_in_different_ecosystems_keep_their_own_releases() {
    let dir = tempfile::tempdir().unwrap();
    let app = write_pkg(
        dir.path(),
        "app",
        "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\",\n  \"dependencies\": {\n    \"utils\": \"^1.0.0\"\n  }\n}\n",
    );
    let js_utils = write_pkg(dir.path(), "utils", "{\"name\":\"utils\",\"version\":\"1.0.0\"}");
    let cargo_utils = dir.path().join("Cargo.toml");
    fs::write(&cargo_utils, "[package]\nname = \"utils\"\nversion = \"1.0.0\"\n").unwrap();
    apply_plan(
        &plan(&[
            ("js-utils", release("1.0.0", "2.0.0")),
            ("rs-utils", release("1.0.0", "1.1.0")),
        ]),
        &[
            target("app", &["app"], &[&app]),
            target("js-utils", &["utils"], &[&js_utils]),
            target("rs-utils", &["utils"], &[&cargo_utils]),
        ],
    )
    .unwrap();
    assert!(fs::read_to_string(&app).unwrap().contains("\"utils\": \"^2.0.0\""));
    assert!(fs::read_to_string(&cargo_utils).unwrap().contains("version = \"1.1.0\""));
}

#[test]
fn an_inherited_version_needs_the_manifest_that_defines_it() {
    let dir = tempfile::tempdir().unwrap();
    let member = dir.path().join("Cargo.toml");
    fs::write(&member, "[package]\nname = \"m\"\nversion.workspace = true\n").unwrap();
    let before = fs::read_to_string(&member).unwrap();
    let err = apply_plan(&plan(&[("ws", release("0.1.0", "0.2.0"))]), &[target("ws", &["m"], &[&member])])
        .unwrap_err();
    assert!(matches!(err, ApplyError::MissingVersion { .. }));
    assert_eq!(fs::read_to_string(&member).unwrap(), before);
}

#[test]
fn a_released_cargo_project_with_no_version_in_any_manifest_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Cargo.toml");
    fs::write(&root, "[workspace]\nmembers = []\n").unwrap();
    let err = apply_plan(&plan(&[("ws", release("0.1.0", "0.2.0"))]), &[target("ws", &["m"], &[&root])])
        .unwrap_err();
    assert!(matches!(err, ApplyError::MissingVersion { .. }));
}

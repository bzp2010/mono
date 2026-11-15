//! Every path that could reach outside the project must fail, and must leave the
//! outside untouched. Each test builds one adversarial layout and checks both.

use std::fs;
use std::path::PathBuf;

use mono_cache::{expand_files, remove_matching, CacheBackend, LocalBackend};

/// A project under `base/proj` and a sentinel directory `base/outside` that must
/// never change.
struct Layout {
    _base: tempfile::TempDir,
    project: PathBuf,
    outside: PathBuf,
    store: tempfile::TempDir,
}

fn layout() -> Layout {
    let base = tempfile::tempdir().unwrap();
    let project = base.path().join("proj");
    let outside = base.path().join("outside");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "outside").unwrap();
    let store = tempfile::tempdir().unwrap();
    Layout { _base: base, project, outside, store }
}

fn outside_unchanged(l: &Layout) -> bool {
    let mut names: Vec<_> = fs::read_dir(&l.outside)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names == vec!["secret.txt".to_string()]
        && fs::read_to_string(l.outside.join("secret.txt")).unwrap() == "outside"
}

fn running_as_root() -> bool {
    fs::read_to_string("/proc/self/status")
        .map(|s| s.lines().any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0")))
        .unwrap_or(false)
}

#[test]
fn parent_directory_pattern_is_rejected() {
    let l = layout();
    assert!(expand_files(&l.project, &["../outside/*".into()]).is_err());
    assert!(outside_unchanged(&l));
}

#[test]
fn character_class_that_matches_parent_is_rejected() {
    let l = layout();
    assert!(expand_files(&l.project, &[".[.]/outside/*".into()]).is_err());
    assert!(outside_unchanged(&l));
}

#[test]
fn absolute_pattern_is_rejected() {
    let l = layout();
    let pattern = format!("{}/*", l.outside.display());
    assert!(expand_files(&l.project, &[pattern]).is_err());
    assert!(outside_unchanged(&l));
}

#[test]
fn file_linked_to_outside_is_not_read_or_matched() {
    let l = layout();
    fs::create_dir_all(l.project.join("src")).unwrap();
    std::os::unix::fs::symlink(l.outside.join("secret.txt"), l.project.join("src/leak.txt")).unwrap();
    assert!(expand_files(&l.project, &["src/*.txt".into()]).is_err());
    assert!(outside_unchanged(&l));
}

#[test]
fn outputs_reached_through_a_linked_directory_are_not_removed() {
    // `dist` links outside the project. Removing the outputs must not delete the
    // sentinel file behind the link.
    let l = layout();
    std::os::unix::fs::symlink(&l.outside, l.project.join("dist")).unwrap();
    assert!(remove_matching(&l.project, &["dist/**".into()]).is_err());
    assert!(outside_unchanged(&l));
}

#[test]
fn removing_through_a_linked_ancestor_is_refused() {
    let l = layout();
    fs::write(l.outside.join("other.txt"), "x").unwrap();
    std::os::unix::fs::symlink(&l.outside, l.project.join("link")).unwrap();
    assert!(remove_matching(&l.project, &["link/other.txt".into()]).is_err());
    assert!(l.outside.join("other.txt").exists());
}

#[test]
fn storing_an_output_that_passes_through_a_link_fails_at_store_time() {
    // `dist` links to `build` inside the project. Removal would refuse this path, so
    // storing it must refuse too, instead of creating an entry that breaks later.
    let l = layout();
    fs::create_dir_all(l.project.join("build")).unwrap();
    fs::write(l.project.join("build/a.js"), "x").unwrap();
    std::os::unix::fs::symlink(l.project.join("build"), l.project.join("dist")).unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    let rel = vec![PathBuf::from("dist/a.js")];
    assert!(backend.put("kb", &l.project, &rel).is_err());
    assert!(!backend.contains("kb").unwrap());
}

#[test]
fn restoring_an_output_never_writes_through_a_linked_ancestor() {
    // The entry holds `pkg/item`. The project has `pkg` as a link to outside. The
    // restore must replace the link with a directory and must not write outside.
    let l = layout();
    fs::create_dir_all(l.project.join("pkg")).unwrap();
    fs::write(l.project.join("pkg/item"), "from entry").unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    backend.put("kr", &l.project, &[PathBuf::from("pkg/item")]).unwrap();

    fs::remove_dir_all(l.project.join("pkg")).unwrap();
    std::os::unix::fs::symlink(&l.outside, l.project.join("pkg")).unwrap();
    assert!(backend.get("kr", &l.project).unwrap().is_some());

    assert!(!fs::symlink_metadata(l.project.join("pkg")).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(l.project.join("pkg/item")).unwrap(), "from entry");
    assert!(outside_unchanged(&l));
}

#[test]
fn a_failed_link_removal_never_writes_through_the_link() {
    // The parent of the link is read-only, so replacing the link fails. The restore
    // must report the failure and must not write into the outside directory.
    use std::os::unix::fs::PermissionsExt;
    if running_as_root() {
        return; // root ignores the permission bits this test depends on
    }
    let l = layout();
    let real = l.project.join("ro/a");
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join("payload.txt"), "payload").unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    backend.put("kn1", &l.project, &[PathBuf::from("ro/a/payload.txt")]).unwrap();

    let ro = l.project.join("ro");
    fs::remove_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&l.outside, &real).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    let result = backend.get("kn1", &l.project);
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(result.is_err());
    assert!(!l.outside.join("payload.txt").exists());
    assert!(outside_unchanged(&l));
}

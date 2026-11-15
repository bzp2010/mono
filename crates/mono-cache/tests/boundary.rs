//! Every path that could reach outside the project must fail, and must leave the
//! outside untouched. Each test builds one adversarial layout and checks both.

use std::fs;
use std::path::{Path, PathBuf};

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
fn restoring_a_link_never_writes_through_it() {
    // The snapshot holds a link to outside. The destination holds a link at the same
    // name. The restore must replace that link, and must not write into the outside.
    let l = layout();
    let source = l.project.join("source");
    fs::create_dir_all(&source).unwrap();
    std::os::unix::fs::symlink(&l.outside, source.join("pkg")).unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    backend.put_dir("k", &l.project, Path::new("source")).unwrap();

    let dest = l.project.join("dest");
    fs::create_dir_all(&dest).unwrap();
    std::os::unix::fs::symlink(&l.outside, dest.join("pkg")).unwrap();
    backend.get_dir("k", &l.project, Path::new("dest")).unwrap();

    assert!(outside_unchanged(&l));
}

#[test]
fn removing_through_a_linked_ancestor_is_refused() {
    let l = layout();
    fs::write(l.outside.join("other.txt"), "x").unwrap();
    std::os::unix::fs::symlink(&l.outside, l.project.join("link")).unwrap();
    let err = mono_cache::remove_matching(&l.project, &["link/other.txt".into()]);
    assert!(err.is_err());
    assert!(l.outside.join("other.txt").exists());
}

#[test]
fn storing_a_tree_that_begins_at_a_link_is_refused() {
    let l = layout();
    std::os::unix::fs::symlink(&l.outside, l.project.join("node_modules")).unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    assert!(backend.put_dir("k", &l.project, Path::new("node_modules")).is_err());
    assert!(outside_unchanged(&l));
}

#[test]
fn restore_replaces_a_linked_ancestor_with_a_directory() {
    // cacheDirs `a/b` where `a` is a link to outside. The restore must replace the
    // link with a real directory, not fail and not remove the outside directory.
    let l = layout();
    let source = l.project.join("src");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("file.txt"), "from snapshot").unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    backend.put_dir("ka", &l.project, Path::new("src")).unwrap();

    std::os::unix::fs::symlink(&l.outside, l.project.join("a")).unwrap();
    assert!(backend.get_dir("ka", &l.project, Path::new("a/b")).unwrap());
    assert!(!fs::symlink_metadata(l.project.join("a")).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(l.project.join("a/b/file.txt")).unwrap(), "from snapshot");
    assert!(outside_unchanged(&l));
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
fn cache_dirs_are_validated_before_a_miss_returns() {
    let l = layout();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    assert!(backend.get_dir("never", &l.project, Path::new("../outside")).is_err());
    assert!(backend.get_dir("never", &l.project, Path::new("/etc")).is_err());
    assert!(backend.get_dir("never", &l.project, Path::new("")).is_err());
    assert!(backend.get_dir("never", &l.project, Path::new(".")).is_err());
}

#[test]
fn keys_must_be_plain_names() {
    let l = layout();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    assert!(backend.contains("/etc").is_err());
    assert!(backend.contains("../x").is_err());
}

#[test]
fn walking_a_file_is_reported_as_not_a_directory() {
    let l = layout();
    fs::write(l.project.join("plain.txt"), "x").unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    let err = backend.put_dir("kc", &l.project, Path::new("plain.txt")).unwrap_err();
    assert!(matches!(err, mono_cache::CacheError::NotADirectory { .. }), "{err}");
}

#[test]
fn removing_an_output_through_a_link_inside_the_project_is_refused() {
    // `dist` links to `build` inside the project. Expansion accepts the file, but
    // removal must still refuse to traverse the link.
    let l = layout();
    fs::create_dir_all(l.project.join("build")).unwrap();
    fs::write(l.project.join("build/a.js"), "x").unwrap();
    std::os::unix::fs::symlink(l.project.join("build"), l.project.join("dist")).unwrap();
    assert!(remove_matching(&l.project, &["dist/a.js".into()]).is_err());
    assert!(l.project.join("build/a.js").exists());
}

#[test]
fn empty_directories_survive_a_round_trip() {
    let l = layout();
    let tree = l.project.join("tree");
    fs::create_dir_all(tree.join("empty/nested")).unwrap();
    fs::write(tree.join("file.txt"), "x").unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    backend.put_dir("kd", &l.project, Path::new("tree")).unwrap();
    fs::remove_dir_all(&tree).unwrap();
    assert!(backend.get_dir("kd", &l.project, Path::new("tree")).unwrap());
    assert!(tree.join("empty/nested").is_dir());
}

fn running_as_root() -> bool {
    fs::read_to_string("/proc/self/status")
        .map(|s| s.lines().any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0")))
        .unwrap_or(false)
}

#[test]
fn a_failed_link_removal_never_writes_through_the_link() {
    // The parent is read-only, so removing the link inside it fails. The restore
    // must report the failure and must not write into the outside directory.
    use std::os::unix::fs::PermissionsExt;
    if running_as_root() {
        return; // root ignores the permission bits this test depends on
    }
    let l = layout();
    let ro = l.project.join("ro");
    fs::create_dir_all(&ro).unwrap();
    std::os::unix::fs::symlink(&l.outside, ro.join("a")).unwrap();
    let backend = LocalBackend::new(l.store.path().to_path_buf());
    let src = l.project.join("src");
    fs::create_dir_all(src.join("b")).unwrap();
    fs::write(src.join("b/payload.txt"), "payload").unwrap();
    backend.put_dir("kn1", &l.project, Path::new("src")).unwrap();

    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    let result = backend.get_dir("kn1", &l.project, Path::new("ro/a"));
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(result.is_err());
    assert!(!l.outside.join("b").exists(), "nothing may be written through the link");
    assert!(outside_unchanged(&l));
}

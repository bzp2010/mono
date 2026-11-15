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
    // The snapshot holds a link to outside and a file beneath the same name. The
    // restore must replace the link with a directory, not write into the outside.
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

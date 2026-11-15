use std::fs;
use std::path::PathBuf;

use mono_cache::{CacheBackend, LocalBackend};

#[test]
fn put_then_get_restores_outputs() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    fs::create_dir_all(project.path().join("dist/nested")).unwrap();
    fs::write(project.path().join("dist/index.js"), "one").unwrap();
    fs::write(project.path().join("dist/nested/b.js"), "two").unwrap();
    let rel = vec![PathBuf::from("dist/index.js"), PathBuf::from("dist/nested/b.js")];

    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put("k1", project.path(), &rel).unwrap();
    fs::remove_dir_all(project.path().join("dist")).unwrap();

    let restored = backend.get("k1", project.path()).unwrap();
    assert_eq!(restored.map(|mut v| { v.sort(); v }), Some(rel));
    assert_eq!(fs::read_to_string(project.path().join("dist/index.js")).unwrap(), "one");
    assert_eq!(fs::read_to_string(project.path().join("dist/nested/b.js")).unwrap(), "two");
}

#[test]
fn get_misses_for_unknown_key() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    assert!(backend.get("never-stored", project.path()).unwrap().is_none());
    assert!(!backend.contains("never-stored").unwrap());
}

#[test]
fn failed_put_leaves_no_entry() {
    // A put that fails half way must not leave an entry that a later get would
    // restore as if it were complete.
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    fs::write(project.path().join("present.txt"), "x").unwrap();
    let rel = vec![PathBuf::from("present.txt"), PathBuf::from("missing.txt")];

    let backend = LocalBackend::new(store.path().to_path_buf());
    assert!(backend.put("k2", project.path(), &rel).is_err());
    assert!(!backend.contains("k2").unwrap());
    assert!(backend.get("k2", project.path()).unwrap().is_none());
}

#[test]
fn first_put_for_a_key_wins() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let rel = vec![PathBuf::from("out.txt")];
    let backend = LocalBackend::new(store.path().to_path_buf());

    fs::write(project.path().join("out.txt"), "first").unwrap();
    backend.put("kw", project.path(), &rel).unwrap();
    fs::write(project.path().join("out.txt"), "second").unwrap();
    backend.put("kw", project.path(), &rel).unwrap();

    fs::remove_file(project.path().join("out.txt")).unwrap();
    backend.get("kw", project.path()).unwrap();
    assert_eq!(fs::read_to_string(project.path().join("out.txt")).unwrap(), "first");
}

#[test]
fn a_user_file_named_like_an_old_marker_is_stored_and_restored() {
    // There is no marker any more, so a project file with that name is ordinary data.
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    fs::write(project.path().join(".mono-complete"), "user data").unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put("km", project.path(), &[PathBuf::from(".mono-complete")]).unwrap();
    fs::remove_file(project.path().join(".mono-complete")).unwrap();
    assert!(backend.get("km", project.path()).unwrap().is_some());
    assert_eq!(fs::read_to_string(project.path().join(".mono-complete")).unwrap(), "user data");
}

#[test]
fn a_missing_destination_is_an_error_not_a_miss() {
    // The entry is present. The destination root does not exist, which is a bug in
    // the caller and must surface instead of looking like a cache miss.
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    fs::write(project.path().join("a.txt"), "x").unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put("kt", project.path(), &[PathBuf::from("a.txt")]).unwrap();
    let missing = project.path().join("does-not-exist");
    assert!(backend.get("kt", &missing).is_err());
}

#[test]
fn keys_must_be_plain_names() {
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    assert!(backend.contains("/etc").is_err());
    assert!(backend.contains("../x").is_err());
}

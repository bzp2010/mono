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

    assert!(backend.get("k1", project.path()).unwrap());
    assert_eq!(fs::read_to_string(project.path().join("dist/index.js")).unwrap(), "one");
    assert_eq!(fs::read_to_string(project.path().join("dist/nested/b.js")).unwrap(), "two");
}

#[test]
fn get_misses_for_unknown_key() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    assert!(!backend.get("never-stored", project.path()).unwrap());
}

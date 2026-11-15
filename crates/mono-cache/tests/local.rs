use std::path::Path;
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
    assert!(!backend.get("k2", project.path()).unwrap());
}

#[test]
fn failed_put_dir_leaves_no_entry() {
    // A named pipe cannot be cached. The walk fails part way, so no entry may appear.
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let target = project.path().join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.bin"), "x").unwrap();
    let fifo = target.join("pipe");
    let status = std::process::Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success());

    let backend = LocalBackend::new(store.path().to_path_buf());
    assert!(backend.put_dir("k3", project.path(), Path::new("target")).is_err());
    assert!(!backend.contains("k3").unwrap());
}

use std::fs;

use mono_cache::{CacheBackend, LocalBackend};

#[test]
fn directory_round_trips_as_a_whole() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let target_dir = project.path().join("target");
    fs::create_dir_all(target_dir.join("debug/deps")).unwrap();
    fs::write(target_dir.join("debug/deps/libx.rlib"), "bits").unwrap();

    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k", &target_dir).unwrap();
    fs::remove_dir_all(&target_dir).unwrap();

    assert!(backend.get_dir("k", &target_dir).unwrap());
    assert_eq!(fs::read_to_string(target_dir.join("debug/deps/libx.rlib")).unwrap(), "bits");
}

#[test]
fn missing_source_directory_is_not_an_error() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k", &project.path().join("absent")).unwrap();
    assert!(!backend.get_dir("k", &project.path().join("absent")).unwrap());
}

#[test]
fn get_dir_misses_for_unknown_key() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    assert!(!backend.get_dir("never", &project.path().join("target")).unwrap());
}

#[test]
fn restore_succeeds_over_a_running_executable() {
    // Restoring must not fail when the destination is a binary that is running
    // (the self-hosted `target/` contains the mono binary itself).
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let target_dir = project.path().join("target");
    fs::create_dir_all(target_dir.join("bin")).unwrap();
    fs::copy("/bin/sleep", target_dir.join("bin/prog")).unwrap();

    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k", &target_dir).unwrap();

    let mut running = std::process::Command::new(target_dir.join("bin/prog"))
        .arg("5")
        .spawn()
        .unwrap();
    let restored = backend.get_dir("k", &target_dir);
    let _ = running.kill();
    let _ = running.wait();

    assert!(restored.unwrap());
}

#[test]
fn symlinks_round_trip_without_being_followed() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let modules = project.path().join("node_modules");
    fs::create_dir_all(modules.join(".bin")).unwrap();
    fs::create_dir_all(modules.join("typescript/bin")).unwrap();
    fs::write(modules.join("typescript/bin/tsc"), "compiler").unwrap();
    std::os::unix::fs::symlink("../typescript/bin/tsc", modules.join(".bin/tsc")).unwrap();

    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k4", &modules).unwrap();
    fs::remove_dir_all(&modules).unwrap();
    assert!(backend.get_dir("k4", &modules).unwrap());

    let link = modules.join(".bin/tsc");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_link(&link).unwrap(), std::path::PathBuf::from("../typescript/bin/tsc"));
    assert_eq!(fs::read_to_string(&link).unwrap(), "compiler");
}

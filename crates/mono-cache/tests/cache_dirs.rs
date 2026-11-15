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

#[test]
fn restored_files_keep_their_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let bin = project.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("tool"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(bin.join("tool"), fs::Permissions::from_mode(0o755)).unwrap();

    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k5", &bin).unwrap();
    fs::remove_dir_all(&bin).unwrap();
    assert!(backend.get_dir("k5", &bin).unwrap());

    let mode = fs::metadata(bin.join("tool")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
}

#[test]
fn concurrent_restores_of_one_entry_all_succeed() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let dir = project.path().join("out");
    fs::create_dir_all(&dir).unwrap();
    for i in 0..50 {
        fs::write(dir.join(format!("f{i}.bin")), vec![i as u8; 4096]).unwrap();
    }
    let backend = std::sync::Arc::new(LocalBackend::new(store.path().to_path_buf()));
    backend.put_dir("k6", &dir).unwrap();

    let dest = project.path().join("restored");
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let backend = backend.clone();
            let dest = dest.clone();
            std::thread::spawn(move || {
                for _ in 0..20 {
                    backend.get_dir("k6", &dest).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(fs::read(dest.join("f7.bin")).unwrap(), vec![7u8; 4096]);
}

#[test]
fn restore_replaces_a_directory_where_the_snapshot_has_a_link() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let modules = project.path().join("node_modules");
    fs::create_dir_all(modules.join("typescript")).unwrap();
    fs::write(modules.join("typescript/tsc"), "compiler").unwrap();
    std::os::unix::fs::symlink("typescript", modules.join("pkg")).unwrap();

    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k7", &modules).unwrap();

    // The destination now has a real directory at the place of the link.
    fs::remove_file(modules.join("pkg")).unwrap();
    fs::create_dir_all(modules.join("pkg")).unwrap();
    fs::write(modules.join("pkg/stale"), "x").unwrap();

    assert!(backend.get_dir("k7", &modules).unwrap());
    assert!(fs::symlink_metadata(modules.join("pkg")).unwrap().file_type().is_symlink());
}

#[test]
fn restore_replaces_a_directory_where_the_snapshot_has_a_file() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let tree = project.path().join("tree");
    fs::create_dir_all(&tree).unwrap();
    fs::write(tree.join("item"), "file").unwrap();

    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k8", &tree).unwrap();

    fs::remove_file(tree.join("item")).unwrap();
    fs::create_dir_all(tree.join("item")).unwrap();
    fs::write(tree.join("item/inner"), "x").unwrap();

    assert!(backend.get_dir("k8", &tree).unwrap());
    assert_eq!(fs::read_to_string(tree.join("item")).unwrap(), "file");
}

#[test]
fn restore_never_writes_through_a_symlinked_ancestor() {
    // The destination's `pkg` is a link to a directory outside the tree. The
    // snapshot wants `pkg` to be a real directory containing a file `item`.
    // Nothing outside the tree may be touched.
    let base = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let source = base.path().join("source");
    fs::create_dir_all(source.join("pkg")).unwrap();
    fs::write(source.join("pkg/item"), "from snapshot").unwrap();
    let backend = LocalBackend::new(store.path().to_path_buf());
    backend.put_dir("k9", &source).unwrap();

    let external = base.path().join("external");
    fs::create_dir_all(external.join("item")).unwrap();
    fs::write(external.join("item/keep.txt"), "outside").unwrap();
    let dest = base.path().join("dest");
    fs::create_dir_all(&dest).unwrap();
    std::os::unix::fs::symlink(&external, dest.join("pkg")).unwrap();

    assert!(backend.get_dir("k9", &dest).unwrap());

    assert_eq!(fs::read_to_string(external.join("item/keep.txt")).unwrap(), "outside");
    assert!(!fs::symlink_metadata(dest.join("pkg")).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(dest.join("pkg/item")).unwrap(), "from snapshot");
}

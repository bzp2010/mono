//! Concurrent publishes and restores must neither fail spuriously nor expose a
//! partial entry to a reader.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use mono_cache::{CacheBackend, LocalBackend};

fn project_with_files(root: &Path, count: usize) {
    fs::create_dir_all(root.join("out")).unwrap();
    for i in 0..count {
        fs::write(root.join("out").join(format!("f{i}.bin")), vec![i as u8; 1024]).unwrap();
    }
}

#[test]
fn concurrent_publishes_of_one_key_all_succeed() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    project_with_files(project.path(), 20);
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    let root: PathBuf = project.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (backend, root, barrier) = (backend.clone(), root.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..10 {
                    backend.put_dir("pub", &root, Path::new("out")).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert!(backend.contains("pub").unwrap());
}

#[test]
fn readers_never_see_a_partial_entry_while_it_is_republished() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    project_with_files(project.path(), 50);
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    backend.put_dir("live", project.path(), Path::new("out")).unwrap();
    let root: PathBuf = project.path().to_path_buf();

    let writer = {
        let (backend, root) = (backend.clone(), root.clone());
        std::thread::spawn(move || {
            for _ in 0..30 {
                backend.put_dir("live", &root, Path::new("out")).unwrap();
            }
        })
    };
    let readers: Vec<_> = (0..4)
        .map(|i| {
            let (backend, root) = (backend.clone(), root.clone());
            std::thread::spawn(move || {
                // Each reader has its own destination, so readers do not interfere.
                let rel = format!("restored-{i}");
                let dest = root.join(&rel);
                for _ in 0..30 {
                    // A restore into a fresh destination must find the complete entry.
                    let _ = fs::remove_dir_all(&dest);
                    assert!(backend.get_dir("live", &root, Path::new(&rel)).unwrap());
                    assert_eq!(fs::read_dir(&dest).unwrap().count(), 50);
                }
            })
        })
        .collect();
    writer.join().unwrap();
    for r in readers {
        r.join().unwrap();
    }
}

#[test]
fn concurrent_restores_over_placeholders_all_succeed() {
    // The destination holds placeholders where the snapshot has directories and
    // files. The placeholders are created once, then every thread restores into
    // them at the same time. Each restore must replace them and succeed.
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    fs::create_dir_all(project.path().join("tree/d")).unwrap();
    for i in 0..20 {
        fs::write(project.path().join(format!("tree/d/f{i}")), "x").unwrap();
    }
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    backend.put_dir("ph", project.path(), Path::new("tree")).unwrap();

    let root: PathBuf = project.path().to_path_buf();
    fs::create_dir_all(root.join("dest")).unwrap();
    fs::write(root.join("dest/d"), "placeholder file").unwrap();

    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (backend, root, barrier) = (backend.clone(), root.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..10 {
                    backend.get_dir("ph", &root, Path::new("dest")).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert!(root.join("dest/d").is_dir());
    assert_eq!(fs::read_dir(root.join("dest/d")).unwrap().count(), 20);
}

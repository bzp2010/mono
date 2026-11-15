//! Concurrent publishes and restores of output entries. An entry is immutable and
//! appears in one rename, so a reader sees either no entry or a complete one.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use mono_cache::{CacheBackend, LocalBackend};

fn project_with_files(root: &Path, count: usize) -> Vec<PathBuf> {
    fs::create_dir_all(root.join("out")).unwrap();
    (0..count)
        .map(|i| {
            let rel = PathBuf::from(format!("out/f{i}.bin"));
            fs::write(root.join(&rel), vec![i as u8; 1024]).unwrap();
            rel
        })
        .collect()
}

#[test]
fn concurrent_publishes_of_one_key_all_succeed() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let rel = project_with_files(project.path(), 20);
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    let root: PathBuf = project.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (backend, root, barrier, rel) = (backend.clone(), root.clone(), barrier.clone(), rel.clone());
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..10 {
                    backend.put("pub", &root, &rel).unwrap();
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
fn readers_see_a_complete_entry_while_it_is_being_published() {
    // The entry exists before any reader starts, and publishing is first-wins, so
    // every restore must be a hit that restores all fifty files.
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let rel = project_with_files(project.path(), 50);
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    let root: PathBuf = project.path().to_path_buf();
    backend.put("live", &root, &rel).unwrap();

    let writer = {
        let (backend, root, rel) = (backend.clone(), root.clone(), rel.clone());
        std::thread::spawn(move || {
            for _ in 0..30 {
                backend.put("live", &root, &rel).unwrap();
            }
        })
    };
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let (backend, root) = (backend.clone(), root.clone());
            std::thread::spawn(move || {
                let mut hits = 0;
                for _ in 0..30 {
                    let files = backend.get("live", &root).unwrap();
                    let files = files.expect("the entry exists, so every restore is a hit");
                    assert_eq!(files.len(), 50, "a hit must restore every file");
                    hits += 1;
                }
                assert_eq!(hits, 30);
            })
        })
        .collect();
    writer.join().unwrap();
    for r in readers {
        r.join().unwrap();
    }
}

#[test]
fn outputs_are_immutable_once_published() {
    // A winner publishes first. Then twelve writers, each with its own source and
    // different bytes, publish under the same key. Every later restore must still
    // return the winner's bytes.
    let base = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    let rel = vec![PathBuf::from("out.txt")];

    let winner = base.path().join("winner");
    fs::create_dir_all(&winner).unwrap();
    fs::write(winner.join("out.txt"), "winner").unwrap();
    backend.put("imm", &winner, &rel).unwrap();

    let barrier = Arc::new(Barrier::new(12));
    let handles: Vec<_> = (0..12u8)
        .map(|i| {
            let (backend, barrier, rel) = (backend.clone(), barrier.clone(), rel.clone());
            let dir = base.path().join(format!("writer-{i}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("out.txt"), format!("writer {i}")).unwrap();
            std::thread::spawn(move || {
                barrier.wait();
                backend.put("imm", &dir, &rel).unwrap();
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    for _ in 0..20 {
        let restored = base.path().join("restored");
        let _ = fs::remove_dir_all(&restored);
        fs::create_dir_all(&restored).unwrap();
        assert!(backend.get("imm", &restored).unwrap().is_some());
        assert_eq!(fs::read_to_string(restored.join("out.txt")).unwrap(), "winner");
    }
}

#[test]
fn concurrent_restores_replace_placeholders_with_files() {
    // The destination holds a directory where the entry has a file. Every thread
    // restores at once, and every restore must succeed.
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let rel = project_with_files(project.path(), 20);
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    let root: PathBuf = project.path().to_path_buf();
    backend.put("ph", &root, &rel).unwrap();

    // Placeholders: a directory where each output file belongs.
    for r in &rel {
        fs::remove_file(root.join(r)).unwrap();
        fs::create_dir_all(root.join(r)).unwrap();
        fs::write(root.join(r).join("inner"), "placeholder").unwrap();
    }
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (backend, root, barrier) = (backend.clone(), root.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..10 {
                    assert!(backend.get("ph", &root).unwrap().is_some());
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    for r in &rel {
        assert!(root.join(r).is_file());
    }
}

#[test]
fn publishes_of_one_key_never_fail_however_many_race() {
    let project = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let rel = project_with_files(project.path(), 10);
    let root: PathBuf = project.path().to_path_buf();
    let backend = Arc::new(LocalBackend::new(store.path().to_path_buf()));
    let barrier = Arc::new(Barrier::new(48));
    let handles: Vec<_> = (0..48)
        .map(|_| {
            let (backend, root, barrier, rel) = (backend.clone(), root.clone(), barrier.clone(), rel.clone());
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..20 {
                    backend.put("many", &root, &rel).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert!(backend.contains("many").unwrap());
}

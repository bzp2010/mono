use std::fs;
use std::path::Path;

use mono_cache::task_key;
use mono_core::TargetConfig;
use mono_graph::TaskId;

fn target(inputs: &[&str]) -> TargetConfig {
    TargetConfig {
        command: "tsc -b".into(),
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
        outputs: vec!["dist/**".into()],
        depends_on: vec![],
        cache_dirs: vec![],
    }
}

fn task() -> TaskId {
    TaskId { project: "a".into(), target: "build".into() }
}

fn key(root: &Path, t: &TargetConfig) -> String {
    task_key(&task(), t, root, &[]).unwrap()
}

#[test]
fn same_inputs_give_same_key() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.ts"), "export {};").unwrap();
    let t = target(&["src/**/*.ts"]);
    assert_eq!(key(dir.path(), &t), key(dir.path(), &t));
}

#[test]
fn changed_input_changes_key() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.ts"), "export {};").unwrap();
    let t = target(&["src/**/*.ts"]);
    let before = key(dir.path(), &t);
    fs::write(dir.path().join("src/a.ts"), "export const x = 1;").unwrap();
    assert_ne!(before, key(dir.path(), &t));
}

#[test]
fn unmatched_glob_is_empty_and_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let t = target(&["nothing/**/*.rs"]);
    assert_eq!(key(dir.path(), &t), key(dir.path(), &t));
}

#[test]
fn dependency_contribution_changes_key() {
    let dir = tempfile::tempdir().unwrap();
    let t = target(&[]);
    let a = task_key(&task(), &t, dir.path(), &["x".to_string()]).unwrap();
    let b = task_key(&task(), &t, dir.path(), &["y".to_string()]).unwrap();
    assert_ne!(a, b);
}

#[test]
fn trailing_double_star_matches_files_beneath_directory() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("dist/nested")).unwrap();
    fs::write(dir.path().join("dist/out.txt"), "x").unwrap();
    fs::write(dir.path().join("dist/nested/b.txt"), "x").unwrap();
    let files = mono_cache::expand_files(dir.path(), &["dist/**".to_string()]).unwrap();
    assert_eq!(
        files,
        vec![
            std::path::PathBuf::from("dist/nested/b.txt"),
            std::path::PathBuf::from("dist/out.txt"),
        ]
    );
}

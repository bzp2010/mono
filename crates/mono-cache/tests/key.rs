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

#[test]
fn glob_metacharacters_in_project_root_do_not_hide_inputs() {
    // A root such as `repo[1]` must be matched literally, not as a character class.
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("repo[1]");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/a.txt"), "x").unwrap();
    let files = mono_cache::expand_files(&root, &["src/*.txt".to_string()]).unwrap();
    assert_eq!(files, vec![std::path::PathBuf::from("src/a.txt")]);
}

#[test]
fn changing_declared_outputs_changes_key() {
    // Restoring a snapshot taken under other output declarations would be wrong.
    let dir = tempfile::tempdir().unwrap();
    let mut t = target(&[]);
    t.outputs = vec!["dist/a".into()];
    let before = key(dir.path(), &t);
    t.outputs = vec!["dist/b".into()];
    assert_ne!(before, key(dir.path(), &t));
}

#[test]
fn patterns_escaping_the_project_root_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    assert!(mono_cache::expand_files(dir.path(), &["../shared/**".to_string()]).is_err());
    assert!(mono_cache::expand_files(dir.path(), &["/etc/*".to_string()]).is_err());
}

#[test]
fn glob_that_expands_outside_the_root_is_rejected() {
    // `.[.]` is a character class that matches `..`, so the literal pattern looks
    // relative while its matches leave the project root.
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("proj");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(base.path().join("shared")).unwrap();
    fs::write(base.path().join("shared/a.txt"), "x").unwrap();
    let result = mono_cache::expand_files(&root, &[".[.]/shared/*.txt".to_string()]);
    assert!(result.is_err());
}

#[test]
fn a_fifo_matched_by_an_output_pattern_is_an_error() {
    // Silently skipping a special file would cache an incomplete output set.
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("dist")).unwrap();
    fs::write(dir.path().join("dist/a"), "x").unwrap();
    let status = std::process::Command::new("mkfifo").arg(dir.path().join("dist/pipe")).status().unwrap();
    assert!(status.success());
    assert!(mono_cache::expand_files(dir.path(), &["dist/*".to_string()]).is_err());
}

use mono_core::{load_project_config, parse_project_config};

#[test]
fn parses_full_target() {
    let json = r#"{
        "dependencies": ["a"],
        "targets": {
            "build": {
                "command": "tsc -b",
                "inputs": ["src/**/*.ts"],
                "outputs": ["dist/**"],
                "dependsOn": ["^build"]
            }
        }
    }"#;
    let cfg = parse_project_config(json).unwrap();
    assert_eq!(cfg.dependencies, vec!["a".to_string()]);
    let build = &cfg.targets["build"];
    assert_eq!(build.command, "tsc -b");
    assert_eq!(build.depends_on, vec!["^build".to_string()]);
}

#[test]
fn optional_fields_default_to_empty() {
    let cfg = parse_project_config(r#"{"targets":{"dev":{"command":"vite"}}}"#).unwrap();
    let dev = &cfg.targets["dev"];
    assert!(dev.inputs.is_empty() && dev.outputs.is_empty());
    assert!(dev.depends_on.is_empty());
    assert!(cfg.dependencies.is_empty());
}

#[test]
fn target_without_command_is_error() {
    assert!(parse_project_config(r#"{"targets":{"build":{}}}"#).is_err());
}

#[test]
fn rejects_native_tool_field() {
    let json = r#"{"targets":{"build":{"command":"tsc","lint":true}}}"#;
    assert!(parse_project_config(json).is_err());
}

#[test]
fn missing_project_file_is_none() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load_project_config(dir.path()).unwrap().is_none());
}

#[test]
fn rejects_cache_dirs() {
    // Only outputs are cached. A directory cache is not part of the model.
    let json = r#"{"targets":{"build":{"command":"cargo build","cacheDirs":["target"]}}}"#;
    assert!(parse_project_config(json).is_err());
}

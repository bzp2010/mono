use mono_core::{parse_root_config, Ecosystem};

#[test]
fn parses_ecosystems() {
    let cfg = parse_root_config(r#"{"ecosystems":["cargo","pnpm"]}"#).unwrap();
    assert_eq!(cfg.ecosystems, vec![Ecosystem::Cargo, Ecosystem::Pnpm]);
}

#[test]
fn rejects_unknown_ecosystem() {
    assert!(parse_root_config(r#"{"ecosystems":["gradle"]}"#).is_err());
}

#[test]
fn missing_file_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let err = mono_core::load_root_config(dir.path()).unwrap_err();
    assert!(matches!(err, mono_core::ConfigError::NotFound { .. }));
}

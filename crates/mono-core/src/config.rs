//! Root configuration read from `mono.json`.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

/// Root configuration of a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonoConfig {
    /// Ecosystems whose workspace signals are read during discovery.
    pub ecosystems: Vec<Ecosystem>,
}

/// Supported ecosystems. Names are lowercase in `mono.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Cargo,
    Pnpm,
    Npm,
}

/// Errors raised while loading configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration file not found: {path}")]
    NotFound { path: PathBuf },
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid configuration: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Parses the contents of a `mono.json` file.
pub fn parse_root_config(json: &str) -> Result<MonoConfig, ConfigError> {
    Ok(serde_json::from_str(json)?)
}

/// Loads `mono.json` from the given workspace root.
pub fn load_root_config(root: &Path) -> Result<MonoConfig, ConfigError> {
    let path = root.join("mono.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ConfigError::NotFound { path })
        }
        Err(source) => return Err(ConfigError::Io { path, source }),
    };
    parse_root_config(&text)
}

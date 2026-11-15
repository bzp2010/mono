//! Per-project configuration read from `project.json`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::config::ConfigError;

/// Contents of a `project.json` file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// Explicit dependencies on other projects, merged with inferred ones.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Targets keyed by name. Each target is a shell command.
    #[serde(default)]
    pub targets: BTreeMap<String, TargetConfig>,
}

/// One target: a shell command plus its caching declarations.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetConfig {
    pub command: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    /// `^name` refers to the same target in dependencies, `name` to this project.
    #[serde(rename = "dependsOn", default)]
    pub depends_on: Vec<String>,
    /// Directories saved and restored as a whole, never hashed.
    #[serde(rename = "cacheDirs", default)]
    pub cache_dirs: Vec<String>,
}

/// Parses the contents of a `project.json` file.
pub fn parse_project_config(json: &str) -> Result<ProjectConfig, ConfigError> {
    Ok(serde_json::from_str(json)?)
}

/// Loads `project.json` from a project directory, or `None` if it is absent.
pub fn load_project_config(dir: &Path) -> Result<Option<ProjectConfig>, ConfigError> {
    let path = dir.join("project.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_project_config(&text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Io { path, source }),
    }
}

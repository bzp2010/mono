//! Workspace discovery and loading shared by all subcommands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mono_core::{load_project_config, load_root_config, ConfigError, ProjectConfig};
use mono_discover::{discover, DiscoverError};
use mono_graph::{build, GraphError, ProjectGraph};
use mono_task::RunError;
use thiserror::Error;

/// Errors surfaced to the user. Each maps to exit code 2.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("no mono.json found in {0} or any parent directory")]
    NoWorkspace(PathBuf),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Discover(#[from] DiscoverError),
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Run(#[from] RunError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A loaded workspace. Fields are added as subcommands need them.
pub struct Workspace {
    pub graph: ProjectGraph,
}

/// Walks up from `start` to the first directory containing `mono.json`.
pub fn find_workspace_root(start: &Path) -> Result<PathBuf, CliError> {
    let mut dir = start;
    loop {
        if dir.join("mono.json").is_file() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return Err(CliError::NoWorkspace(start.to_path_buf())),
        }
    }
}

/// Reads the root config, discovers projects, merges their `project.json`
/// files, and builds the graph.
pub fn load_workspace(root: &Path) -> Result<Workspace, CliError> {
    let config = load_root_config(root)?;
    let discovered = discover(root, &config.ecosystems)?;
    let mut configs: BTreeMap<String, ProjectConfig> = BTreeMap::new();
    for project in &discovered {
        if let Some(cfg) = load_project_config(&project.root)? {
            configs.insert(project.name.clone(), cfg);
        }
    }
    let graph = build(discovered, &configs)?;
    Ok(Workspace { graph })
}

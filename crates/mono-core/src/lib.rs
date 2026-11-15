//! Configuration and project model shared by all mono crates.

pub mod config;
pub mod model;
pub mod project;

pub use config::{load_root_config, parse_root_config, ConfigError, Ecosystem, MonoConfig};
pub use model::DiscoveredProject;
pub use project::{load_project_config, parse_project_config, ProjectConfig, TargetConfig};

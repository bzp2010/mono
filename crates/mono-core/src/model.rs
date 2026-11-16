//! Project model produced by ecosystem discovery.

use std::path::PathBuf;

use crate::config::Ecosystem;

/// A project found by discovery, before `project.json` is merged in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredProject {
    /// The name the ecosystem gives the project, if it has one. A project without a
    /// name must be given one in its `project.json`.
    pub name: Option<String>,
    /// The name other packages use for this project in dependency specs, if the
    /// ecosystem has one. Dependencies are recorded with these names.
    pub package_name: Option<String>,
    pub root: PathBuf,
    pub ecosystem: Ecosystem,
    /// Dependencies inferred from the ecosystem's own workspace signals, as package names.
    pub dependencies: Vec<String>,
    /// The version the ecosystem declares for the project, if it declares one.
    pub version: Option<String>,
}

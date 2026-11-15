//! Project model produced by ecosystem discovery.

use std::path::PathBuf;

use crate::config::Ecosystem;

/// A project found by discovery, before explicit config is merged in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredProject {
    pub name: String,
    pub root: PathBuf,
    pub ecosystem: Ecosystem,
    /// Dependencies inferred from the ecosystem's own workspace signals.
    pub dependencies: Vec<String>,
}

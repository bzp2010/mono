//! Cache keys and pluggable cache backends.
//!
//! Every path inside a project is handled through a confined tree, so a
//! pattern, a cache entry, or a link cannot read, write, or remove anything
//! outside the project directory.

use std::path::{Path, PathBuf};

use thiserror::Error;

mod confine;
mod key;
mod local;

pub use key::{dir_key, expand_files, output_digest, remove_matching, task_key};
pub use local::LocalBackend;

/// Errors raised by cache keys and backends.
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid glob {pattern}: {message}")]
    Glob { pattern: String, message: String },
    #[error("{pattern} must be relative to the project root, without .., and must resolve inside it")]
    OutsideProject { pattern: String },
    #[error("{path} is a symbolic link, and mono does not write or remove through links")]
    ThroughLink { path: PathBuf },
}

/// Storage for task outputs, addressed by cache key. Every relative path is
/// interpreted inside `project_root`.
pub trait CacheBackend {
    /// Whether a complete entry exists for `key`.
    fn contains(&self, key: &str) -> Result<bool, CacheError>;
    /// Restores the entry for `key` into `project_root`. Returns `false` on a miss.
    fn get(&self, key: &str, project_root: &Path) -> Result<bool, CacheError>;
    /// Stores the files at `rel_paths` under `key`.
    fn put(&self, key: &str, project_root: &Path, rel_paths: &[PathBuf]) -> Result<(), CacheError>;
    /// Restores the tree stored under `key` into `rel_dir` of `project_root`.
    /// Returns `false` on a miss.
    fn get_dir(&self, key: &str, project_root: &Path, rel_dir: &Path) -> Result<bool, CacheError>;
    /// Stores the tree at `rel_dir` of `project_root` under `key`. A missing
    /// directory is a no-op.
    fn put_dir(&self, key: &str, project_root: &Path, rel_dir: &Path) -> Result<(), CacheError>;
}

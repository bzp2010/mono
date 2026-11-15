//! Cache keys and pluggable cache backends.
//!
//! Every path inside a project is handled through a confined tree, so a
//! pattern, a cache entry, or a link cannot read, write, or remove anything
//! outside the project directory, provided no other process edits the tree
//! while mono runs.

use std::path::{Path, PathBuf};

use thiserror::Error;

mod confine;
mod key;
mod local;

pub use key::{dir_key, expand_files, output_digest, remove_matching, remove_stale, task_key};
pub use local::LocalBackend;

/// Checks a `cacheDirs` name without touching the filesystem. The runner calls it
/// before the cache is consulted, so an invalid name is refused on every run.
pub fn check_cache_dir(dir: &str) -> Result<(), CacheError> {
    confine::check_dir_name(Path::new(dir))
}

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
    #[error("{path} is not a directory")]
    NotADirectory { path: PathBuf },
    #[error("{path} is neither a regular file, a directory, nor a link, and cannot be cached")]
    UnsupportedFile { path: PathBuf },
    #[error("cache key {key} must be a non-empty name of letters, digits, '-' or '_'")]
    InvalidKey { key: String },
}

/// Storage for task outputs, addressed by cache key. Every relative path is
/// interpreted inside `project_root`.
pub trait CacheBackend {
    /// Whether a complete entry exists for `key`.
    fn contains(&self, key: &str) -> Result<bool, CacheError>;
    /// The file paths stored under `key`, relative to the project root. Empty on a miss.
    fn stored_files(&self, key: &str) -> Result<Vec<PathBuf>, CacheError>;
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

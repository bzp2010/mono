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

pub use key::{expand_files, files_digest, output_digest, remove_matching, remove_stale, task_key};
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
    /// Whether an entry exists for `key`. An entry only exists once it is complete.
    fn contains(&self, key: &str) -> Result<bool, CacheError>;
    /// Restores the entry for `key` into `project_root`. Returns the files restored,
    /// relative to the project root, or `None` on a miss.
    fn get(&self, key: &str, project_root: &Path) -> Result<Option<Vec<PathBuf>>, CacheError>;
    /// Stores the files at `rel_paths` under `key`. The first entry for a key wins.
    fn put(&self, key: &str, project_root: &Path, rel_paths: &[PathBuf]) -> Result<(), CacheError>;
}

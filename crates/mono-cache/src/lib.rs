//! Cache keys and pluggable cache backends.

use std::path::{Path, PathBuf};

use thiserror::Error;

mod key;
mod local;

pub use key::{dir_key, expand_files, output_digest, task_key};
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
}

/// Storage for task outputs, addressed by cache key.
pub trait CacheBackend {
    /// Restores the entry for `key` into `project_root`. Returns `false` on a miss.
    fn get(&self, key: &str, project_root: &Path) -> Result<bool, CacheError>;
    /// Stores the files at `rel_paths` (relative to `project_root`) under `key`.
    fn put(&self, key: &str, project_root: &Path, rel_paths: &[PathBuf]) -> Result<(), CacheError>;
    /// Restores a whole directory tree stored under `key` into `dest_dir`.
    /// Returns `false` on a miss.
    fn get_dir(&self, key: &str, dest_dir: &Path) -> Result<bool, CacheError>;
    /// Stores a whole directory tree under `key`. A missing source is a no-op.
    fn put_dir(&self, key: &str, src_dir: &Path) -> Result<(), CacheError>;
}

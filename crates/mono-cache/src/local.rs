//! Local directory backend: each key is a directory of stored files.

use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::{CacheBackend, CacheError};

/// Stores entries under `dir/<key>/`, preserving relative paths.
pub struct LocalBackend {
    dir: PathBuf,
}

impl LocalBackend {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn entry_dir(&self, key: &str) -> PathBuf {
        self.dir.join(key)
    }

    /// Builds an entry in a private staging directory, then renames it into
    /// place. A reader sees either no entry or a complete one, never a partial
    /// write, and a failed build leaves nothing behind.
    fn publish(
        &self,
        key: &str,
        build: impl FnOnce(&Path) -> Result<(), CacheError>,
    ) -> Result<(), CacheError> {
        fs::create_dir_all(&self.dir)
            .map_err(|source| CacheError::Io { path: self.dir.clone(), source })?;
        let staging = tempfile::TempDir::new_in(&self.dir)
            .map_err(|source| CacheError::Io { path: self.dir.clone(), source })?;
        build(staging.path())?;
        let dst = self.entry_dir(key);
        if dst.exists() {
            fs::remove_dir_all(&dst).map_err(|source| CacheError::Io { path: dst.clone(), source })?;
        }
        let staged = staging.keep();
        fs::rename(&staged, &dst).map_err(|source| {
            let _ = fs::remove_dir_all(&staged);
            CacheError::Io { path: dst.clone(), source }
        })
    }
}

impl CacheBackend for LocalBackend {
    fn get(&self, key: &str, project_root: &Path) -> Result<bool, CacheError> {
        let src = self.entry_dir(key);
        if !src.is_dir() {
            return Ok(false);
        }
        for entry in WalkDir::new(&src) {
            let entry = entry.map_err(|e| CacheError::Io {
                path: src.clone(),
                source: e.into(),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry.path().strip_prefix(&src).expect("walked from src");
            copy(entry.path(), &project_root.join(rel))?;
        }
        Ok(true)
    }

    fn get_dir(&self, key: &str, dest_dir: &Path) -> Result<bool, CacheError> {
        let src = self.entry_dir(key);
        if !src.is_dir() {
            return Ok(false);
        }
        Self::copy_tree(&src, dest_dir)?;
        Ok(true)
    }

    fn put_dir(&self, key: &str, src_dir: &Path) -> Result<(), CacheError> {
        if !src_dir.is_dir() {
            return Ok(());
        }
        self.publish(key, |stage| Self::copy_tree(src_dir, stage))
    }

    fn put(&self, key: &str, project_root: &Path, rel_paths: &[PathBuf]) -> Result<(), CacheError> {
        self.publish(key, |stage| {
            for rel in rel_paths {
                copy(&project_root.join(rel), &stage.join(rel))?;
            }
            Ok(())
        })
    }
}

impl LocalBackend {
    fn copy_tree(from: &Path, to: &Path) -> Result<(), CacheError> {
        for entry in WalkDir::new(from) {
            let entry = entry.map_err(|e| CacheError::Io { path: from.to_path_buf(), source: e.into() })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry.path().strip_prefix(from).expect("walked from root");
            copy(entry.path(), &to.join(rel))?;
        }
        Ok(())
    }
}

/// Copies `from` to `to` through a temporary sibling that is renamed into
/// place. Writing in place fails with ETXTBSY when `to` is a running binary;
/// rename replaces the directory entry and leaves the running inode alone.
fn copy(from: &Path, to: &Path) -> Result<(), CacheError> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| CacheError::Io { path: parent.to_path_buf(), source })?;
    }
    let name = to.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let tmp = to.with_file_name(format!(".{name}.mono-tmp"));
    fs::copy(from, &tmp)
        .map_err(|source| CacheError::Io { path: from.to_path_buf(), source })?;
    fs::rename(&tmp, to).map_err(|source| CacheError::Io { path: to.to_path_buf(), source })
}

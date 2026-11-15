//! Local directory backend: each key is a directory of stored files.
//!
//! Entries are built in a private staging directory and renamed into place, so a
//! reader sees either no entry or a complete one.

use std::fs;
use std::path::{Path, PathBuf};

use crate::confine::{EntryKind, Tree};
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

    /// Builds an entry in a staging directory, then renames it into place. A failed
    /// build removes its staging directory and leaves no entry behind.
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

    /// Restores the entries of a stored tree into `dest` under `prefix`.
    fn restore(&self, store: &Path, dest: &Tree, prefix: &Path) -> Result<(), CacheError> {
        let stored = Tree::new(store);
        for (rel, kind) in stored.walk(Path::new(""))? {
            let target = prefix.join(&rel);
            match kind {
                EntryKind::Dir => dest.make_dir(&target)?,
                EntryKind::File => dest.install_file(&store.join(&rel), &target)?,
                EntryKind::Link(link) => dest.install_link(&link, &target)?,
            }
        }
        Ok(())
    }
}

impl CacheBackend for LocalBackend {
    fn contains(&self, key: &str) -> Result<bool, CacheError> {
        Ok(self.entry_dir(key).is_dir())
    }

    fn get(&self, key: &str, project_root: &Path) -> Result<bool, CacheError> {
        let store = self.entry_dir(key);
        if !store.is_dir() {
            return Ok(false);
        }
        self.restore(&store, &Tree::new(project_root), Path::new(""))?;
        Ok(true)
    }

    fn put(&self, key: &str, project_root: &Path, rel_paths: &[PathBuf]) -> Result<(), CacheError> {
        let project = Tree::new(project_root);
        self.publish(key, |stage| {
            let staged = Tree::new(stage);
            for rel in rel_paths {
                staged.install_file(&project.source(rel)?, rel)?;
            }
            Ok(())
        })
    }

    fn get_dir(&self, key: &str, project_root: &Path, rel_dir: &Path) -> Result<bool, CacheError> {
        let store = self.entry_dir(key);
        if !store.is_dir() {
            return Ok(false);
        }
        let dest = Tree::new(project_root);
        dest.make_dir(rel_dir)?;
        self.restore(&store, &dest, rel_dir)?;
        Ok(true)
    }

    fn put_dir(&self, key: &str, project_root: &Path, rel_dir: &Path) -> Result<(), CacheError> {
        let project = Tree::new(project_root);
        let entries = project.walk(rel_dir)?;
        if entries.is_empty() {
            return Ok(());
        }
        self.publish(key, |stage| {
            let staged = Tree::new(stage);
            for (rel, kind) in entries {
                match kind {
                    EntryKind::Dir => staged.make_dir(&rel)?,
                    EntryKind::File => staged.install_file(&project.source(&rel_dir.join(&rel))?, &rel)?,
                    EntryKind::Link(link) => staged.install_link(&link, &rel)?,
                }
            }
            Ok(())
        })
    }
}

//! Local directory backend: each key is a directory of stored files.
//!
//! An entry is built in a private staging directory, marked complete, and renamed
//! into place. Only a complete entry counts. Output entries are immutable: the
//! first complete entry for a key wins. Cache directory entries are replaced by
//! each new publish, because their key does not cover the content.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::confine::{check_dir_name, EntryKind, Tree};
use crate::{CacheBackend, CacheError};

/// Written last into every staged entry. An entry without it is not an entry.
const COMPLETE_MARKER: &str = ".mono-complete";

static TRASH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// How many times a publish retries when another publisher races it.
const PUBLISH_ATTEMPTS: usize = 8;

/// Stores entries under `dir/<key>/`, preserving relative paths.
pub struct LocalBackend {
    dir: PathBuf,
}

impl LocalBackend {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The directory of one entry. A key is a plain name, so it cannot point
    /// outside the cache directory.
    fn entry_dir(&self, key: &str) -> Result<PathBuf, CacheError> {
        let plain = !key.is_empty()
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !plain {
            return Err(CacheError::InvalidKey { key: key.to_string() });
        }
        Ok(self.dir.join(key))
    }

    /// Whether `dir` is a real directory (not a link) that carries the marker.
    fn is_complete(dir: &Path) -> bool {
        matches!(fs::symlink_metadata(dir), Ok(meta) if meta.is_dir())
            && dir.join(COMPLETE_MARKER).is_file()
    }

    /// Builds an entry in staging, marks it complete, and moves it into place.
    ///
    /// With `replace` false, an existing complete entry is kept and the staged
    /// build is discarded. With `replace` true, the existing entry is moved aside
    /// first, so readers see either the old complete entry or the new one.
    fn publish(
        &self,
        key: &str,
        replace: bool,
        build: impl FnOnce(&Path) -> Result<(), CacheError>,
    ) -> Result<(), CacheError> {
        let dst = self.entry_dir(key)?;
        fs::create_dir_all(&self.dir)
            .map_err(|source| CacheError::Io { path: self.dir.clone(), source })?;
        let staging = tempfile::TempDir::new_in(&self.dir)
            .map_err(|source| CacheError::Io { path: self.dir.clone(), source })?;
        build(staging.path())?;
        let marker = staging.path().join(COMPLETE_MARKER);
        fs::write(&marker, b"complete").map_err(|source| CacheError::Io { path: marker, source })?;
        if !replace && Self::is_complete(&dst) {
            return Ok(());
        }
        let staged = staging.keep();
        // Another publisher can move an entry away between our steps, so the move
        // and the rename are retried a bounded number of times.
        let mut failure = None;
        for _ in 0..PUBLISH_ATTEMPTS {
            if !replace && Self::is_complete(&dst) {
                let _ = fs::remove_dir_all(&staged);
                return Ok(());
            }
            let trash = self.dir.join(format!(
                ".trash-{}-{}",
                std::process::id(),
                TRASH_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let moved_aside = match fs::rename(&dst, &trash) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(source) => {
                    let _ = fs::remove_dir_all(&staged);
                    return Err(CacheError::Io { path: dst, source });
                }
            };
            let renamed = fs::rename(&staged, &dst);
            if moved_aside {
                let _ = fs::remove_dir_all(&trash);
            }
            match renamed {
                Ok(()) => return Ok(()),
                Err(source) => failure = Some(source),
            }
        }
        let _ = fs::remove_dir_all(&staged);
        match failure {
            Some(source) => Err(CacheError::Io { path: dst, source }),
            None => Ok(()),
        }
    }

    /// Restores the entries of a stored tree into `dest` under `prefix`. The marker
    /// is bookkeeping, not part of the output, so it is never restored.
    fn restore(&self, store: &Path, dest: &Tree, prefix: &Path) -> Result<(), CacheError> {
        // Tree::walk reports a missing directory as empty, which is right for a put
        // but would turn a replaced entry into a hit that restored nothing.
        if !store.is_dir() {
            return Err(CacheError::Io {
                path: store.to_path_buf(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            });
        }
        let stored = Tree::new(store);
        for (rel, kind) in stored.walk(Path::new(""))? {
            if rel == Path::new(COMPLETE_MARKER) {
                continue;
            }
            let target = prefix.join(&rel);
            match kind {
                EntryKind::Dir => dest.make_dir(&target)?,
                EntryKind::File => dest.install_file(&store.join(&rel), &target)?,
                EntryKind::Link(link) => dest.install_link(&link, &target)?,
            }
        }
        Ok(())
    }

    /// Restores an entry. A file that vanished while the restore walked the entry
    /// means the entry was replaced or damaged underneath it, so the result is a
    /// miss. The caller then runs the command and publishes a fresh entry.
    fn restore_or_miss(&self, store: &Path, dest: &Tree, prefix: &Path) -> Result<bool, CacheError> {
        match self.restore(store, dest, prefix) {
            Ok(()) => Ok(true),
            Err(CacheError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}

impl CacheBackend for LocalBackend {
    fn contains(&self, key: &str) -> Result<bool, CacheError> {
        Ok(Self::is_complete(&self.entry_dir(key)?))
    }

    fn stored_files(&self, key: &str) -> Result<Vec<PathBuf>, CacheError> {
        let store = self.entry_dir(key)?;
        if !Self::is_complete(&store) {
            return Ok(Vec::new());
        }
        Ok(Tree::new(&store)
            .walk(Path::new(""))?
            .into_iter()
            .filter(|(rel, kind)| *kind == EntryKind::File && rel != Path::new(COMPLETE_MARKER))
            .map(|(rel, _)| rel)
            .collect())
    }

    fn get(&self, key: &str, project_root: &Path) -> Result<bool, CacheError> {
        let store = self.entry_dir(key)?;
        if !Self::is_complete(&store) {
            return Ok(false);
        }
        self.restore_or_miss(&store, &Tree::new(project_root), Path::new(""))
    }

    fn put(&self, key: &str, project_root: &Path, rel_paths: &[PathBuf]) -> Result<(), CacheError> {
        let project = Tree::new(project_root);
        self.publish(key, false, |stage| {
            let staged = Tree::new(stage);
            for rel in rel_paths {
                staged.install_file(&project.source(rel)?, rel)?;
            }
            Ok(())
        })
    }

    fn get_dir(&self, key: &str, project_root: &Path, rel_dir: &Path) -> Result<bool, CacheError> {
        check_dir_name(rel_dir)?;
        let store = self.entry_dir(key)?;
        if !Self::is_complete(&store) {
            return Ok(false);
        }
        let dest = Tree::new(project_root);
        dest.make_dir(rel_dir)?;
        self.restore_or_miss(&store, &dest, rel_dir)
    }

    fn put_dir(&self, key: &str, project_root: &Path, rel_dir: &Path) -> Result<(), CacheError> {
        check_dir_name(rel_dir)?;
        let project = Tree::new(project_root);
        // A missing directory has nothing to store, so it is a no-op.
        let Some(entries) = project.walk(rel_dir).map(Some).or_else(|e| match e {
            CacheError::Io { ref source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            other => Err(other),
        })? else {
            return Ok(());
        };
        self.publish(key, true, |stage| {
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

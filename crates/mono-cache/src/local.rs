//! Local directory backend.
//!
//! An entry is the directory `dir/<key>/`, holding the output files at their
//! project-relative paths. It is built in a staging directory and renamed into
//! place in one step, so an entry that exists is always complete. Entries are
//! never modified. The first entry for a key wins, and a later publish of the same
//! key discards its staged build.

use std::fs;
use std::path::{Path, PathBuf};

use crate::confine::{EntryKind, Tree};
use crate::{CacheBackend, CacheError};

/// Stores entries under `dir/<key>/`.
pub struct LocalBackend {
    dir: PathBuf,
}

impl LocalBackend {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The directory of one entry. A key is a plain name, so it cannot point outside
    /// the cache directory.
    fn entry_dir(&self, key: &str) -> Result<PathBuf, CacheError> {
        let plain = !key.is_empty()
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !plain {
            return Err(CacheError::InvalidKey { key: key.to_string() });
        }
        Ok(self.dir.join(key))
    }

    /// Whether an entry exists. An entry is a real directory, never a link.
    fn exists(entry: &Path) -> bool {
        matches!(fs::symlink_metadata(entry), Ok(meta) if meta.is_dir())
    }

    /// Builds an entry in staging and renames it into place. A failed build leaves no
    /// entry. A rename that loses to a concurrent publish of the same key is a success,
    /// because the same key names the same content.
    fn publish(
        &self,
        key: &str,
        build: impl FnOnce(&Path) -> Result<(), CacheError>,
    ) -> Result<(), CacheError> {
        let dst = self.entry_dir(key)?;
        if Self::exists(&dst) {
            return Ok(());
        }
        fs::create_dir_all(&self.dir)
            .map_err(|source| CacheError::Io { path: self.dir.clone(), source })?;
        let staging = tempfile::TempDir::new_in(&self.dir)
            .map_err(|source| CacheError::Io { path: self.dir.clone(), source })?;
        build(staging.path())?;
        let staged = staging.keep();
        match fs::rename(&staged, &dst) {
            Ok(()) => Ok(()),
            Err(_) if Self::exists(&dst) => {
                let _ = fs::remove_dir_all(&staged);
                Ok(())
            }
            Err(source) => {
                let _ = fs::remove_dir_all(&staged);
                Err(CacheError::Io { path: dst, source })
            }
        }
    }
}

impl CacheBackend for LocalBackend {
    fn contains(&self, key: &str) -> Result<bool, CacheError> {
        Ok(Self::exists(&self.entry_dir(key)?))
    }

    fn get(&self, key: &str, project_root: &Path) -> Result<Option<Vec<PathBuf>>, CacheError> {
        let entry = self.entry_dir(key)?;
        if !Self::exists(&entry) {
            return Ok(None);
        }
        let dest = Tree::new(project_root);
        let mut restored = Vec::new();
        let result = (|| -> Result<(), CacheError> {
            for (rel, kind) in Tree::new(&entry).walk(Path::new(""))? {
                match kind {
                    // Parent directories are created by the file installs below.
                    EntryKind::Dir => {}
                    EntryKind::File => {
                        dest.install_file(&entry.join(&rel), &rel)?;
                        restored.push(rel);
                    }
                    EntryKind::Link(_) => {
                        return Err(CacheError::UnsupportedFile { path: entry.join(&rel) });
                    }
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(Some(restored)),
            // The entry was removed while it was read. That is a miss, and the caller
            // runs the command. Any other missing path is a real error.
            Err(CacheError::Io { path, source })
                if source.kind() == std::io::ErrorKind::NotFound && path.starts_with(&entry) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
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
}

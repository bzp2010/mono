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
    /// Copies a tree, keeping empty directories and symlinks as they are.
    /// Symlinks are never followed, so a link into the tree is stored as a link.
    fn copy_tree(from: &Path, to: &Path) -> Result<(), CacheError> {
        for entry in WalkDir::new(from).follow_links(false) {
            let entry = entry.map_err(|e| CacheError::Io {
                path: from.to_path_buf(),
                source: e.into(),
            })?;
            let rel = entry.path().strip_prefix(from).expect("walked from root");
            let dst = to.join(rel);
            let kind = entry.file_type();
            if kind.is_dir() {
                fs::create_dir_all(&dst)
                    .map_err(|source| CacheError::Io { path: dst.clone(), source })?;
            } else if kind.is_symlink() {
                let target = fs::read_link(entry.path())
                    .map_err(|source| CacheError::Io { path: entry.path().to_path_buf(), source })?;
                symlink_into_place(&target, &dst)?;
            } else if kind.is_file() {
                copy(entry.path(), &dst)?;
            }
        }
        Ok(())
    }
}

/// Copies `from` to `to` through a uniquely named temporary file in the same
/// directory, renamed into place. Each copy owns its temporary file, so
/// concurrent restores of one destination cannot interfere. Writing in place
/// would also fail with ETXTBSY when `to` is a running binary.
fn copy(from: &Path, to: &Path) -> Result<(), CacheError> {
    let parent = to.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|source| CacheError::Io { path: parent.to_path_buf(), source })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|source| CacheError::Io { path: parent.to_path_buf(), source })?;
    let mut src = fs::File::open(from)
        .map_err(|source| CacheError::Io { path: from.to_path_buf(), source })?;
    std::io::copy(&mut src, tmp.as_file_mut())
        .map_err(|source| CacheError::Io { path: from.to_path_buf(), source })?;
    let mode = src
        .metadata()
        .map_err(|source| CacheError::Io { path: from.to_path_buf(), source })?
        .permissions();
    fs::set_permissions(tmp.path(), mode)
        .map_err(|source| CacheError::Io { path: tmp.path().to_path_buf(), source })?;
    clear_conflicting_directory(to)?;
    tmp.persist(to)
        .map_err(|e| CacheError::Io { path: to.to_path_buf(), source: e.error })?;
    Ok(())
}

/// Removes a real directory at `dst` so that a file or link can take its place.
/// The check uses `symlink_metadata`, so a link to a directory is never followed.
fn clear_conflicting_directory(dst: &Path) -> Result<(), CacheError> {
    match fs::symlink_metadata(dst) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(dst)
            .map_err(|source| CacheError::Io { path: dst.to_path_buf(), source }),
        _ => Ok(()),
    }
}

/// Creates a symbolic link at `dst`, replacing whatever was there, through a
/// temporary link that is renamed into place.
fn symlink_into_place(target: &Path, dst: &Path) -> Result<(), CacheError> {
    let parent = dst.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|source| CacheError::Io { path: parent.to_path_buf(), source })?;
    for attempt in 0..1000u32 {
        let tmp = parent.join(format!(".mono-link-{}-{attempt}", std::process::id()));
        match std::os::unix::fs::symlink(target, &tmp) {
            Ok(()) => {
                clear_conflicting_directory(dst)?;
                return fs::rename(&tmp, dst).map_err(|source| {
                    let _ = fs::remove_file(&tmp);
                    CacheError::Io { path: dst.to_path_buf(), source }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(CacheError::Io { path: tmp, source }),
        }
    }
    Err(CacheError::Io {
        path: dst.to_path_buf(),
        source: std::io::Error::other("could not find a free temporary link name"),
    })
}

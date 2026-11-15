//! Confined filesystem access inside one project tree.
//!
//! Every path that mono reads, writes, or removes inside a project goes through
//! [`Tree`]. A tree trusts its root directory and nothing below it:
//!
//! - Relative paths and patterns must be relative and must not contain `..`.
//! - Reads resolve symbolic links, and the resolved file must stay inside the root.
//! - Removals and writes never traverse a symbolic link. Removal fails when an
//!   ancestor is a link. Installation replaces a link or a file in an ancestor
//!   position with a real directory, so nothing is written through a link.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

use crate::CacheError;

/// The kind of one entry found by [`Tree::walk`]. Links are reported, not followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Dir,
    File,
    Link(PathBuf),
}

/// A directory tree that all relative paths are confined to.
pub(crate) struct Tree {
    root: PathBuf,
}

impl Tree {
    pub(crate) fn new(root: &Path) -> Self {
        Self { root: root.to_path_buf() }
    }

    /// Expands glob patterns into sorted, unique file paths relative to the root.
    ///
    /// A trailing `/**` means every file beneath the directory. Every match is
    /// checked: its relative path must not escape, and its resolved file must
    /// lie inside the root, so character classes such as `.[.]` cannot reach
    /// a sibling directory.
    pub(crate) fn expand(&self, patterns: &[String]) -> Result<Vec<PathBuf>, CacheError> {
        let mut out = Vec::new();
        for pattern in patterns {
            check_relative_path(Path::new(pattern))?;
            let normalized = match pattern.strip_suffix("/**") {
                Some(prefix) => format!("{prefix}/**/*"),
                None => pattern.clone(),
            };
            // The glob crate matches only directories for a trailing `**`, so
            // `dist/**` is rewritten above. The root is escaped so that its own
            // metacharacters match literally.
            let escaped_root = glob::Pattern::escape(&self.root.to_string_lossy());
            let full = PathBuf::from(escaped_root).join(&normalized);
            let entries = glob::glob(&full.to_string_lossy()).map_err(|e| CacheError::Glob {
                pattern: pattern.clone(),
                message: e.to_string(),
            })?;
            for entry in entries {
                let path = entry.map_err(|e| CacheError::Io {
                    path: e.path().to_path_buf(),
                    source: e.into(),
                })?;
                if path.is_dir() {
                    continue;
                }
                if !path.is_file() {
                    // A named pipe, socket, or dangling link cannot be cached.
                    return Err(CacheError::UnsupportedFile { path });
                }
                let rel = path
                    .strip_prefix(&self.root)
                    .map_err(|_| CacheError::OutsideProject { pattern: pattern.clone() })?
                    .to_path_buf();
                check_relative_path(&rel)?;
                self.check_resolves_inside(&rel)?;
                out.push(rel);
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Reads a file inside the tree. Symbolic links are followed only when the
    /// resolved file is still inside the root.
    pub(crate) fn read(&self, rel: &Path) -> Result<Vec<u8>, CacheError> {
        check_relative_path(rel)?;
        self.check_resolves_inside(rel)?;
        let path = self.root.join(rel);
        fs::read(&path).map_err(|source| CacheError::Io { path, source })
    }

    /// The path of a file inside the tree, to copy from.
    pub(crate) fn source(&self, rel: &Path) -> Result<PathBuf, CacheError> {
        check_relative_path(rel)?;
        self.check_ancestors(rel)?;
        self.check_resolves_inside(rel)?;
        Ok(self.root.join(rel))
    }

    /// Removes one file. A missing file is not an error.
    pub(crate) fn remove_file(&self, rel: &Path) -> Result<(), CacheError> {
        check_relative_path(rel)?;
        self.check_ancestors(rel)?;
        let path = self.root.join(rel);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(CacheError::Io { path, source }),
        }
    }

    /// Installs a regular file copied from `src` at `rel`, replacing whatever was there.
    pub(crate) fn install_file(&self, src: &Path, rel: &Path) -> Result<(), CacheError> {
        check_relative_path(rel)?;
        self.prepare_parents(rel)?;
        copy_atomic(src, &self.root.join(rel))
    }

    /// Every entry beneath `under`, relative to it, with the directory itself as
    /// the first entry (empty path). Links are reported, not followed. A missing
    /// `under` is an error, so a caller never mistakes it for an empty tree.
    pub(crate) fn walk(&self, under: &Path) -> Result<Vec<(PathBuf, EntryKind)>, CacheError> {
        check_relative_path(under)?;
        self.check_ancestors(under)?;
        let base = self.root.join(under);
        let meta = fs::symlink_metadata(&base)
            .map_err(|source| CacheError::Io { path: base.clone(), source })?;
        if meta.file_type().is_symlink() {
            return Err(CacheError::ThroughLink { path: base });
        }
        if !meta.is_dir() {
            return Err(CacheError::NotADirectory { path: base });
        }
        let mut out = vec![(PathBuf::new(), EntryKind::Dir)];
        // The root is not skipped with min_depth: with min_depth, a root that vanishes
        // while it is read yields no error, so the walk would look empty and succeed.
        for entry in WalkDir::new(&base).follow_links(false) {
            let entry = entry.map_err(|e| CacheError::Io {
                path: base.clone(),
                source: e.into(),
            })?;
            if entry.depth() == 0 {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&base)
                .expect("walked from base")
                .to_path_buf();
            let kind = entry.file_type();
            let kind = if kind.is_dir() {
                EntryKind::Dir
            } else if kind.is_symlink() {
                let target = fs::read_link(entry.path()).map_err(|source| CacheError::Io {
                    path: entry.path().to_path_buf(),
                    source,
                })?;
                EntryKind::Link(target)
            } else if kind.is_file() {
                EntryKind::File
            } else {
                return Err(CacheError::UnsupportedFile { path: entry.path().to_path_buf() });
            };
            out.push((rel, kind));
        }
        // WalkDir can end quietly when the root vanishes while it is read. Checking the
        // root again catches that: if it still exists here, the listing is complete.
        fs::symlink_metadata(&base).map_err(|source| CacheError::Io { path: base.clone(), source })?;
        Ok(out)
    }

    /// Makes every ancestor of `rel` below the root a real directory. A link or a
    /// file in such a position is replaced. The root itself is trusted.
    fn prepare_parents(&self, rel: &Path) -> Result<(), CacheError> {
        let Some(parent) = rel.parent() else {
            return Ok(());
        };
        let mut current = self.root.clone();
        for component in parent.components() {
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(meta) if meta.is_dir() => {}
                // A link or file in an ancestor position is replaced by a directory.
                // A failed removal surfaces from create_dir, which then fails.
                Ok(_) => {
                    remove_non_directory(&current)?;
                    create_dir(&current)?;
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => create_dir(&current)?,
                Err(source) => return Err(CacheError::Io { path: current.clone(), source }),
            }
        }
        Ok(())
    }

    /// Fails when an ancestor of `rel` below the root is a symbolic link.
    fn check_ancestors(&self, rel: &Path) -> Result<(), CacheError> {
        let Some(parent) = rel.parent() else {
            return Ok(());
        };
        let mut current = self.root.clone();
        for component in parent.components() {
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(CacheError::ThroughLink { path: current });
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(source) => return Err(CacheError::Io { path: current, source }),
            }
        }
        Ok(())
    }

    /// Fails when the file at `rel` resolves to a place outside the root.
    fn check_resolves_inside(&self, rel: &Path) -> Result<(), CacheError> {
        let path = self.root.join(rel);
        let resolved = fs::canonicalize(&path).map_err(|source| CacheError::Io {
            path: path.clone(),
            source,
        })?;
        let root = fs::canonicalize(&self.root).map_err(|source| CacheError::Io {
            path: self.root.clone(),
            source,
        })?;
        if resolved.starts_with(&root) {
            Ok(())
        } else {
            Err(CacheError::OutsideProject { pattern: rel.display().to_string() })
        }
    }
}

/// Rejects absolute paths, prefixes, and any `..` component.
fn check_relative_path(path: &Path) -> Result<(), CacheError> {
    let escapes = path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)));
    if escapes {
        return Err(CacheError::OutsideProject { pattern: path.display().to_string() });
    }
    Ok(())
}

/// Creates one directory. Two restores can race to create the same directory,
/// so a directory that already exists is not an error.
fn create_dir(path: &Path) -> Result<(), CacheError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        // symlink_metadata, not is_dir: a link to a directory must not pass as one.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => Ok(()),
            _ => Err(CacheError::Io { path: path.to_path_buf(), source: e }),
        },
        Err(source) => Err(CacheError::Io { path: path.to_path_buf(), source }),
    }
}

/// Removes a link or a file that sits where a directory is needed. A removal that
/// fails is tolerated only when another restore has already replaced the path
/// with a directory; any other failure is reported.
fn remove_non_directory(path: &Path) -> Result<(), CacheError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => Ok(()),
            _ => Err(CacheError::Io { path: path.to_path_buf(), source }),
        },
    }
}

/// Removes a real directory at `dst` so that a file or link can take its place.
/// `symlink_metadata` is used, so a link to a directory is never followed.
fn clear_real_directory(dst: &Path) -> Result<(), CacheError> {
    match fs::symlink_metadata(dst) {
        Ok(meta) if meta.is_dir() => match fs::remove_dir_all(dst) {
            Ok(()) => Ok(()),
            Err(source) => match fs::symlink_metadata(dst) {
                // Another restore removed or replaced it first. That is success.
                Ok(now) if now.is_dir() => Err(CacheError::Io { path: dst.to_path_buf(), source }),
                _ => Ok(()),
            },
        },
        _ => Ok(()),
    }
}

/// Copies `src` to `dst` through a uniquely named temporary file in the same
/// directory, keeping the source permissions. Renaming avoids ETXTBSY when `dst`
/// is a running binary.
fn copy_atomic(src: &Path, dst: &Path) -> Result<(), CacheError> {
    let parent = dst.parent().unwrap_or(Path::new("."));
    // The source is opened and copied before any existing directory at `dst` is
    // touched, so a source that vanished does not leave `dst` half destroyed.
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|source| CacheError::Io { path: parent.to_path_buf(), source })?;
    let mut from = fs::File::open(src).map_err(|source| CacheError::Io { path: src.to_path_buf(), source })?;
    io::copy(&mut from, tmp.as_file_mut()).map_err(|source| CacheError::Io { path: src.to_path_buf(), source })?;
    let mode = from
        .metadata()
        .map_err(|source| CacheError::Io { path: src.to_path_buf(), source })?
        .permissions();
    fs::set_permissions(tmp.path(), mode)
        .map_err(|source| CacheError::Io { path: tmp.path().to_path_buf(), source })?;
    clear_real_directory(dst)?;
    tmp.persist(dst)
        .map_err(|e| CacheError::Io { path: dst.to_path_buf(), source: e.error })?;
    Ok(())
}

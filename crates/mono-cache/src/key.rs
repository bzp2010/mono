//! Cache keys and digests over declared inputs and outputs.

use std::fs;
use std::path::{Path, PathBuf};

use mono_core::TargetConfig;
use mono_graph::TaskId;
use sha2::{Digest, Sha256};

use crate::CacheError;

/// Expands glob patterns relative to `root` into sorted, unique file paths
/// that are relative to `root`. Directories are skipped. A pattern that
/// matches nothing contributes nothing.
pub fn expand_files(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, CacheError> {
    let mut out = Vec::new();
    for pattern in patterns {
        // The glob crate matches only directories for a trailing `**`, so
        // `dist/**` is rewritten to `dist/**/*` to mean every file beneath it.
        let normalized = match pattern.strip_suffix("/**") {
            Some(prefix) => format!("{prefix}/**/*"),
            None => pattern.clone(),
        };
        // The root is escaped so that metacharacters in its path match literally.
        let escaped_root = glob::Pattern::escape(&root.to_string_lossy());
        let full = std::path::PathBuf::from(escaped_root).join(normalized);
        let entries = glob::glob(&full.to_string_lossy()).map_err(|e| CacheError::Glob {
            pattern: pattern.clone(),
            message: e.to_string(),
        })?;
        for entry in entries {
            let path = entry.map_err(|e| CacheError::Io {
                path: e.path().to_path_buf(),
                source: e.into(),
            })?;
            if path.is_file() {
                let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(rel);
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Computes the cache key of a task.
///
/// The key covers the task identity, the command, the content of every input
/// file, and the contributions of the task's dependencies. A dependency's
/// contribution is its output digest when it declares outputs, otherwise its key.
pub fn task_key(
    task: &TaskId,
    target: &TargetConfig,
    project_root: &Path,
    dep_contributions: &[String],
) -> Result<String, CacheError> {
    let mut h = Sha256::new();
    feed(&mut h, "task", task.to_string().as_bytes());
    feed(&mut h, "command", target.command.as_bytes());
    for rel in expand_files(project_root, &target.inputs)? {
        let content = read(&project_root.join(&rel))?;
        feed(&mut h, "input-path", rel.to_string_lossy().as_bytes());
        feed(&mut h, "input-content", &Sha256::digest(&content));
    }
    for dep in dep_contributions {
        feed(&mut h, "dependency", dep.as_bytes());
    }
    Ok(hex(&h.finalize()))
}

/// Key for a `cacheDirs` entry: the target key combined with the directory
/// path, so one target can cache several directories without collisions.
pub fn dir_key(target_key: &str, dir: &str) -> String {
    let mut h = Sha256::new();
    feed(&mut h, "target-key", target_key.as_bytes());
    feed(&mut h, "cache-dir", dir.as_bytes());
    hex(&h.finalize())
}

/// Digest over the paths and contents of output files, in sorted path order.
pub fn output_digest(project_root: &Path, rel_paths: &[PathBuf]) -> Result<String, CacheError> {
    let mut h = Sha256::new();
    for rel in rel_paths {
        let content = read(&project_root.join(rel))?;
        feed(&mut h, "output-path", rel.to_string_lossy().as_bytes());
        feed(&mut h, "output-content", &Sha256::digest(&content));
    }
    Ok(hex(&h.finalize()))
}

/// Length-prefixed field, so distinct field sequences cannot collide.
fn feed(h: &mut Sha256, label: &str, bytes: &[u8]) {
    h.update((label.len() as u64).to_le_bytes());
    h.update(label.as_bytes());
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}

fn read(path: &Path) -> Result<Vec<u8>, CacheError> {
    fs::read(path).map_err(|source| CacheError::Io { path: path.to_path_buf(), source })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

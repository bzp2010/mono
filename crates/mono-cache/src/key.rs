//! Cache keys and digests over declared inputs and outputs.

use std::fs;
use std::path::{Path, PathBuf};

use mono_core::TargetConfig;
use mono_graph::TaskId;
use sha2::{Digest, Sha256};

use crate::confine::Tree;
use crate::CacheError;

/// Expands glob patterns relative to `root` into sorted file paths. See
/// [`Tree::expand`] for the rules.
pub fn expand_files(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, CacheError> {
    Tree::new(root).expand(patterns)
}

/// Removes every file that the patterns match under `root`, as the outputs of
/// an earlier version. Missing files are skipped.
pub fn remove_matching(root: &Path, patterns: &[String]) -> Result<(), CacheError> {
    let tree = Tree::new(root);
    for rel in tree.expand(patterns)? {
        tree.remove_file(&rel)?;
    }
    Ok(())
}

/// Removes the files that the patterns match under `root` but that are not in
/// `keep`. Used after a snapshot is restored, so that only outputs left by
/// another version disappear.
pub fn remove_stale(root: &Path, patterns: &[String], keep: &[PathBuf]) -> Result<(), CacheError> {
    let tree = Tree::new(root);
    let keep: std::collections::HashSet<&PathBuf> = keep.iter().collect();
    for rel in tree.expand(patterns)? {
        if !keep.contains(&rel) {
            tree.remove_file(&rel)?;
        }
    }
    Ok(())
}

/// Computes the cache key of a task.
///
/// The key covers the task identity, the command, the declared output patterns,
/// the content of every input file, and the dependency contributions. A
/// dependency's contribution is its output digest when it declares outputs,
/// otherwise its key.
pub fn task_key(
    task: &TaskId,
    target: &TargetConfig,
    project_root: &Path,
    dep_contributions: &[String],
) -> Result<String, CacheError> {
    let tree = Tree::new(project_root);
    let mut h = Sha256::new();
    feed(&mut h, "task", task.to_string().as_bytes());
    feed(&mut h, "command", target.command.as_bytes());
    // Output declarations are part of the key: a snapshot taken under other
    // declarations must not be restored.
    for pattern in &target.outputs {
        feed(&mut h, "output-declaration", pattern.as_bytes());
    }
    for rel in tree.expand(&target.inputs)? {
        let content = tree.read(&rel)?;
        feed(&mut h, "input-path", rel.to_string_lossy().as_bytes());
        feed(&mut h, "input-content", &Sha256::digest(&content));
    }
    for dep in dep_contributions {
        feed(&mut h, "dependency", dep.as_bytes());
    }
    Ok(hex(&h.finalize()))
}

/// Digest over the names and contents of the given files. A file that does not
/// exist contributes its name and a marker, so creating it changes the digest.
pub fn files_digest(paths: &[PathBuf]) -> Result<String, CacheError> {
    let mut h = Sha256::new();
    for path in paths {
        feed(&mut h, "file-name", path.to_string_lossy().as_bytes());
        match fs::read(path) {
            Ok(content) => feed(&mut h, "file-content", &Sha256::digest(&content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => feed(&mut h, "file-absent", b""),
            Err(source) => return Err(CacheError::Io { path: path.clone(), source }),
        }
    }
    Ok(hex(&h.finalize()))
}

/// Digest over the paths and contents of output files, in sorted path order.
pub fn output_digest(project_root: &Path, rel_paths: &[PathBuf]) -> Result<String, CacheError> {
    let tree = Tree::new(project_root);
    let mut h = Sha256::new();
    for rel in rel_paths {
        let content = tree.read(rel)?;
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

//! Git integration: which files changed, relative to a revision.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

/// Errors raised while asking git.
#[derive(Debug, Error)]
pub enum VcsError {
    #[error("failed to run git: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("git {command} failed: {stderr}")]
    Git { command: String, stderr: String },
}

/// Files changed in the repository containing `dir`, relative to `base`.
///
/// `base` is resolved to a commit and must name one. The comparison is against the
/// merge base of `base` and `HEAD`, so changes that only `base` received since the
/// branch was cut are not reported. The set is the union of tracked files that
/// differ from that point in the working tree, which includes staged, unstaged, and
/// committed changes, and untracked files that are not ignored. Renames are reported
/// as a deletion and an addition, so both paths count. Paths are absolute, under the
/// repository's top-level directory.
pub fn changed_files(dir: &Path, base: &str) -> Result<Vec<PathBuf>, VcsError> {
    let top = PathBuf::from(git_text(dir, &["rev-parse", "--show-toplevel"])?.trim_end_matches('\n'));
    // `--end-of-options` and `^{commit}` keep `base` from being read as an option, a
    // path, or a tree-ish that is not a commit.
    let spec = format!("{base}^{{commit}}");
    let base_commit = git_text(&top, &["rev-parse", "--verify", "--end-of-options", &spec])?
        .trim()
        .to_string();
    let point = git_text(&top, &["merge-base", &base_commit, "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or(base_commit);

    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    for rel in git_paths(&top, &["diff", "--name-only", "-z", "--no-renames", &point, "--"])? {
        files.insert(top.join(rel));
    }
    for rel in git_paths(&top, &["ls-files", "--others", "--exclude-standard", "-z"])? {
        files.insert(top.join(rel));
    }
    Ok(files.into_iter().collect())
}

/// Runs git and returns its stdout as text.
fn git_text(dir: &Path, args: &[&str]) -> Result<String, VcsError> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(VcsError::Spawn)?;
    if !output.status.success() {
        return Err(VcsError::Git {
            command: args.join(" "),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs git with `-z` output and returns the paths it lists.
fn git_paths(top: &Path, args: &[&str]) -> Result<Vec<PathBuf>, VcsError> {
    let text = git_text(top, args)?;
    Ok(text
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect())
}

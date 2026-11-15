//! Runs target commands in project directories, with optional output caching.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use mono_cache::{expand_files, output_digest, remove_stale, task_key, CacheBackend, CacheError};
use mono_graph::{task_dependencies, GraphError, ProjectGraph, TaskId};
use thiserror::Error;

/// Result of one task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutcome {
    /// The command ran and exited 0.
    Success,
    /// The outputs were restored from the cache; the command did not run.
    Cached,
    /// The command ran and exited with a non-zero code.
    Failed { code: i32 },
}

/// Errors raised while running tasks. A non-zero exit is an outcome, not an error.
#[derive(Debug, Error)]
pub enum RunError {
    #[error("failed to start command for {task}: {source}")]
    Spawn {
        task: String,
        #[source]
        source: std::io::Error,
    },
    #[error("command for {task} was terminated by a signal")]
    Terminated { task: String },
    #[error("task {task} refers to a project or target that is not in the graph")]
    Unknown { task: String },
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Cache(#[from] CacheError),
}

/// Runs `command` through `sh -c` with `root` as the working directory.
pub fn run_task(task: &TaskId, root: &Path, command: &str) -> Result<TaskOutcome, RunError> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(root)
        .status()
        .map_err(|source| RunError::Spawn { task: task.to_string(), source })?;
    if status.success() {
        return Ok(TaskOutcome::Success);
    }
    match status.code() {
        Some(code) => Ok(TaskOutcome::Failed { code }),
        None => Err(RunError::Terminated { task: task.to_string() }),
    }
}

/// Runs the planned tasks in order. Stops after the first failure, which is
/// included in the returned list.
///
/// A task with declared `outputs` is cached when `cache` is provided: on a hit
/// its outputs are restored and the command is skipped. Tasks without outputs
/// always run.
pub fn run_plan(
    graph: &ProjectGraph,
    plan: &[TaskId],
    cache: Option<&dyn CacheBackend>,
) -> Result<Vec<(TaskId, TaskOutcome)>, RunError> {
    // Each finished task's contribution to its dependents' keys.
    let mut contributions: BTreeMap<TaskId, String> = BTreeMap::new();
    let mut results = Vec::new();
    for task in plan {
        let unknown = || RunError::Unknown { task: task.to_string() };
        let project = graph.project(&task.project).ok_or_else(unknown)?;
        let config = project.targets.get(&task.target).ok_or_else(unknown)?;
        let root = project.root.as_path();

        let dep_contributions: Vec<String> = task_dependencies(graph, task)?
            .iter()
            .map(|dep| contributions.get(dep).cloned().unwrap_or_default())
            .collect();
        let key = task_key(task, config, root, &dep_contributions)?;
        let cacheable = cache.is_some() && !config.outputs.is_empty();

        // A hit restores the outputs, then removes files that were outputs of an
        // earlier version but are not part of this entry. A miss leaves existing
        // outputs alone, because the command may be incremental and rely on them.
        let outcome = match cache {
            Some(backend) if cacheable => match backend.get(&key, root)? {
                Some(restored) => {
                    remove_stale(root, &config.outputs, &restored)?;
                    TaskOutcome::Cached
                }
                None => run_task(task, root, &config.command)?,
            },
            _ => run_task(task, root, &config.command)?,
        };

        let rel = expand_files(root, &config.outputs)?;
        let publish = cacheable && outcome == TaskOutcome::Success && !rel.is_empty();
        if let Some(backend) = cache.filter(|_| publish) {
            backend.put(&key, root, &rel)?;
        }
        let contribution = if config.outputs.is_empty() {
            key
        } else {
            output_digest(root, &rel)?
        };
        contributions.insert(task.clone(), contribution);

        let failed = matches!(outcome, TaskOutcome::Failed { .. });
        results.push((task.clone(), outcome));
        if failed {
            break;
        }
    }
    Ok(results)
}

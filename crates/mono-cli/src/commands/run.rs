//! `mono run <target>`: execute a target across the graph.

use std::env;

use mono_cache::LocalBackend;
use mono_graph::{affected, plan};
use mono_task::{run_plan, TaskOutcome};

use crate::context::{find_workspace_root, load_workspace, CliError};

/// Directory under the workspace root where task outputs are cached.
const CACHE_DIR: &str = ".mono/cache";

/// Runs `target` and returns the process exit code: 0 on success, 1 if a task failed.
///
/// With `base`, only the projects affected by changes since that revision run.
pub fn run(target: &str, base: Option<&str>) -> Result<i32, CliError> {
    let cwd = env::current_dir()?;
    let root = find_workspace_root(&cwd)?;
    let workspace = load_workspace(&root)?;
    let mut tasks = plan(&workspace.graph, target)?;
    if let Some(base) = base {
        let changed = mono_vcs_git::changed_files(&root, base)?;
        let affected = affected(&workspace.graph, &changed);
        tasks.retain(|task| affected.contains(&task.project));
    }
    let cache = LocalBackend::new(root.join(CACHE_DIR));
    let results = run_plan(&workspace.graph, &tasks, Some(&cache))?;

    let mut code = 0;
    for (task, outcome) in &results {
        match outcome {
            TaskOutcome::Success => println!("{task}: ok"),
            TaskOutcome::Cached => println!("{task}: cached"),
            TaskOutcome::Failed { code: exit } => {
                eprintln!("{task}: failed with exit code {exit}");
                code = 1;
            }
        }
    }
    Ok(code)
}

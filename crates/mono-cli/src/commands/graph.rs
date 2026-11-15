//! `mono graph`: print the project graph in dependency order.

use std::env;

use crate::context::{find_workspace_root, load_workspace, CliError};

/// Prints one line per project, dependencies first: `name` or `name -> dep, ...`.
pub fn graph() -> Result<i32, CliError> {
    let cwd = env::current_dir()?;
    let root = find_workspace_root(&cwd)?;
    let workspace = load_workspace(&root)?;
    for name in workspace.graph.topo_order() {
        let deps = workspace.graph.deps_of(name);
        if deps.is_empty() {
            println!("{name}");
        } else {
            println!("{name} -> {}", deps.join(", "));
        }
    }
    Ok(0)
}

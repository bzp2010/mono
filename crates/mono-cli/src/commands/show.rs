//! `mono show`: print the metadata of the workspace's projects.

use std::env;
use std::path::Path;

use mono_graph::Project;
use serde_json::{json, Value};

use crate::context::{find_workspace_root, load_workspace, CliError, Workspace};

/// Prints the project names, one per line, or as a JSON array.
pub fn projects(as_json: bool) -> Result<i32, CliError> {
    let workspace = load()?;
    let names: Vec<&str> = workspace.graph.projects().map(|p| p.name.as_str()).collect();
    if as_json {
        println!("{}", json!(names));
    } else {
        for name in names {
            println!("{name}");
        }
    }
    Ok(0)
}

/// Prints the metadata of one project, as text or as a JSON object.
pub fn project(name: &str, as_json: bool) -> Result<i32, CliError> {
    let workspace = load()?;
    let Some(project) = workspace.graph.project(name) else {
        let known: Vec<&str> = workspace.graph.projects().map(|p| p.name.as_str()).collect();
        return Err(CliError::UnknownProject { name: name.to_string(), known: known.join(", ") });
    };
    let view = describe(&workspace, project);
    if as_json {
        println!("{view}");
    } else {
        print_text(&view);
    }
    Ok(0)
}

fn load() -> Result<Workspace, CliError> {
    let cwd = env::current_dir()?;
    let root = find_workspace_root(&cwd)?;
    load_workspace(&root)
}

/// The metadata of one project. Both output formats are rendered from this value.
fn describe(workspace: &Workspace, project: &Project) -> Value {
    let targets: serde_json::Map<String, Value> = project
        .targets
        .iter()
        .map(|(name, target)| {
            let value = json!({
                "command": target.command,
                "inputs": target.inputs,
                "outputs": target.outputs,
                "dependsOn": target.depends_on,
            });
            (name.clone(), value)
        })
        .collect();
    json!({
        "name": project.name,
        "package": project.package_name,
        "version": project.version,
        "ecosystem": project.ecosystem.as_str(),
        "root": relative_root(&workspace.root, &project.root),
        "dependencies": workspace.graph.deps_of(&project.name),
        "dependents": workspace.graph.dependents_of(&project.name),
        "targets": targets,
    })
}

/// The project root relative to the workspace root, with `.` for the root itself.
/// Paths are compared as given first and canonicalized second, because the working
/// directory and the discovered roots can spell the same directory differently.
fn relative_root(workspace: &Path, root: &Path) -> String {
    let rel = root.strip_prefix(workspace).map(Path::to_path_buf).or_else(|_| {
        let workspace = workspace.canonicalize()?;
        let root = root.canonicalize()?;
        root.strip_prefix(&workspace)
            .map(Path::to_path_buf)
            .map_err(|_| std::io::Error::other("outside the workspace"))
    });
    match rel {
        Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
        Ok(rel) => rel.display().to_string(),
        Err(_) => root.display().to_string(),
    }
}

fn print_text(view: &Value) {
    let text = |key: &str| view[key].as_str().unwrap_or("(none)").to_string();
    println!("name: {}", text("name"));
    println!("package: {}", text("package"));
    println!("version: {}", text("version"));
    println!("ecosystem: {}", text("ecosystem"));
    println!("root: {}", text("root"));
    println!("dependencies: {}", list(&view["dependencies"]));
    println!("dependents: {}", list(&view["dependents"]));
    let targets = view["targets"].as_object().expect("targets is an object");
    if targets.is_empty() {
        println!("targets: (none)");
        return;
    }
    println!("targets:");
    for (name, target) in targets {
        println!("  {name}: {}", target["command"].as_str().unwrap_or_default());
        for (label, key) in [("inputs", "inputs"), ("outputs", "outputs"), ("dependsOn", "dependsOn")] {
            if !target[key].as_array().is_some_and(Vec::is_empty) {
                println!("    {label}: {}", list(&target[key]));
            }
        }
    }
}

/// A JSON array of strings as a comma-separated list, or `(none)` when empty.
fn list(value: &Value) -> String {
    let items: Vec<&str> = value.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if items.is_empty() { "(none)".to_string() } else { items.join(", ") }
}

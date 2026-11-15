//! Task graph: expands `dependsOn` into an ordered list of tasks.

use std::collections::BTreeMap;
use std::fmt;

use crate::project_graph::{GraphError, ProjectGraph};

/// One task: a target of one project.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId {
    pub project: String,
    pub target: String,
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.project, self.target)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    InProgress,
    Done,
}

/// Plans `target` across the graph: every project that defines it, plus the
/// tasks its `dependsOn` pulls in, ordered so dependencies come first.
///
/// Projects are visited in topological order, so the result is deterministic.
pub fn plan(graph: &ProjectGraph, target: &str) -> Result<Vec<TaskId>, GraphError> {
    let mut marks: BTreeMap<TaskId, Mark> = BTreeMap::new();
    let mut stack: Vec<TaskId> = Vec::new();
    let mut order = Vec::new();
    for name in graph.topo_order() {
        let defines = graph
            .project(name)
            .is_some_and(|p| p.targets.contains_key(target));
        if defines {
            let task = TaskId { project: name.to_string(), target: target.to_string() };
            visit(graph, task, &mut marks, &mut stack, &mut order)?;
        }
    }
    Ok(order)
}

fn visit(
    graph: &ProjectGraph,
    task: TaskId,
    marks: &mut BTreeMap<TaskId, Mark>,
    stack: &mut Vec<TaskId>,
    order: &mut Vec<TaskId>,
) -> Result<(), GraphError> {
    match marks.get(&task) {
        Some(Mark::Done) => return Ok(()),
        Some(Mark::InProgress) => {
            let start = stack.iter().position(|t| *t == task).unwrap_or(0);
            let mut path: Vec<String> = stack[start..].iter().map(ToString::to_string).collect();
            path.push(task.to_string());
            return Err(GraphError::TaskCycle { path });
        }
        None => {}
    }
    marks.insert(task.clone(), Mark::InProgress);
    stack.push(task.clone());
    for dep in task_dependencies(graph, &task)? {
        visit(graph, dep, marks, stack, order)?;
    }
    stack.pop();
    marks.insert(task.clone(), Mark::Done);
    order.push(task);
    Ok(())
}

/// Resolves one task's `dependsOn` entries into concrete tasks.
///
/// `^name` expands to `name` in each direct dependency that defines it, and is
/// skipped where it does not. A bare `name` refers to this project and must exist.
/// The tasks that `task` directly depends on, as declared by its `dependsOn`.
pub fn task_dependencies(graph: &ProjectGraph, task: &TaskId) -> Result<Vec<TaskId>, GraphError> {
    let project = graph.project(&task.project).expect("task refers to a graph project");
    let config = project.targets.get(&task.target).expect("planned target exists");
    let mut out = Vec::new();
    for entry in &config.depends_on {
        if let Some(name) = entry.strip_prefix('^') {
            for dep in graph.deps_of(&task.project) {
                let defines = graph.project(dep).is_some_and(|p| p.targets.contains_key(name));
                if defines {
                    out.push(TaskId { project: dep.clone(), target: name.to_string() });
                }
            }
        } else {
            if !project.targets.contains_key(entry) {
                return Err(GraphError::MissingTarget {
                    project: task.project.clone(),
                    target: entry.clone(),
                });
            }
            out.push(TaskId { project: task.project.clone(), target: entry.clone() });
        }
    }
    Ok(out)
}

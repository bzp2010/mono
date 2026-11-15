//! Project graph and task graph.

mod project_graph;
mod task_graph;

pub use project_graph::{build, GraphError, Project, ProjectGraph};
pub use task_graph::{plan, task_dependencies, TaskId};

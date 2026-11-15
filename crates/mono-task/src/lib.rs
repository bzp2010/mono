//! Executes target commands for planned tasks.

mod runner;

pub use runner::{run_plan, run_plan_in_workspace, run_task, RunError, TaskOutcome};

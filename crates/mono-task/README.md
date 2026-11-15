# mono-task

Runs target commands and combines execution with the cache.

## What it owns

- `run_task`: runs a command through `sh -c` in the project directory and reports `Success` or `Failed { code }`.
- `run_plan`: runs a planned task list in order and stops after the first failure. The failed task is still included in the result.
- `TaskOutcome::Cached`: a task whose outputs were restored from the cache. Its command did not run.

## Pitfalls

- When a cache backend is in use and the target declares outputs, every file matching those outputs is removed before the cache is consulted. A restored snapshot is therefore the complete output set, and files left by another version cannot survive.
- A task with `outputs` is skipped on a cache hit. A task with only `cacheDirs` always runs its command. Its directories are restored before the run and saved after a successful run, so the tool can reuse its own incremental state.
- A target without outputs and without a cache backend always runs. Its key still contributes to dependents.
- A command killed by a signal is `RunError::Terminated`, not an exit code. The distinction keeps a crash from looking like a normal failure.
- Commands run sequentially. There is no parallelism yet.

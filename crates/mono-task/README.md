# mono-task

Runs target commands and combines execution with the cache.

## What it owns

- `run_task`: runs a command through `sh -c` in the project directory and reports `Success` or `Failed { code }`.
- `run_plan`: runs a planned task list in order and stops after the first failure. The failed task is still included in the result.
- `TaskOutcome::Cached`: a task whose outputs were restored from the cache. Its command did not run.

## Pitfalls

- On a cache hit, every file matching the target's outputs is removed before the snapshot is restored, so the restored set is the complete output set. On a miss, existing outputs are left alone, because the command may be incremental and rely on them.
- A task with `outputs` is skipped on a cache hit. A task without outputs always runs its command. Tools that keep their own incremental state, such as Cargo's `target/`, are not declared as outputs, so their commands always run and manage their own state.
- A target without outputs and without a cache backend always runs. Its key still contributes to dependents.
- A command killed by a signal is `RunError::Terminated`, not an exit code. The distinction keeps a crash from looking like a normal failure.
- Commands run sequentially. There is no parallelism yet.

# mono-cli

The `mono` binary. It parses arguments, finds the workspace, and calls the library crates. It holds no logic of its own beyond output formatting.

## Subcommands

- `mono run <target>`: runs the target across the graph. Prints `<project>:<target>: ok` or `: cached` for each executed task.
- `mono run <target> --affected [--base <rev>]`: runs the target only in projects affected by changes, which are the changed projects and the projects that depend on them. A change to a workspace input such as `Cargo.lock` or `pnpm-lock.yaml` affects every project of its ecosystem, and `mono.json` affects every project. The dependencies of the affected tasks are planned too, since their outputs and cache keys are inputs of the affected tasks. Changes are measured against `<rev>`, which defaults to `HEAD`, so uncommitted changes count. In CI, pass the base branch as `--base`.
- `mono graph`: prints one line per project in dependency order, as `name` or `name -> dep1, dep2`.
- `mono --version`.

## Exit codes

- `0`: success. Projects without the target are skipped and do not count as failures.
- `1`: a task ran and failed. The failing task is reported on stderr.
- `2`: usage, configuration, discovery, or I/O error. The message starts with `mono:`.

## Pitfalls

- The workspace is the nearest ancestor with `mono.json`. Running from a subdirectory works, and project paths are still resolved from the workspace.
- The local cache lives in `<workspace>/.mono/cache`, which is git-ignored.
- `--affected` needs a git repository. Outside one, it is a usage error with exit code 2. Plain `mono run` does not need git.
- `context.rs` holds `load_workspace`, which every subcommand uses. Add new fields to `Workspace` only when a subcommand reads them.

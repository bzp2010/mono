# mono-cli

The `mono` binary. It parses arguments, finds the workspace, and calls the library crates. It holds no logic of its own beyond output formatting.

## Subcommands

- `mono run <target>`: runs the target across the graph. Prints `<project>:<target>: ok` or `: cached` for each executed task.
- `mono graph`: prints one line per project in dependency order, as `name` or `name -> dep1, dep2`.
- `mono --version`.

## Exit codes

- `0`: success. Projects without the target are skipped and do not count as failures.
- `1`: a task ran and failed. The failing task is reported on stderr.
- `2`: usage, configuration, discovery, or I/O error. The message starts with `mono:`.

## Pitfalls

- The workspace is the nearest ancestor with `mono.json`. Running from a subdirectory works, and project paths are still resolved from the workspace.
- The local cache lives in `<workspace>/.mono/cache`, which is git-ignored.
- `context.rs` holds `load_workspace`, which every subcommand uses. Add new fields to `Workspace` only when a subcommand reads them.

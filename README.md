# mono

`mono` is a monorepo management tool written in Rust. It discovers projects from the workspace signals of each ecosystem, builds a project graph, runs the shell commands that projects declare as targets, and caches outputs where that is safe.

Status: early development. The tool already manages this repository. Expect breaking changes. There is no stable API or configuration promise before 1.0.

## Concepts

- **Workspace**: a directory containing `mono.json`. `mono` finds it by walking up from the current directory.
- **Project**: a unit discovered from a workspace signal (a Cargo workspace, a pnpm or npm workspace member). Each project may have a `project.json`.
- **Target**: a named shell command declared in `project.json`. `mono run <target>` runs it in every project that defines it and skips the others.
- **Cache**: the outputs of a target are cached under a key derived from its inputs. A target with no outputs is never cached, and its command always runs.

## Usage

```sh
mono graph            # print projects, dependencies first
mono run build        # run the build target across the graph
```

## Documentation map

- [mono-core](crates/mono-core/README.md): configuration files and the shared project model.
- [mono-discover](crates/mono-discover/README.md): reading Cargo, pnpm, and npm workspaces.
- [mono-graph](crates/mono-graph/README.md): the project graph and the task plan.
- [mono-cache](crates/mono-cache/README.md): cache keys, digests, and cache backends.
- [mono-task](crates/mono-task/README.md): running commands and combining them with the cache.
- [mono-cli](crates/mono-cli/README.md): the `mono` binary, subcommands, and exit codes.

## Developing mono

`mono` manages this repository itself. The root `project.json` declares `build` and `test` targets, so `mono run build` and `mono run test` work from the root. Running `mono run test` inside an automated test would contend for the Cargo build lock, so the self-host test only checks `mono graph`.

## License

`mono` is free software licensed under the GNU Affero General Public License, version 3 or (at your option) any later version. See [LICENSE](LICENSE).

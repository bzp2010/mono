# mono-discover

Finds projects from each ecosystem's own workspace definition. It never guesses: if a signal is absent, no edge is created.

## What it owns

- `discover(root, ecosystems)`: dispatches to the enabled ecosystems.
- Cargo: runs `cargo metadata` and returns the whole workspace as one project.
- pnpm: reads `pnpm-workspace.yaml` (YAML). A dependency is an edge only when its spec starts with `workspace:` and names another member.
- npm and yarn: reads `workspaces` from the root `package.json`. A dependency is an edge when its spec equals a member's exact version, or is a `file:` path that resolves to the member directory.

## Pitfalls

- A Cargo workspace is one project. Crate-to-crate path dependencies are not recorded, because the graph has no crate-level nodes.
- The Cargo project is named after the workspace root directory. Renaming the directory renames the project, since `project.json` has no name field yet.
- A workspace member directory without `package.json` is an error (`MissingManifest`), not a silent skip.
- Semver ranges such as `^1.0.0` that resolve to a local member are not detected. Only exact versions and `file:` paths are edges.
- Workspace patterns are expanded with the `glob` crate. Patterns are relative to the workspace root.

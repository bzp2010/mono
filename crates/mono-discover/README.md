# mono-discover

Finds projects from each ecosystem's own workspace definition. It never guesses: if a signal is absent, no edge is created.

## What it owns

- `discover(root, ecosystems)`: dispatches to the enabled ecosystems.
- Cargo: runs `cargo metadata` and returns the whole workspace as one project.
- pnpm: reads `pnpm-workspace.yaml` (YAML). Every package is a project: the root package when the root has a `package.json`, and each member. A dependency is an edge only when its spec starts with `workspace:`. `workspace:foo@*` is an alias that resolves to package `foo`. A path reference such as `workspace:../@scope/foo` names the dependency key, so a scope in the path is not mistaken for an alias.
- npm and yarn: reads `workspaces` from the root `package.json`. A dependency is an edge when its spec equals a member's exact version, or is a `file:` path that resolves to the member directory.

## Pitfalls

- A Cargo workspace is one project, named after the workspace root directory. Crate-to-crate path dependencies are not recorded, because the graph has no crate-level nodes. Renaming the directory renames the project, since `project.json` has no name field yet.
- Discovery never invents a name. A pnpm root package without a `name` is discovered unnamed, and so has no name here. The graph reports it unless `project.json` names it. A member without a `name` is an error.
- Each discovered project has two names. `name` is the project's name, and `package_name` is the name that other packages use in dependency specs. Dependencies are recorded as package names, and the graph maps them to project names.
- Workspace patterns starting with `!` exclude directories, as pnpm does. Exclusions are applied before member manifests are checked, so an excluded directory never needs a `package.json`.
- A matched member directory without `package.json` is an error (`MissingManifest`), not a silent skip.
- Semver ranges such as `^1.0.0` that resolve to a local member are not detected. Only exact versions and `file:` paths are edges.
- Workspace patterns are relative to the workspace root. The root path is escaped before the pattern is joined, so a root such as `ws[1]` matches literally.

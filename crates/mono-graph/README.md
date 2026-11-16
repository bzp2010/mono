# mono-graph

Turns discovered projects and explicit config into a validated graph, and plans task order for one target name.

## What it owns

- `build(discovered, configs)`: resolves each project's name, maps discovered dependencies from package names to project names, merges explicit dependencies, then rejects unnamed projects, duplicate names, unknown dependencies, and cycles. The result is `ProjectGraph`, with a topological order where dependencies come first.
- `plan(graph, target)`: collects every project that defines the target and expands `dependsOn` into `TaskId`s, dependencies first.
- `ProjectGraph`: besides lookup and `topo_order`, `deps_of` gives the direct dependencies of a project and `dependents_of` the direct reverse edges, in name order. A `Project` carries its package name and version for display and release planning.
- `task_dependencies(graph, task)`: the direct task dependencies of one task. The cache uses it to build dependent keys.

## Pitfalls

- `^name` is skipped in a dependency project that does not define `name`. This is intentional, so a library need not define every target.
- A bare `name` in `dependsOn` must exist in the same project. A missing one is `MissingTarget`, because a typo there would otherwise vanish silently.
- A cycle among tasks in one project is `TaskCycle`. The project graph is acyclic by construction, but `dependsOn` edges inside a project are checked here.
- Projects are visited in topological order, so plans are deterministic.
- `affected(graph, workspace_root, changed)`: the projects that changed files belong to, the projects of an ecosystem whose workspace input changed, every project if `mono.json` changed, and every project that depends on the affected ones, directly or through other projects. A file belongs to the projects whose root is its most specific ancestor. Several projects can share a root, and all of them own its files. Files outside every project belong to none.

## Pitfalls

- `affected` takes absolute paths in the same form as project roots. Relative paths match nothing.
- Workspace inputs are a fixed table in `affected.rs`. Lockfiles and toolchain files (`Cargo.lock`, `rust-toolchain`, `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, and similar) affect only the projects of their ecosystem. `mono.json` affects every project. A workspace input is recognised only directly under the workspace root, and it belongs to no project, so a Cargo project that shares the root with a pnpm package is not affected by the pnpm lockfile.
- Other files outside every project, such as a README, affect nothing.
- Dependencies of an affected project are not affected by it, but they are still planned by `--affected`. The affected task's cache key includes its dependencies' outputs, so the dependencies must be evaluated. An unchanged dependency is a cache hit.

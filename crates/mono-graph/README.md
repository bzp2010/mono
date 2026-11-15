# mono-graph

Turns discovered projects and explicit config into a validated graph, and plans task order for one target name.

## What it owns

- `build(discovered, configs)`: merges inferred and explicit dependencies, then rejects duplicate names, unknown dependencies, and cycles. The result is `ProjectGraph`, with a topological order where dependencies come first.
- `plan(graph, target)`: collects every project that defines the target and expands `dependsOn` into `TaskId`s, dependencies first.
- `task_dependencies(graph, task)`: the direct task dependencies of one task. The cache uses it to build dependent keys.

## Pitfalls

- `^name` is skipped in a dependency project that does not define `name`. This is intentional, so a library need not define every target.
- A bare `name` in `dependsOn` must exist in the same project. A missing one is `MissingTarget`, because a typo there would otherwise vanish silently.
- A cycle among tasks in one project is `TaskCycle`. The project graph is acyclic by construction, but `dependsOn` edges inside a project are checked here.
- Projects are visited in topological order, so plans are deterministic.

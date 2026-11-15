# mono-core

Configuration types and the shared project model. Other crates depend on this one and it depends on nothing in the workspace.

## What it owns

- `mono.json` (`MonoConfig`): the list of ecosystems to discover. Only `cargo`, `pnpm`, and `npm` are valid. Unknown keys are rejected.
- `project.json` (`ProjectConfig`, `TargetConfig`): explicit dependencies and targets. Each target has a `command` and optional `inputs`, `outputs`, and `dependsOn`. Unknown keys are rejected, so native tool fields fail early.
- `DiscoveredProject`: a project as found by discovery, before explicit config is merged.

## Pitfalls

- Missing files are not errors for `project.json` (`load_project_config` returns `None`), but they are errors for `mono.json` (`ConfigError::NotFound`).
- `dependsOn` strings are stored verbatim. The leading `^` is interpreted in `mono-graph`, not here.
- Names in `ecosystems` are lowercase on disk and in the serialized form.
- `project.json` may set `name`. It overrides the name the ecosystem gives, and it is the name other projects use in `dependencies` and `dependsOn`. An empty name is rejected.

# mono-release

Release planning. It reads change records and computes the next version of every project that a release touches.

## What it owns

- `parse_record`: reads a change record. It is a markdown file that starts with a frontmatter block, which maps project names to `major`, `minor`, or `patch`. The rest of the file is the summary. Delimiter lines must be exactly `---`, and both LF and CRLF line endings are accepted.
- `compute_plan`: turns the records into a release plan. Each project gets the largest bump that any record gives it. A project that depends on a released project gets at least a `patch` bump, transitively.
- `apply_plan`: writes a plan into manifest files. For each released project it sets the new version in `package.json` (`version`) or `Cargo.toml` (`[package].version`, or `[workspace.package].version` that members inherit). It also moves internal dependency specs that name a released project at its old version. The caller passes an `ApplyTarget` per project: its `ManifestKind`, the manifests it owns, and the package names other manifests use for it (every member crate for a Cargo workspace). Package names are scoped by kind, so an npm package and a Cargo crate with the same name keep separate releases.
- `Release`: the current version, the next version, and the bump of one project.

## Pitfalls

- A record for a project that has no version is an error. So is a bump keyword other than `major`, `minor`, or `patch`.
- Propagation follows the dependents passed in. The caller builds that map from the project graph. Each project is processed once, so a cyclic map terminates, but the project graph itself rejects cycles.
- Only the plain `major.minor.patch` arithmetic is done. Pre-release and build metadata are dropped from the next version.
- `compute_plan` writes nothing. `apply_plan` rewrites manifests only. The changelog and tags come in later steps.
- A dependency spec moves when it is the old version, optionally behind `^`, `~`, or `=`. A spec that gives only leading components, such as `0.1` or `1`, follows the same number of leading components of the new version. Other ranges, `workspace:`, `file:`, and `peerDependencies` are left alone.
- `package.json` is reserialised, keeping key order, the indentation it already uses, CRLF line endings, and a final newline. Compact one-line objects are expanded. `Cargo.toml` is edited in place, so comments and layout stay.
- `apply_plan` updates every manifest in memory first, so a parse or lookup error writes nothing. The writes themselves are sequential, so an I/O failure partway through can leave some files updated. It does not touch lockfiles. Run the ecosystem's install afterwards to refresh them.
- A released project must get a literal version written in at least one of its manifests, or `apply_plan` returns `MissingVersion` and writes nothing. For a Cargo workspace whose members use `version.workspace = true`, the target must therefore include the manifest that defines `[workspace.package].version`.

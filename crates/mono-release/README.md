# mono-release

Release planning. It reads change records and computes the next version of every project that a release touches.

## What it owns

- `parse_record`: reads a change record. It is a markdown file that starts with a frontmatter block, which maps project names to `major`, `minor`, or `patch`. The rest of the file is the summary.
- `compute_plan`: turns the records into a release plan. Each project gets the largest bump that any record gives it. A project that depends on a released project gets at least a `patch` bump, transitively.
- `Release`: the current version, the next version, and the bump of one project.

## Pitfalls

- A record for a project that has no version is an error. So is a bump keyword other than `major`, `minor`, or `patch`.
- Propagation follows the dependents passed in. The caller builds that map from the project graph.
- Only the plain `major.minor.patch` arithmetic is done. Pre-release and build metadata are dropped from the next version.
- The plan does not write anything. Applying it to manifests, the changelog, and tags comes in later steps.

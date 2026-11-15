# mono-vcs-git

Git integration. It answers one question: which files changed, relative to a revision.

## What it owns

- `changed_files(dir, base)`: the union of tracked files that differ from the merge base of `base` and `HEAD`, in the working tree, and untracked files that are not ignored. Paths are absolute, under the repository's top-level directory.

## Pitfalls

- The result includes uncommitted changes, both staged and unstaged, so it describes the working tree as it is now.
- Comparing against the merge base means that changes the base branch received after the branch was cut are not counted as changes of this branch.
- When there is no merge base, for example in an unrelated history or a shallow clone without the common ancestor, the call is an error. It never falls back to comparing with the base tip, which could report no change where there is one.
- `base` must resolve to a commit. It is passed after `--end-of-options`, so a name that is also a path or an option is still a revision. An unknown revision is an error, and so is a directory outside a repository.
- Paths come from git with `-z`, so names with spaces or newlines are handled. Renames are reported as a deletion and an addition, so both paths count.
- Only the local git command is used. There is no network access and no git library.

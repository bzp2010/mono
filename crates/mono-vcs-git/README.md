# mono-vcs-git

Git integration. It answers one question: which files changed, relative to a revision.

## What it owns

- `changed_files(dir, base)`: the union of tracked files that differ from `base` in the working tree, and untracked files that are not ignored. Paths are absolute, under the repository's top-level directory.

## Pitfalls

- The result includes uncommitted changes, both staged and unstaged, so it describes the working tree as it is now.
- `base` is any revision git understands. An unknown revision is an error, and so is a directory outside a repository.
- Paths come from git with `-z`, so names with spaces or newlines are handled.
- Only the local git command is used. There is no network access and no git library.

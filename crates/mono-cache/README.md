# mono-cache

Cache keys, output digests, and the pluggable backend trait. Only the local directory backend is implemented.

## What it owns

- `task_key`: SHA-256 over the task id, the command, the declared output patterns, the content of every input file, and the dependency contributions. Each field is length-prefixed.
- `output_digest`: digest of the output file paths and contents. A dependent uses it in place of the dependency's key when the dependency declares outputs.
- `expand_files` and `remove_matching`: resolve output patterns against a project and remove them.
- `remove_stale`: after a restore, removes the output files that the restored entry does not contain.
- `CacheBackend`: `contains`, `get` (restore, returning the restored files), and `put` (store).
- `LocalBackend`: stores each key as the directory `dir/<key>/`, holding the output files at their project-relative paths.
- `confine::Tree` (crate-private): the only place where mono touches a path inside a project. Every read, write, and removal goes through it.

Only output files are cached. Tools that keep their own state, such as Cargo's `target/`, are not outputs and are not cached by mono. Their commands always run.

## Entries

- An entry is built in a staging directory inside the cache directory and renamed into place in one step. An entry that exists is therefore always complete. A failed build leaves nothing behind.
- Entries are immutable. The first entry for a key wins. A publish that loses the race is a success, because the same key names the same content.
- A restore walks the entry and installs each file at its project-relative path. A file that disappears while the entry is read, because the entry was removed, is a miss. Any other missing path is an error.

## The confinement rules

These rules are enforced by `Tree`, and callers cannot bypass them.

- Patterns and relative paths must be relative and must not contain `..`. A character class that could match `..`, such as `.[.]`, is caught because every matched path is checked too.
- A matched or read file must resolve inside the project. A link to a file outside the project is an error.
- Special files such as named pipes, and dangling links, are errors wherever they are matched. They are never skipped silently.
- Writes and removals never traverse a symbolic link. Removing through a link is an error.
- Restoring replaces a link or file in an ancestor position with a real directory before anything is written beneath it. Writes never go through a link.

The rules are tested in `tests/boundary.rs`. Each test builds one adversarial layout and checks that a sentinel directory outside the project is untouched. `tests/concurrency.rs` covers concurrent publishes and restores, and `tests/local.rs` covers the entry lifecycle.

These guarantees assume that no other process changes the project tree while mono runs. The checks are path-based, so a concurrent process that swaps a directory for a link between a check and its use can still race them. mono does not use `openat` or `O_NOFOLLOW` today.

## Pitfalls

- A trailing `/**` means every file beneath the directory. The `glob` crate matches only directories for a trailing `**`, so `x/**` is rewritten to `x/**/*`.
- Output declarations are part of the key. Changing `outputs` invalidates the entry.
- Outputs may not pass through a symbolic link. Storing and removing such a path both fail with a clear error, so the failure appears on the first run.
- A cache key must be a plain name of letters, digits, `-`, or `_`.
- Each file copy goes through its own uniquely named temporary file in the destination directory and is renamed into place. The source permissions are kept, so restored executables stay executable. Writing in place would fail with ETXTBSY when the destination is a running binary.
- Two restores of one entry can run at the same time. Directory creation and placeholder replacement tolerate the race.
- Crashed runs can leave a staging directory behind. Nothing removes it yet.
- An entry is stored only when the task succeeded and matched at least one output file. An empty output set is never cached.

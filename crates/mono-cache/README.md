# mono-cache

Cache keys, digests, and the pluggable backend trait. Only the local directory backend is implemented.

## What it owns

- `task_key`: SHA-256 over the task id, the command, the declared output patterns, the content of every input file, and the dependency contributions. Each field is length-prefixed. `cacheDirs` are not part of the key.
- `output_digest`: digest of output file paths and contents. A dependent uses it in place of the dependency's key when the dependency declares outputs.
- `dir_key`: key for one `cacheDirs` entry, derived from the target key and the directory path.
- `expand_files`, `remove_matching`, `remove_stale`: resolve patterns against a project and remove outputs.
- `check_cache_dir`: validates a `cacheDirs` name without touching the filesystem.
- `CacheBackend`: `get` and `put` for file lists, `get_dir` and `put_dir` for trees, `contains` and `stored_files` for inspecting an entry.
- `LocalBackend`: stores each key as `dir/<key>/`.
- `confine::Tree` (crate-private): the only place where mono touches a path inside a project. Every read, write, and removal goes through it.

## The confinement rules

These rules are enforced by `Tree`. Callers cannot bypass them.

- Patterns and relative paths must be relative and must not contain `..`. A character class that could match `..`, such as `.[.]`, is caught because every matched path is checked too.
- A matched or read file must resolve inside the project. A link to a file outside the project is an error.
- Special files such as named pipes, and dangling links, are errors wherever they are matched. They are never skipped silently.
- Removal and writes never traverse a symbolic link. Removing through a link is an error.
- Restoring replaces a link or a file in an ancestor position with a real directory before anything is written beneath it. Removal failures are reported unless another restore has already replaced the path, and no write goes through a link.
- Symbolic links inside a cached tree are stored and restored as links. They are not followed, and empty directories are kept.

The rules are tested in `tests/boundary.rs`. Each test builds one adversarial layout and checks that a sentinel directory outside the project is untouched. `tests/concurrency.rs` and `tests/cache_dirs.rs` cover concurrent publishes and restores.

These guarantees assume that no other process changes the project tree while mono runs. The checks are path-based, so a concurrent process that swaps a directory for a link between a check and its use can still race them. mono does not use `openat` or `O_NOFOLLOW` today.

## Entries and their lifecycle

- An entry is a directory that carries a completion marker. A directory without the marker is not an entry, so a crash or a manual edit cannot make a damaged entry look complete. The marker is bookkeeping and is never restored.
- An entry is built in a private staging directory inside the cache directory, marked complete, and renamed into place. A failed build leaves no entry, and a reader never sees a partial one.
- Output entries are immutable. The first complete entry for a key wins, and a later publish of the same key discards its staged build.
- `cacheDirs` entries are replaced by each publish. Their key does not cover the content, so the newest snapshot must win. The previous entry is moved aside first, so a reader sees either the old complete entry or the new one.
- A restore that finds a file missing, because the entry was replaced or damaged while it ran, reports a miss. The caller then runs the command and publishes a fresh entry.

## Pitfalls

- A trailing `/**` means every file beneath the directory. The `glob` crate matches only directories for a trailing `**`, so `x/**` is rewritten to `x/**/*`.
- Output declarations are part of the key. Changing `outputs` invalidates the entry.
- `cacheDirs` keys come from the target key, which covers `inputs` only. Declare the files that should invalidate the tool cache, such as `Cargo.lock`, as inputs. Do not declare source files there, or the tool cache is invalidated on every edit.
- A cache directory name is checked before the cache is consulted. An empty name, `.`, `..`, and an absolute path are rejected on every run, including an outputs hit.
- A cache key must be a plain name of letters, digits, `-`, or `_`.
- Outputs and `cacheDirs` may not pass through a symbolic link. Storing and removing such a path both fail with a clear error, so the failure appears on the first run.
- Each file copy goes through its own uniquely named temporary file in the destination directory and is renamed into place. The source permissions are kept, so restored executables stay executable. Writing in place would fail with ETXTBSY when the destination is a running binary, which happens when `target/` contains the `mono` executable.
- Two restores of one entry can run at the same time. Directory creation and placeholder replacement tolerate the race.
- Crashed runs can leave a staging directory or a trash directory behind. Nothing removes them yet.
- An entry is stored only when the task succeeded and matched at least one output file. An empty output set is never cached.

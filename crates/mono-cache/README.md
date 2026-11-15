# mono-cache

Cache keys, digests, and the pluggable backend trait. Only the local directory backend is implemented.

## What it owns

- `task_key`: SHA-256 over the task id, the command, the declared output patterns, the content of every input file, and the dependency contributions. Each field is length-prefixed.
- `output_digest`: digest of output file paths and contents. A dependent uses it in place of the dependency's key when the dependency declares outputs.
- `dir_key`: key for one `cacheDirs` entry, derived from the target key and the directory path.
- `expand_files` and `remove_matching`: resolve patterns against a project and remove outputs.
- `CacheBackend`: `get` and `put` for file lists, `get_dir` and `put_dir` for trees, and `contains` for complete entries.
- `LocalBackend`: stores each key as `dir/<key>/`.
- `confine::Tree` (crate-private): the only place where mono touches a path inside a project. Every read, write, and removal goes through it.

## The confinement rules

These rules are enforced by `Tree`. Callers cannot bypass them.

- Patterns and relative paths must be relative and must not contain `..`. A character class that could match `..`, such as `.[.]`, is caught because every matched path is checked too.
- A matched or read file must resolve inside the project. A link to a file outside the project is an error.
- Removal and writes never traverse a symbolic link. Removing through a link is an error.
- Restoring replaces a link or a file in an ancestor position with a real directory before anything is written beneath it. A restore never writes through a link.
- Symbolic links inside a cached tree are stored and restored as links. They are not followed, and empty directories are kept.

The rules are tested in `tests/boundary.rs`. Each test builds one adversarial layout and checks that a sentinel directory outside the project is untouched.

## Pitfalls

- A trailing `/**` means every file beneath the directory. The `glob` crate matches only directories for a trailing `**`, so `x/**` is rewritten to `x/**/*`.
- Output declarations are part of the key. Changing `outputs` invalidates the entry.
- `cacheDirs` keys come from the target key, which covers `inputs` only. Declare the files that should invalidate the tool cache, such as `Cargo.lock`, as inputs. Do not declare source files there, or the tool cache is invalidated on every edit.
- Outputs and `cacheDirs` may not pass through a symbolic link. Such a path fails with a clear error instead of being followed.
- An entry is published only when it is complete. It is built in a private staging directory inside the cache directory and renamed into place. A failed build leaves no entry, and a reader never sees a partial one.
- Each file copy goes through its own uniquely named temporary file in the destination directory and is renamed into place. The source permissions are kept, so restored executables stay executable. Writing in place would fail with ETXTBSY when the destination is a running binary, which happens when `target/` contains the `mono` executable.
- Directory creation tolerates a concurrent creation of the same directory, so two restores of one entry can run at the same time.
- An entry is stored only when the task succeeded and matched at least one output file. An empty output set is never cached.

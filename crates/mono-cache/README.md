# mono-cache

Cache keys, digests, and the pluggable backend trait. Only the local directory backend is implemented.

## What it owns

- `task_key`: SHA-256 over the task id, the command, the declared output patterns, the content of every input file, and the dependency contributions. Each field is length-prefixed.
- `output_digest`: digest of output file paths and contents. A dependent uses it in place of the dependency's key when the dependency declares outputs.
- `dir_key`: key for one `cacheDirs` entry, derived from the target key and the directory path.
- `expand_files`: resolves input and output patterns relative to the project root. `check_relative` rejects absolute patterns and any `..` component, for inputs, outputs, and `cacheDirs` alike.
- `contains`: reports whether a complete entry exists for a key, without restoring it.
- `CacheBackend`: `get` and `put` for file lists, `get_dir` and `put_dir` for trees.
- `LocalBackend`: stores each key as `dir/<key>/`.

## Pitfalls

- A trailing `/**` means every file beneath the directory. The `glob` crate matches only directories for a trailing `**`, so `expand_files` rewrites `x/**` to `x/**/*`.
- The project root is escaped before it is joined with a pattern, so a root such as `repo[1]` matches literally.
- Output declarations are part of the key. Changing `outputs` invalidates the entry.
- `cacheDirs` keys come from the target key, which covers `inputs` only. Declare the files that should invalidate the tool cache, such as `Cargo.lock`, as inputs. Do not declare source files there, or the tool cache is invalidated on every edit.
- An entry is published only when it is complete. It is built in a private staging directory inside the cache directory and renamed into place. A failed build leaves no entry, and a reader never sees a partial one.
- Symlinks in a cached tree are stored as links and restored as links. They are never followed, and empty directories are kept.
- Restoring a file or link over a real directory removes that directory first. Links are never followed while doing so.
- Each file copy goes through its own uniquely named temporary file in the destination directory, then is renamed into place. The source permissions are kept, so restored executables stay executable. Writing in place would fail with ETXTBSY when the destination is a running binary, which happens when `target/` contains the `mono` executable.
- An entry is stored only when the task succeeded and matched at least one output file. An empty output set is never cached.

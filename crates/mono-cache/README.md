# mono-cache

Cache keys, digests, and the pluggable backend trait. Only the local directory backend is implemented.

## What it owns

- `task_key`: SHA-256 over the task id, the command, the content of every input file, and the dependency contributions. Each field is length-prefixed.
- `output_digest`: digest of output file paths and contents. A dependent uses it in place of the dependency's key when the dependency declares outputs.
- `dir_key`: key for one `cacheDirs` entry, derived from the target key and the directory path.
- `CacheBackend`: `get` and `put` for file lists, `get_dir` and `put_dir` for trees.
- `LocalBackend`: stores each key as `dir/<key>/`.

## Pitfalls

- A trailing `/**` means every file beneath the directory. The `glob` crate matches only directories for a trailing `**`, so `expand_files` rewrites `x/**` to `x/**/*`. Without this, outputs were silently empty and caching never hit.
- `cacheDirs` keys come from the target key, which covers `inputs` only. Declare the files that should invalidate the tool cache, such as `Cargo.lock`, as inputs. Do not declare source files there, or the tool cache is invalidated on every edit.
- Restoring a file writes a temporary sibling and renames it into place. Writing in place fails with ETXTBSY when the destination is a running binary, which happens when `target/` contains the `mono` executable itself.
- A failed or interrupted restore can leave `.<name>.mono-tmp` files behind. They are harmless and are overwritten on the next restore.
- An entry is stored only when the task succeeded and matched at least one output file. An empty output set is never cached.

# duplicate_file_finder

A small Rust CLI that walks a directory tree and either lists the largest files or finds and deletes byte-identical duplicates.

## Build

```sh
cargo build --release
```

The binary lands at `target/release/duplicate_file_finder`.

## Usage

```
duplicate_file_finder [OPTIONS] --path <PATH>
```

| Flag | Description |
| --- | --- |
| `-p`, `--path <PATH>` | Root directory to scan (required). |
| `-c`, `--clean` | Duplicate-cleaning mode. Conflicts with `--sort`. |
| `-r`, `--real-run` | Actually delete files. Without it, clean mode is a dry run. |
| `-e`, `--remove-empty-files` | Also consider (and delete) zero-byte files. |
| `-s`, `--sort` | Sorting mode: print the largest files. This is the default when `--clean` is absent. |
| `-n`, `--num-sorting <N>` | How many files to print in sorting mode (default: 20). |

### Sorting mode

Prints the largest files under `<PATH>`, biggest first, with sizes in MB:

```sh
duplicate_file_finder -s -p ~/Downloads -n 10
```

### Clean mode

Groups files by size, compares same-size files byte for byte, and removes the later copy of each match. It defaults to a dry run — nothing is deleted until you pass `--real-run`:

```sh
# see what would be removed
duplicate_file_finder -c -p ~/Downloads

# actually remove the duplicates
duplicate_file_finder -c -p ~/Downloads --real-run

# include zero-byte files
duplicate_file_finder -c -p ~/Downloads -e --real-run
```

**Deletion is permanent** — files are removed with `remove_file`, not moved to the trash. Always do a dry run first.

## How it works

1. **Walk** — an iterative traversal of the tree collects `(size, path)` pairs.
2. **Group** — paths are bucketed into a `HashMap` keyed by file size; only buckets with more than one entry can contain duplicates.
3. **Compare** — candidates are read in 4 KB chunks and compared as they go, so differing files bail out early instead of being hashed or read in full. Sizes are re-checked at open time in case a file changed since the walk.
4. **Delete** — for each confirmed pair, the second path is deleted (or reported, in a dry run).

### What gets skipped

- Hidden entries (names starting with `.`) and directories named `target`.
- Symlinks — their metadata describes the link, not the target.
- Hard links (`nlink > 1`), so the tool never breaks a shared inode.
- In clean mode, macOS bundle directories (`.app`, `.framework`, `.bundle`, `.xpc`, `.plugin`, `.appex`, `.kext`, `.prefPane`, `.qlgenerator`), since deleting files inside them can break the corresponding app.
- Unreadable directories and files with inaccessible metadata are reported on stderr and skipped rather than aborting the run.

## Platform

Unix only — link counts come from `std::os::unix::fs::MetadataExt`. Requires a Rust toolchain supporting edition 2024.

# File System Scanner

A small Rust terminal UI (built with [ratatui](https://ratatui.rs)) that walks a directory tree and either lists the largest files or finds and deletes byte-identical duplicates.

## Build and run

```sh
cargo build --release
./target/release/duplicate_file_finder
```

Or just `cargo run --release`. The app takes no command-line arguments; everything is configured from inside the UI.

## Usage

The app opens on a main menu with two modes: **Clean** and **Sort**. Scans run on a background thread, so the UI stays responsive and shows a spinner and a live log while it works.

### Clean mode

Finds duplicate files under a directory and removes (or reports) the extra copies.

1. Pick **Clean** from the main menu.
2. Set the options:
   - **Real run** — actually delete files. Off by default, which makes the run a dry run that only reports what _would_ be removed.
   - **Remove empty files** — also consider (and delete) zero-byte files.
   - **Path** — the root directory to scan. Type it directly into the box; the border turns green when it's a valid directory and red when it isn't.
3. Move to **Run** and press Enter.

The running screen shows whether this is a dry run or a real run, plus a live log of what the scan is doing. When it finishes, the results screen opens with two panes:

- **Success Output** — the files that were removed (or would be, in a dry run), each with the file it duplicates.
- **Error Output** — files or directories that couldn't be read, opened, or deleted. Anything listed here was not fully checked, so the run may have missed duplicates in it.

**Deletion is permanent.** Files are removed with `remove_file`, not moved to the trash, and there is no confirmation step once you press Run with Real run enabled. Always do a dry run first.

### Sort mode

Lists the largest files under a directory, biggest first, with sizes in MB.

1. Pick **Sort** from the main menu.
2. Enter the **path** to scan and **how many files** to show (default: 20).
3. Press Enter.

The list appears in the results screen's Success Output pane.

### Keys

| Screen        | Keys                                                                                                                                                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Main menu     | `j`/`k`, `↑`/`↓`, or `Tab`/`Shift+Tab` to move · `Enter` to select · `q` to quit                                                                                                                                   |
| Clean options | `Tab`/`↓` and `Shift+Tab`/`↑` to move between controls (`j`/`k` also work outside the path box) · `Space` toggles a checkbox · type to edit the path · `Enter` on **Run** starts the scan · `Esc` back to the menu |
| Sort options  | `Tab`/`Shift+Tab` or `↑`/`↓` to switch fields · type to edit · `Enter` to run · `Esc` back to the menu                                                                                                             |
| Running       | `q` to quit                                                                                                                                                                                                        |
| Results       | `j`/`k` or `↑`/`↓` to scroll · `PgUp`/`PgDn` to jump · `Tab` to switch between Success and Error panes · `q` to quit                                                                                               |

Quitting while a scan is running stops it immediately. During a real run, that means deletion stops partway through.

## How it works

1. **Walk** — an iterative traversal of the tree collects `(size, path)` pairs.
2. **Group** — paths are bucketed into a `HashMap` keyed by file size; only buckets with more than one entry can contain duplicates.
3. **Compare** — the first 4 KB of every file in a bucket is read once and cached, so most non-matching pairs are ruled out without reopening files. Pairs whose prefixes match are then compared in 4 KB chunks from that point on, bailing out at the first difference instead of hashing or reading the whole file. Sizes are re-checked at open time in case a file changed since the walk.
4. **Delete** — for each confirmed pair, the second path is deleted (or reported, in a dry run).

The scan runs on a worker thread and sends progress lines and the final report back to the UI over a channel.

### What gets skipped

- Hidden entries (names starting with `.`) and directories named `target`.
- Symlinks — their metadata describes the link, not the target.
- Hard links (`nlink > 1`), so the tool never breaks a shared inode.
- In clean mode, macOS bundle directories (`.app`, `.framework`, `.bundle`, `.xpc`, `.plugin`, `.appex`, `.kext`, `.prefPane`, `.qlgenerator`), since deleting files inside them can break the corresponding app.
- Unreadable directories and files with inaccessible metadata are skipped rather than aborting the run, and listed in the Error Output pane.

## Platform

Unix only — link counts come from `std::os::unix::fs::MetadataExt`. Requires a Rust toolchain supporting edition 2024.

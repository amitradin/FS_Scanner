# File System Scanner

A small Rust terminal UI (built with [ratatui](https://ratatui.rs)) that walks a directory tree and either lists the largest files or finds byte-identical duplicates and empty files, letting you delete them one at a time.

## Build and run

```sh
cargo build --release
./target/release/FS_Scanner
```

Or just `cargo run --release`. The app takes no command-line arguments; everything is configured from inside the UI.

## Usage

The app opens on a main menu with two modes: **Clean** and **Sort**. Scans run on a background thread, so the UI stays responsive and shows a spinner and a live log while it works.

### Clean mode

Finds duplicate and empty files under a directory. The scan itself never deletes anything; it only builds a list you can review and act on.

1. Pick **Clean** from the main menu.
2. Type the root directory into the **Path** box. The border turns green when it's a valid directory and red when it isn't.
3. Move to **Run** and press Enter.

When the scan finishes, the results screen opens with two panes:

- **Success Output** — every duplicate found, each shown with the file it duplicates, plus every zero-byte file. In each group of identical files, the first one found is treated as the original and is not listed.
- **Error Output** — files or directories that couldn't be read or opened. Anything listed here was not fully checked, so the scan may have missed duplicates in it.

To delete a file, select it in the Success Output pane, press `d`, then `y` to confirm (or `n` to cancel). The file disappears from the list once it's deleted; if the delete fails, it stays in the list.

**Deletion is permanent.** Files are removed with `remove_file`, not moved to the trash.

### Sort mode

Lists the largest files under a directory, biggest first, with sizes in MB.

1. Pick **Sort** from the main menu.
2. Enter the **path** to scan and **how many files** to show (default: 20).
3. Press Enter.

The list appears in the results screen's Success Output pane.

### Log screen

Press `l` on the results screen to see the full log of the last scan (directories visited, size groups, errors). `Esc` goes back to the results.

### Keys

| Screen        | Keys                                                                                                                                                              |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Main menu     | `j`/`k`, `↑`/`↓`, or `Tab`/`Shift+Tab` to move · `Enter` to select · `q` to quit                                                                                  |
| Clean options | `Tab`/`↓` and `Shift+Tab`/`↑` to move between Path and Run (`j`/`k` also work on Run) · type to edit the path · `Enter` in the path box jumps to Run · `Enter` on Run starts the scan · `Esc` back to the menu |
| Sort options  | `Tab`/`Shift+Tab` or `↑`/`↓` to switch fields · type to edit · `Enter` to run · `Esc` back to the menu                                                            |
| Running       | `q` to quit                                                                                                                                                       |
| Results       | `j`/`k` or `↑`/`↓` to scroll · `PgUp`/`PgDn` to jump to the top/bottom · `Tab` to switch panes · `d` to delete the selected file (then `y`/`n`) · `l` for the log · `Esc` back to the menu · `q` to quit |
| Log           | `j`/`k` or `↑`/`↓` to scroll · `PgUp`/`PgDn` to jump to the top/bottom · `Esc` back to the results · `q` to quit                                                   |

Going back to the main menu from the results screen clears the current results.

## How it works

1. **Walk** — the tree is walked in parallel with [rayon](https://github.com/rayon-rs/rayon): each directory is its own task, so idle threads pick up subdirectories from busy ones. This collects `(size, path)` pairs.
2. **Group by size** — paths are bucketed by file size. Only buckets with more than one file can contain duplicates; the zero-byte bucket is kept regardless so empty files get reported.
3. **Group by prefix** — within each bucket, the first 4 KB of every file is read once and hashed with [BLAKE3](https://github.com/BLAKE3-team/BLAKE3), splitting the bucket into groups with matching prefixes. For files of 4 KB or less, that's the whole file, so a matching prefix means a duplicate.
4. **Confirm** — for larger files, a group of exactly two is compared directly in 4 KB chunks from byte 4096 onward, stopping at the first difference. Groups of three or more are fully hashed with BLAKE3 instead, so each file is read once rather than once per pair.

Buckets are processed in parallel. File sizes are re-checked when each file is opened, in case it changed since the walk. The worker thread sends progress lines and the final report back to the UI over a channel.

### What gets skipped

- Hidden entries (names starting with `.`) and directories named `target`.
- Symlinks — their metadata describes the link, not the target.
- Hard links (`nlink > 1`), so the tool never breaks a shared inode.
- In clean mode, macOS bundle directories (`.app`, `.framework`, `.bundle`, `.xpc`, `.plugin`, `.appex`, `.kext`, `.prefPane`, `.qlgenerator`), since they often contain duplicates on purpose and deleting files inside them can break the app.
- Unreadable directories and files with inaccessible metadata are skipped rather than aborting the scan, and listed in the Error Output pane.

## Platform

Unix only — link counts come from `std::os::unix::fs::MetadataExt`. Requires a Rust toolchain supporting edition 2024.

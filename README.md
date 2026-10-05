# File System Scanner

A small Rust terminal UI (built with [ratatui](https://ratatui.rs)) for finding what's taking up disk space. It can find byte-identical duplicates and empty files, list the largest files under a directory, or let you browse the whole file system sorted by size. In every mode you can delete files one at a time.

Example of FS Walk:
<img width="1280" height="720" alt="rec" src="https://github.com/user-attachments/assets/5b01e313-264c-4d8c-810d-a198013af7ff" />



## Build and run

```sh
cargo build --release
./target/release/FS_Scanner
```

Or just `cargo run --release`. The app takes no command-line arguments; everything is configured from inside the UI.

## Usage

The main menu has three modes: **Find Dups**, **Sort**, and **FS Walk**. Scans run on a background thread, so the UI stays responsive and shows a spinner while it works. Find Dups and Sort also show a live log.

### Find Dups

Finds duplicate and empty files under a directory. The scan itself never deletes anything; it only builds a list you can review and act on.

1. Pick **Find Dups** from the main menu.
2. Type the root directory into the **Path** box. The border turns green when it's a valid directory and red when it isn't.
3. Move to **Run** and press Enter.

When the scan finishes, the results screen opens with two panes:

- **Success**: every duplicate found, each shown with the file it duplicates, plus every zero-byte file. In each group of identical files, the first one found is treated as the original and is not listed.
- **Errors**: files or directories that couldn't be read or opened. Anything listed here was not fully checked, so the scan may have missed duplicates in it.

### Sort

Lists the largest files under a directory, biggest first, with sizes in MB.

1. Pick **Sort** from the main menu.
2. Enter the **path** to scan and **how many files** to show (default: 20).
3. Press Enter. Nothing happens until the path is a valid directory and the count is a number.

The list appears in the Success pane of the results screen, and unreadable entries appear in the Errors pane.

### FS Walk

Browse the whole file system as a tree, sorted by size at every level, similar to `ncdu`.

1. Pick **FS Walk** from the main menu. There are no options; the scan always starts at `/`.
2. Wait for the scan to finish. On a full disk this can take a while.
3. Browse. Each entry shows its total size (MB below 1 GB, GB above) and path. Directories are shown in bold and files are dimmed.

Press Enter or `l` to open a directory and `h` to go back up. Sizes are the space actually allocated on disk (`blocks × 512`), so sparse files and filesystem compression make them differ from what Sort reports. Pressing `d` on a file deletes it, and its size is subtracted from every parent directory's total. Directories can't be deleted.

The walk includes hidden files and hard links, unlike the other modes, but skips symlinks and these mount points and virtual paths: `/System/Volumes`, `/Volumes`, `/dev`, `/.nofollow`, `/.resolve`, `/.vol`. Directories it can't read are shown as empty and aren't listed as errors. The tree is built once, so changes made outside the app after the scan don't show up until you run it again.

### Deleting files

On the Find Dups or Sort results screen, select a file in the Success pane and press `d`. In FS Walk, select a file and press `d`. Then press `y` to confirm or `n` to cancel. A message shows whether the delete worked. If it did, the file is removed from the list. If it failed, the file stays.

**Deletion is permanent.** Files are removed with `remove_file`, not moved to the trash.

### Log screen

Press `l` on the results screen to see the full log of the last Find Dups or Sort scan (directories visited, size groups, errors). `Esc` goes back to the results.

### Keys

| Screen            | Keys                                                                                                                                                                                                      |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Main menu         | `j`/`k`, `↑`/`↓`, or `Tab`/`Shift+Tab` to move · `Enter` to select · `q` to quit                                                                                                                          |
| Find Dups options | `Tab`/`↓` and `Shift+Tab`/`↑` to move between Path and Run (`j`/`k` also work on Run) · type to edit the path · `Enter` in the path box jumps to Run · `Enter` on Run starts the scan · `Esc` back to the menu |
| Sort options      | `Tab`/`Shift+Tab` or `↑`/`↓` to switch fields · type to edit · `Enter` to run · `Esc` back to the menu                                                                                                    |
| Running / loading | `q` to quit                                                                                                                                                                                               |
| Results           | `j`/`k` or `↑`/`↓` to scroll · `PgUp`/`PgDn` to jump to the top/bottom · `Tab` to switch panes · `d` to delete the selected file (then `y`/`n`) · `l` for the log · `Esc` back to the menu · `q` to quit   |
| Log               | `j`/`k` or `↑`/`↓` to scroll · `PgUp`/`PgDn` to jump to the top/bottom · `Esc` back to the results · `q` to quit                                                                                           |
| FS Walk           | `j`/`k`, `↑`/`↓`, or `Tab`/`Shift+Tab` to move · `PgUp`/`PgDn` to jump to the top/bottom · `Enter`/`l` to open a directory · `h` to go up · `d` to delete the selected file (then `y`/`n`) · `Esc` back to the menu · `q` to quit |

Going back to the main menu from the results or FS Walk screen discards the current results and the walked tree.

## How it works

### Find Dups

1. **Walk**: the tree is walked in parallel with [rayon](https://github.com/rayon-rs/rayon). Each directory is its own task, so idle threads pick up subdirectories from busy ones. This collects `(size, path)` pairs.
2. **Group by size**: paths are bucketed by file size. Only buckets with more than one file can contain duplicates. The zero-byte bucket is kept even if it has one file, so empty files get reported.
3. **Group by prefix**: within each bucket, the first 4 KB of every file is read once and hashed with [BLAKE3](https://github.com/BLAKE3-team/BLAKE3), splitting the bucket into groups with matching prefixes. For files of 4 KB or less, that's the whole file, so a matching prefix means a duplicate.
4. **Confirm**: for larger files, a group of exactly two is compared directly in 4 KB chunks from byte 4096 onward, stopping at the first difference. Groups of three or more are fully hashed with BLAKE3 instead, so each file is read once rather than once per pair.

Buckets are processed in parallel. File sizes are re-checked when each file is opened, in case they changed since the walk. The worker thread sends progress lines and the final report back to the UI over a channel.

Sort uses the same walk, then sorts the files by size and keeps the top N.

### FS Walk

The tree is built recursively from `/`, reading each directory's entries in parallel with rayon. Each node stores its path, its allocated size, and, for directories, its children sorted largest-first. A directory's size is its own blocks plus the sum of its children. The finished tree is sent to the UI over a channel, and navigation keeps a stack of child indices into it.

### What Find Dups and Sort skip

- Hidden entries (names starting with `.`) and directories named `target`.
- Symlinks, because their metadata describes the link, not the target.
- Hard links (`nlink > 1`), so the tool never breaks a shared inode.
- In Find Dups only, macOS bundle directories (`.app`, `.framework`, `.bundle`, `.xpc`, `.plugin`, `.appex`, `.kext`, `.prefPane`, `.qlgenerator`), since they often contain duplicates on purpose and deleting files inside them can break the app.
- Unreadable directories and files with inaccessible metadata. The scan skips them instead of stopping and lists them in the Errors pane.

## Platform

Unix only, because link counts and block sizes come from `std::os::unix::fs::MetadataExt`. The FS Walk skip list is aimed at macOS. Requires a Rust toolchain supporting edition 2024.

The slow FS Walk test scans the whole disk and is ignored by default. Run it with `cargo test -- --ignored`.

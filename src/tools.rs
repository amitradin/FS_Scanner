use std::collections::{HashMap, hash_map};
use std::fs;
use std::fs::{DirEntry, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

use crate::Message;
use std::sync::mpsc::Sender;

#[derive(Debug)]
pub struct CleanReport {
    pub success: Vec<String>,
    pub errors: Vec<String>,
}

pub fn clean_main(
    path: &Path,
    remove_empty: bool,
    real_run: bool,
    sender: Sender<Message>,
) -> Result<CleanReport, String> {
    let mut fail = Vec::new();
    let _ = sender.send(Message::Log(String::from("Starting to populate paths")));
    let (files, mut err) = populate_paths(path, remove_empty, true, sender.clone())?;
    fail.append(&mut err);
    let (succ, mut err) = run_clean(files, remove_empty, real_run, sender.clone())?;
    fail.append(&mut err);
    Ok(CleanReport {
        success: succ,
        errors: fail,
    })
}

pub fn sort_main(
    path: &Path,
    num_sorting: usize,
    sender: Sender<Message>,
) -> Result<CleanReport, String> {
    let (mut files, mut failed) = populate_paths(path, false, false, sender.clone())?;
    let mut fail = Vec::new();
    fail.append(&mut failed);
    let _ = sender.send(Message::Log(format!(
        "Starting to sort the files, total files in comparing: {}",
        files.len()
    )));
    let succ = run_sort(&mut files, num_sorting);
    Ok(CleanReport {
        success: succ,
        errors: fail,
    })
}
pub fn run_clean(
    files: Vec<(u64, PathBuf)>,
    remove_empty: bool,
    real_run: bool,
    sender: Sender<Message>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let files = group_into_similar(files, sender.clone());
    let mut succ = Vec::new();
    let mut err = Vec::new();
    for item in files {
        let (mut success, mut fail) = scan_and_clean(
            item.1,
            item.0 as usize,
            remove_empty,
            real_run,
            sender.clone(),
        )?;
        succ.append(&mut success);
        err.append(&mut fail)
    }
    Ok((succ, err))
}
pub fn run_sort(files: &mut [(u64, PathBuf)], num_sorting: usize) -> Vec<String> {
    let mut res = Vec::new();
    files.sort_by_key(|a| std::cmp::Reverse(a.0));
    let len = num_sorting.min(files.len());
    for i in 0..len {
        let curr = files.get(i).unwrap();
        res.push(format!(
            "{:.2}MB : {:?}",
            (curr.0 as f64 / 1_000_000.0),
            curr.1
        ))
    }
    res
}

/// Scans the FS from the root provided by the user. The Scan is being done via BFS
pub fn populate_paths(
    path: &Path,
    remove_empty: bool,
    is_clean: bool,
    sender: Sender<Message>,
) -> Result<(Vec<(u64, PathBuf)>, Vec<String>), String> {
    const BUNDLE_EXTS: &[&str] = &[
        "app",
        "framework",
        "bundle",
        "xpc",
        "plugin",
        "appex",
        "kext",
        "prefPane",
        "qlgenerator",
    ];
    let mut err = Vec::new();
    let mut dir_paths: Vec<PathBuf> = vec![path.to_path_buf()];
    let mut file_paths: Vec<(u64, PathBuf)> = Vec::new();
    while let Some(curr_dir) = dir_paths.pop() {
        let _ = sender.send(Message::Log(format!("Starting to scan {:?}", curr_dir,)));
        let tester = fs::read_dir(&curr_dir);
        if let Err(e) = tester {
            err.push(format!("Could not read {:?}, got an error {e}", curr_dir));
            continue;
        }
        let curr_dir: Vec<DirEntry> = tester
            .unwrap() // Safe becuase checked
            .filter_map(|entry| entry.ok().filter(|e| e.file_type().is_ok()))
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|s| {
                        !s.starts_with('.')
                            && (entry.file_type().unwrap().is_file() || s != "target")
                    })
                    .unwrap_or(false)
            })
            .collect();
        for entry in curr_dir {
            // this will not fail as we filtered for erros in file_type
            let file_type = entry.file_type().unwrap();
            // We should skip symlinks since the metadata is fucked.
            if file_type.is_symlink() {
                continue;
            }

            let metadata = entry.metadata();
            if let Err(e) = metadata {
                err.push(format!(
                    "Could not access the metadata of {:?}, got an error {e}",
                    entry.path()
                ));
                continue;
            }
            let metadata = metadata.unwrap();
            if metadata.file_type().is_file() {
                // HardLink
                if metadata.nlink() > 1 {
                    continue;
                } else if metadata.len() > 0 || remove_empty {
                    file_paths.push((metadata.len(), entry.path()));
                }
            } else if metadata.is_dir() {
                let path = entry.path();
                let mut is_bundle = false;
                // When cleaning, we don't want to scan for bundles, as those directories usually
                // contain duplicates
                if is_clean {
                    is_bundle = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| BUNDLE_EXTS.iter().any(|b| b.eq_ignore_ascii_case(e)));
                }
                if is_bundle {
                    continue;
                }
                dir_paths.push(entry.path());
            }
        }
    }
    Ok((file_paths, err))
}

fn group_into_similar(
    files: Vec<(u64, PathBuf)>,
    sender: Sender<Message>,
) -> HashMap<u64, Vec<PathBuf>> {
    let _ = sender.send(Message::Log(String::from(
        "Starting to group files into similar sizes",
    )));
    let mut mapping: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for item in files {
        if let hash_map::Entry::Vacant(e) = mapping.entry(item.0) {
            e.insert(vec![item.1]);
        } else {
            mapping.get_mut(&item.0).unwrap().push(item.1)
        }
    }
    for (key, val) in &mapping {
        let _ = sender.send(Message::Log(format!(
            "Group sized {key} had {} elements",
            val.len()
        )));
    }

    mapping
}
/* since we don't want to compare the entire files at once (can be very wastful) we should instead
* check each chunk at a time. so I'll read 4KB at a time */
fn compare_2_files(file1: &mut File, file2: &mut File, len: usize) -> Result<bool, std::io::Error> {
    let mut remain = len;
    let mut chunk1 = [0u8; 4096];
    let mut chunk2 = [0u8; 4096];

    // If size has changed since gropued
    if file1.metadata()?.len() != file2.metadata()?.len() {
        return Ok(false);
    }
    while remain > 0 {
        let n = remain.min(4096);
        file1.read_exact(&mut chunk1[0..n])?;
        file2.read_exact(&mut chunk2[0..n])?;
        if chunk1[0..n] != chunk2[0..n] {
            return Ok(false);
        }
        remain -= n;
    }

    Ok(true)
}

fn scan_and_clean(
    files: Vec<PathBuf>,
    len: usize,
    remove_empty: bool,
    real_run: bool,
    sender: Sender<Message>,
) -> Result<(Vec<String>, Vec<String>), String> {
    // A cache of first 4096 bytes of the file. This can help on small files. This can reduce the
    // total reads from O(n^2) to O(n)
    let mut succ: Vec<String> = Vec::new();
    let mut fail: Vec<String> = Vec::new();
    let mut index_to_first_hash: HashMap<usize, [u8; 4096]> = HashMap::new();
    let prefix = len.min(4096);
    fail.append(&mut read_first_4096_bytes(
        &files,
        &mut index_to_first_hash,
        prefix,
        sender.clone(),
    ));
    if len == 0 && remove_empty {
        let _ = sender.send(Message::Log(String::from("Starting to scan empty files")));
        return delete_empty(files, real_run, sender.clone());
    }
    let mut gone_over = vec![false; files.len()];
    for i in 0..files.len() {
        if gone_over[i] {
            continue;
        }
        let curr_file = &files[i];
        for j in (i + 1)..files.len() {
            if gone_over[j] {
                continue;
            }
            let compare = &files[j];
            if !index_to_first_hash.contains_key(&i) || !index_to_first_hash.contains_key(&j) {
                continue;
            }

            // this unwrap will not failed as we checked if the keys are indeed in the map
            let mut comp = index_to_first_hash.get(&i).unwrap()[0..prefix]
                == index_to_first_hash.get(&j).unwrap()[0..prefix];
            if !comp {
                continue;
            }

            // if the file size is <= 4096 the the prefix check is all that was needed, and we do
            // not need to read the entire file.
            // Otherwise, We don't want to read the entire first 4096 bytes again so we should start
            // from byte 4096
            if len > 4096 {
                let file1 = File::open(curr_file);
                if let Err(e) = file1 {
                    fail.push(format!("Could not open {:?}, got an error: {e}", curr_file));
                    continue;
                }
                let mut file1 = file1.unwrap();
                let seek = file1.seek(SeekFrom::Start(4096));
                if let Err(e) = seek {
                    fail.push(format!("Could not seek {:?}, got an error: {e}", curr_file));
                    continue;
                }
                let file2 = File::open(compare);

                if let Err(e) = file2 {
                    fail.push(format!("Could not open {:?}, got an error: {e}", compare));
                    continue;
                }
                let mut file2 = file2.unwrap();
                let seek = file2.seek(SeekFrom::Start(4096));
                if let Err(e) = seek {
                    fail.push(format!("Could not seek {:?}, got an error: {e}", compare));
                    continue;
                }
                let _ = sender.send(Message::Log(format!(
                    "Comparing {:?} and {:?}",
                    curr_file, compare
                )));
                let comp2 = compare_2_files(&mut file1, &mut file2, len - 4096);
                if let Err(e) = comp2 {
                    fail.push(format!(
                        "Could not compare {:?}, {:?}, got an error {e}",
                        curr_file, compare
                    ));
                    continue;
                }
                comp = comp2.unwrap();
            }

            if comp {
                if real_run {
                    if let Err(e) = fs::remove_file(compare) {
                        fail.push(format!(
                            "Could not remove file {compare:?}, got an error: {e}"
                        ));
                        continue;
                    } else {
                        succ.push(format!(
                            "Removing file \n{:?} \nit is equal to \n{:?}\n",
                            compare, curr_file
                        ));
                    }
                } else {
                    succ.push(format!(
                        "This is a dry run, Would remove file \n{:?} \nit is equal to \n{:?}\n",
                        compare, curr_file
                    ));
                }
                gone_over[j] = true;
            }
        }
    }
    Ok((succ, fail))
}

fn read_first_4096_bytes(
    files: &Vec<PathBuf>,
    mapping: &mut HashMap<usize, [u8; 4096]>,
    len: usize,
    sender: Sender<Message>,
) -> Vec<String> {
    let mut fail = Vec::new();
    for i in 0..files.len() {
        let mut buf = [0u8; 4096];
        let curr = File::open(&files[i]);
        if let Err(e) = curr.as_ref() {
            fail.push(format!("Could not open {:?}, got an error: {e}", files[i]));
            continue;
        }
        let mut curr = curr.unwrap();

        let read = curr.read_exact(&mut buf[0..len]);
        if let Err(e) = read {
            fail.push(format!("Could not read {:?}, got an error: {e}", files[i]));
            continue;
        }
        let _ = sender.send(Message::Log(format!(
            "Cacheing first 4096 bytes of {:?}",
            files[i]
        )));
        mapping.insert(i, buf);
    }
    fail
}
fn delete_empty(
    empty_files: Vec<PathBuf>,
    real_run: bool,
    sender: Sender<Message>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut succ: Vec<String> = Vec::new();
    let mut fail: Vec<String> = Vec::new();

    for path in empty_files {
        let _ = sender.send(Message::Log(format!(
            "Checking to see if {:?} is actually empty before deleting",
            path
        )));
        let file = File::open(&path);
        if let Err(e) = file.as_ref() {
            fail.push(format!("Could not open {:?}, got an error: {e}", path));
            continue;
        }
        let file = file.unwrap();

        let metadata = file.metadata();
        if let Err(e) = metadata {
            fail.push(format!(
                "Could not access the metadata of {:?}, got an error: {e}",
                path
            ));
            continue;
        }
        let metadata = metadata.unwrap();
        if metadata.len() == 0 {
            if real_run {
                if let Err(e) = fs::remove_file(&path) {
                    fail.push(format!(
                        "Could not remove file {:?}, got an error: {e}",
                        path
                    ));

                    continue;
                } else {
                    succ.push(format!("Removed {:?} : sized 0", path));
                }
            } else {
                succ.push(format!(
                    "This is a dry run, would remove {:?} : sized 0",
                    path
                ));
            }
        }
    }
    Ok((succ, fail))
}

use std::collections::{HashMap, hash_map};
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

use crate::structs_and_enums::Message;
use rayon::prelude::*;
use std::sync::mpsc::Sender;

const BUF_SIZE: usize = 1024 * 1024; //1 MB

type ComplaxType = Vec<(Vec<(String, PathBuf)>, Vec<String>)>;

#[derive(Debug)]
pub struct CleanReport {
    pub success: Vec<(String, PathBuf)>,
    pub errors: Vec<String>,
}

pub fn clean_main(path: &Path, sender: Sender<Message>) -> Result<CleanReport, String> {
    let mut fail = Vec::new();
    let _ = sender.send(Message::Log(String::from("Starting to populate paths")));
    let (files, mut err) = populate_paths(path, true, sender.clone());
    fail.append(&mut err);
    let (succ, mut err) = run_clean(files, sender.clone());
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
    let (mut files, mut failed) = populate_paths(path, false, sender.clone());
    let mut fail = Vec::new();
    fail.append(&mut failed);
    let _ = sender.send(Message::Log(format!(
        "Starting to sort the files, total files in comparing: {}",
        files.len()
    )));
    let succ = run_sort(&mut files, num_sorting);
    Ok(CleanReport {
        success: (succ),
        errors: fail,
    })
}
pub fn run_clean(
    files: Vec<(u64, PathBuf)>,
    sender: Sender<Message>,
) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let files = group_into_similar(files, sender.clone());
    let files = files.into_iter().filter(|(size, vec)| {
        if *size > 0u64 {
            vec.len() > 1usize
        } else {
            true
        }
    });

    let mut succ_fin: Vec<(String, PathBuf)> = Vec::new();
    let mut fail_fin: Vec<String> = Vec::new();

    let groups: Vec<_> = files.collect();
    let results: ComplaxType = groups
        .into_par_iter()
        .map(|item| {
            let mut succ = Vec::new();
            let mut err = Vec::new();
            if item.0 == 0 {
                let (mut success, mut fail) =
                    scan_and_clean(item.1, item.0 as usize, sender.clone());

                succ.append(&mut success);
                err.append(&mut fail)
            } else {
                let prefix_groups = group_by_prefix(item.1, item.0, &mut err);
                for group in prefix_groups {
                    let dup_group = if item.0 as usize <= 4096 {
                        vec![group]
                    } else if group.len() == 2 {
                        match tails_equal(&group[0], &group[1], item.0) {
                            Ok(true) => vec![group],
                            Ok(false) => vec![],
                            Err(e) => {
                                err.push(format!(
                                    "Could not compare {:?}, and {:?}, got an error: {e}",
                                    group[0], group[1]
                                ));
                                vec![]
                            }
                        }
                    } else {
                        group_by_hash(group, item.0, &mut err)
                    };
                    for vec in dup_group {
                        remove_by_hash(vec, &mut succ);
                    }
                }
            }
            (succ, err)
        })
        .collect();
    for (mut succ, mut err) in results {
        succ_fin.append(&mut succ);
        fail_fin.append(&mut err);
    }

    (succ_fin, fail_fin)
}
pub fn run_sort(files: &mut [(u64, PathBuf)], num_sorting: usize) -> Vec<(String, PathBuf)> {
    let mut res = Vec::new();
    files.sort_by_key(|a| std::cmp::Reverse(a.0));
    let len = num_sorting.min(files.len());
    for i in 0..len {
        let curr = files.get(i).unwrap();
        res.push((
            format!("{:.2}MB : {:?}", (curr.0 as f64 / 1_000_000.0), curr.1),
            PathBuf::default(),
        ))
    }
    res
}

// When cleaning, we don't want to scan bundles, as those directories usually contain duplicates
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

/// Scans the FS from the root provided by the user. Every directory is its own rayon task, so idle
/// threads steal subdirectories from busy ones and uneven trees still split evenly.
pub fn populate_paths(
    path: &Path,
    is_clean: bool,
    sender: Sender<Message>,
) -> (Vec<(u64, PathBuf)>, Vec<String>) {
    walk(path.to_path_buf(), is_clean, &sender)
}

/// Scans `dir`, then walks all of its subdirectories in parallel and merges their results.
fn walk(
    dir: PathBuf,
    is_clean: bool,
    sender: &Sender<Message>,
) -> (Vec<(u64, PathBuf)>, Vec<String>) {
    let (mut files, subdirs, mut err) = scan_dir(&dir, is_clean, sender);
    let nested: Vec<_> = subdirs
        .into_par_iter()
        .map(|sub| walk(sub, is_clean, sender))
        .collect();
    for (mut sub_files, mut sub_err) in nested {
        files.append(&mut sub_files);
        err.append(&mut sub_err);
    }
    (files, err)
}

/// Reads a single directory. Returns its files, the subdirectories to descend into, and errors.
fn scan_dir(
    dir: &Path,
    is_clean: bool,
    sender: &Sender<Message>,
) -> (Vec<(u64, PathBuf)>, Vec<PathBuf>, Vec<String>) {
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    let mut err = Vec::new();
    let _ = sender.send(Message::Log(format!("Starting to scan {:?}", dir)));
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            err.push(format!("Could not read {:?}, got an error {e}", dir));
            return (files, subdirs, err);
        }
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        // Skip hidden entries and `target` directories.
        // We should skip symlinks since the metadata describes the link, not the target.
        if name.starts_with('.')
            || (!file_type.is_file() && name == "target")
            || file_type.is_symlink()
        {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(e) => {
                err.push(format!(
                    "Could not access the metadata of {:?}, got an error {e}",
                    entry.path()
                ));
                continue;
            }
        };
        if metadata.is_file() {
            // HardLink
            if metadata.nlink() == 1 {
                files.push((metadata.len(), entry.path()));
            }
        } else if metadata.is_dir() {
            let path = entry.path();
            let is_bundle = is_clean
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| BUNDLE_EXTS.iter().any(|b| b.eq_ignore_ascii_case(e)));
            if !is_bundle {
                subdirs.push(path);
            }
        }
    }
    (files, subdirs, err)
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
    sender: Sender<Message>,
) -> (Vec<(String, PathBuf)>, Vec<String>) {
    // A cache of first 4096 bytes of the file. This can help on small files. This can reduce the
    // total reads from O(n^2) to O(n)
    let mut fail: Vec<String> = Vec::new();
    let mut index_to_first_hash: HashMap<usize, [u8; 4096]> = HashMap::new();
    let prefix = len.min(4096);
    fail.append(&mut read_first_4096_bytes(
        &files,
        &mut index_to_first_hash,
        prefix,
        sender.clone(),
    ));
    let _ = sender.send(Message::Log(String::from("Starting to scan empty files")));
    delete_empty(files, sender.clone())
}

fn read_first_4096_bytes(
    files: &[PathBuf],
    mapping: &mut HashMap<usize, [u8; 4096]>,
    len: usize,
    sender: Sender<Message>,
) -> Vec<String> {
    let mut fail = Vec::new();
    for (i, file) in files.iter().enumerate() {
        let mut buf = [0u8; 4096];
        let curr = File::open(&files[i]);
        if let Err(e) = curr.as_ref() {
            fail.push(format!("Could not open {:?}, got an error: {e}", file));
            continue;
        }
        let mut curr = curr.unwrap();

        let read = curr.read_exact(&mut buf[0..len]);
        if let Err(e) = read {
            fail.push(format!("Could not read {:?}, got an error: {e}", file));
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
    sender: Sender<Message>,
) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let mut succ: Vec<(String, PathBuf)> = Vec::new();
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
            succ.push((format!("{:?} : sized 0", path), path));
        }
    }
    (succ, fail)
}

fn hash_file(
    path: &Path,
    len: u64,
) -> Result<blake3::Hash, Box<dyn std::error::Error + Send + Sync>> {
    let mut file = File::open(path)?;
    let mut buf = [0u8; BUF_SIZE];
    if file.metadata()?.len() != len {
        return Err(Box::from("File length has changed sicne the walk"));
    }
    let mut hasher = blake3::Hasher::new();
    let mut remain = len as usize;
    while remain > 0 {
        let n = remain.min(BUF_SIZE);
        file.read_exact(&mut buf[0..n])?;
        hasher.update(&buf[0..n]);
        remain -= n;
    }

    Ok(hasher.finalize())
}

fn group_by_hash(paths: Vec<PathBuf>, len: u64, fail: &mut Vec<String>) -> Vec<Vec<PathBuf>> {
    let mut by_hash: HashMap<blake3::Hash, Vec<PathBuf>> = HashMap::new();
    let hashed: Vec<_> = paths
        .into_par_iter()
        .map(|path| {
            let hash = hash_file(&path, len);
            (path, hash)
        })
        .collect();

    for (path, hash) in hashed {
        match hash {
            Ok(val) => by_hash.entry(val).or_default().push(path),
            Err(e) => fail.push(format!("Could not hash {path:?}, got an error: {e}")),
        }
    }
    by_hash.into_values().filter(|vec| vec.len() > 1).collect()
}

fn remove_by_hash(paths: Vec<PathBuf>, succ: &mut Vec<(String, PathBuf)>) {
    let first = paths.first().unwrap().clone();
    for element in paths.into_iter().skip(1) {
        succ.push((
            format!("file \n{:?} \n is equal to \n{:?}\n", element, first),
            element,
        ))
    }
}

fn group_by_prefix(paths: Vec<PathBuf>, len: u64, fail: &mut Vec<String>) -> Vec<Vec<PathBuf>> {
    let mut mapping: HashMap<blake3::Hash, Vec<PathBuf>> = HashMap::new();
    let n = (len as usize).min(4096);

    let prefix: Vec<(PathBuf, std::io::Result<blake3::Hash>)> = paths
        .into_par_iter()
        .map(|path| {
            let mut buffer = [0u8; 4096];
            let res = File::open(&path).and_then(|mut f| {
                if f.metadata()?.len() != len {
                    return Err(std::io::Error::other(
                        "File has changed size since the walk",
                    ));
                }
                f.read_exact(&mut buffer[0..n])?;
                Ok(blake3::hash(&buffer[0..n]))
            });
            (path, res)
        })
        .collect();

    for (path, res) in prefix {
        match res {
            Ok(hash) => mapping.entry(hash).or_default().push(path),
            Err(e) => fail.push(format!("Could not read {path:?}, got an error: {e}")),
        }
    }
    mapping
        .into_values()
        .filter(|entry| entry.len() > 1)
        .collect()
}

fn tails_equal(a: &Path, b: &Path, len: u64) -> std::io::Result<bool> {
    let mut file1 = File::open(a)?;
    let mut file2 = File::open(b)?;

    if file1.metadata()?.len() != len || file2.metadata()?.len() != len {
        return Ok(false);
    }

    file2.seek(SeekFrom::Start(4096))?;
    file1.seek(SeekFrom::Start(4096))?;
    compare_2_files(&mut file1, &mut file2, len as usize - 4096)
}

pub fn delete_file(path: &PathBuf) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

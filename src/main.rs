use clap::Parser;
use std::collections::{HashMap, hash_map};
use std::fs::{self};
use std::fs::{DirEntry, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(group(
    clap::ArgGroup::new("features").required(true)
))]
struct Options {
    /// This flag activates the cleaning function. Nothing gets deleted if real_run is not set
    #[arg(short, long, default_value_t = false, group = "features")]
    clean: bool,
    /// This is the only required flag.
    #[arg(short, long, required = true)]
    path: PathBuf,
    /// If this flag is set, then all of the duplicates would get deleted
    #[arg(short, long, default_value_t = false, requires = "clean")]
    real_run: bool,
    /// Makes clean remove all empty files as well
    #[arg(short = 'e', long, default_value_t = false, requires = "clean")]
    remove_empty_files: bool,
    /// This is a feature which is unrelated to clean. this sorts the files by size.
    #[arg(
        short,
        long,
        default_value_t = false,
        conflicts_with = "clean",
        group = "features"
    )]
    sort: bool,
    /// This can specify how many files to show in the sort.
    /// Gets the minimum of specified and the actual number of files
    #[arg(short, long, default_value_t = 20, requires = "sort")]
    num_sorting: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let op = Options::parse();
    let mut files = populate_paths(&op.path, op.remove_empty_files, op.clean)?;
    if op.clean {
        run_clean(files, &op)?;
    } else {
        run_sort(&mut files, op.num_sorting);
    }
    Ok(())
}
fn run_clean(
    files: Vec<(u64, PathBuf)>,
    option: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let files = group_into_similar(files);
    for item in files {
        scan_and_clean(item.1, item.0 as usize, option)?;
    }
    Ok(())
}
fn run_sort(files: &mut Vec<(u64, PathBuf)>, num_sorting: usize) -> &mut Vec<(u64, PathBuf)> {
    files.sort_by(|a, b| b.0.cmp(&a.0));
    let len = num_sorting.min(files.len());
    for i in 0..len {
        let curr = files.get(i).unwrap();
        println!("{:.2}MB : {:?}", (curr.0 as f64 / 1_000_000.0), curr.1)
    }
    files
}

/// Scans the FS from the root provided by the user. The Scan is being done via BFS
fn populate_paths(
    path: &Path,
    remove_empty: bool,
    is_clean: bool,
) -> Result<Vec<(u64, PathBuf)>, std::io::Error> {
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
    let mut dir_paths: Vec<PathBuf> = vec![path.to_path_buf()];
    let mut file_paths: Vec<(u64, PathBuf)> = Vec::new();
    while let Some(curr_dir) = dir_paths.pop() {
        let tester = fs::read_dir(&curr_dir);
        if let Err(e) = tester {
            eprintln!("Could not read {curr_dir:?}, moving to the next, Error was: {e:?}");
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
                eprintln!(
                    "Could not access the metadata of {:?}, got an error of {e}",
                    entry.path()
                );
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
                    eprintln!(
                        "Skipping bundle {path:?} (deleting it might break the corresponding app)"
                    );
                    continue;
                }
                dir_paths.push(entry.path());
            }
        }
    }
    Ok(file_paths)
}

fn group_into_similar(files: Vec<(u64, PathBuf)>) -> HashMap<u64, Vec<PathBuf>> {
    let mut mapping: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for item in files {
        if let hash_map::Entry::Vacant(e) = mapping.entry(item.0) {
            e.insert(vec![item.1]);
        } else {
            mapping.get_mut(&item.0).unwrap().push(item.1)
        }
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
    option: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    // A cache of first 4096 bytes of the file. This can help on small files. This can reduce that
    // total reads from O(n^2) to O(n)
    let mut index_to_first_hash: HashMap<usize, [u8; 4096]> = HashMap::new();
    let prefix = len.min(4096) as usize;
    read_first_4096_bytes(&files, &mut index_to_first_hash, prefix)?;
    if len == 0 && option.remove_empty_files {
        delete_empty(files, option.real_run)?;
        return Ok(());
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
                    println!("Could not open {:?}, got an error {e}", curr_file);
                    continue;
                }
                let mut file1 = file1.unwrap();
                let seek = file1.seek(SeekFrom::Start(4096));
                if let Err(e) = seek {
                    println!(
                        "Could not seek the first 4096 of {:?}, got an error {e}",
                        curr_file
                    );
                    continue;
                }
                let file2 = File::open(compare);
                if let Err(e) = file2 {
                    println!("Could not open {:?}, got an error {e}", curr_file);
                    continue;
                }
                let mut file2 = file2.unwrap();
                let seek = file1.seek(SeekFrom::Start(4096));
                if let Err(e) = seek {
                    println!(
                        "Could not seek the first 4096 of {:?}, got an error {e}",
                        compare
                    );
                    continue;
                }
                let comp2 = compare_2_files(&mut file1, &mut file2, (len - 4096) as usize);
                if let Err(e) = comp2 {
                    eprintln!(
                        "Could not compare {:?} and {:?}, got an error of {e}",
                        curr_file, compare
                    );
                    continue;
                }
                comp = comp2.unwrap();
            }

            if comp {
                if option.real_run {
                    println!("Removing file {:?} it is equal to {:?}", compare, curr_file);
                    if let Err(e) = fs::remove_file(compare) {
                        eprintln!("Could not delete {:?}, got an Error {e}", { compare });
                        continue;
                    }
                } else {
                    println!(
                        "This is a dry run, would remove file {:?} it is equal to {:?}",
                        compare, curr_file
                    )
                }
                gone_over[j] = true;
            }
        }
    }
    Ok(())
}

fn read_first_4096_bytes(
    files: &Vec<PathBuf>,
    mapping: &mut HashMap<usize, [u8; 4096]>,
    len: usize,
) -> Result<(), io::Error> {
    for i in 0..files.len() {
        let mut buf = [0u8; 4096];
        let curr = File::open(&files[i]);
        if let Err(e) = curr.as_ref() {
            println!("Could not open {:?}, got an error {e}", &files[i]);
            continue;
        }
        let mut curr = curr.unwrap();
        let read = curr.read_exact(&mut buf[0..len]);
        if let Err(e) = read {
            println!("Could not read {:?}, got an error {e}", &files[i]);
            continue;
        }
        mapping.insert(i, buf);
    }
    Ok(())
}
fn delete_empty(empty_files: Vec<PathBuf>, real_run: bool) -> Result<(), io::Error> {
    for path in &empty_files {
        let file = File::open(path);
        if let Err(e) = file.as_ref() {
            println!("Could not open {:?}, got an erorr {e}", path);
        }
        let file = file.unwrap();

        let metadata = file.metadata();
        if let Err(e) = metadata {
            eprintln!("Could not get the metadata of {:?}, got an error {e}", path);
            continue;
        }
        let metadata = metadata.unwrap();
        if metadata.len() == 0 {
            if real_run {
                if let Err(e) = fs::remove_file(path) {
                    eprintln!("Could not remove {:?}, got an error {e}", path);
                    continue;
                } else {
                    println!("Removed empty file: {:?}", path);
                }
            } else {
                println!("This is a dry run, would remove file {:?}", path)
            }
        }
    }
    Ok(())
}

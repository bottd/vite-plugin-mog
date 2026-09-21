//! Stamps the native binary with a digest of what it was built from.
//!
//! The JavaScript build computes the same digest over the same files, and the
//! `vite-plugin-mog/parser` entry refuses to run when the two disagree. A
//! version string cannot do this job: an unreleased local checkout and the
//! published release whose binary it falls back to carry the same one.
//!
//! FNV-1a rather than anything from `std`: `DefaultHasher` is explicitly not
//! stable across Rust versions or platforms, and this digest has to match one
//! computed in Node, on six different build hosts.

use std::path::{Path, PathBuf};

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

fn main() {
    let mut files = sources(Path::new("src/parser"));
    files.push(PathBuf::from("Cargo.lock"));

    // Sorted by the same spelling that is hashed, so the order cannot drift
    // from the one the JavaScript side produces.
    let mut files: Vec<String> = files
        .iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect();
    files.sort();

    let mut hash = OFFSET;
    for path in &files {
        println!("cargo:rerun-if-changed={path}");
        // The path goes in as well as the contents, so moving or renaming a
        // file changes the digest even when no byte of it does.
        digest(&mut hash, path.as_bytes());
        digest(&mut hash, &std::fs::read(path).unwrap_or_default());
    }

    println!("cargo:rustc-env=MOG_BUILD_ID={hash:016x}");
}

fn digest(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(PRIME);
    }
}

fn sources(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .flat_map(|path| match path.is_dir() {
            true => sources(&path),
            false => Vec::from_iter(
                path.extension()
                    .is_some_and(|extension| extension == "rs")
                    .then_some(path),
            ),
        })
        .collect()
}

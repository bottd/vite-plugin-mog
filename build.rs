//! Stamps the native binary with a digest of what it was built from.
//!
//! The JavaScript build computes the same digest over the same files, and the
//! `vite-plugin-mog/parser` entry refuses to run when the two disagree. A
//! version string cannot do this job: an unreleased local checkout and the
//! published release whose binary it falls back to carry the same one.
//!
//! `Cargo.lock` stands in for the dependency tree, which covers `mog-parser`
//! because it is pinned by git rev. Under the development `[patch]` in
//! `Cargo.toml` it is a path dependency instead, which the lock records without
//! a checksum — so editing the local parser changes neither digest. That is a
//! development-only gap; a released build has no patch.
//!
//! FNV-1a rather than anything from `std`: `DefaultHasher` is explicitly not
//! stable across Rust versions or platforms, and this digest has to match one
//! computed in Node, on six different build hosts.

use std::path::{Path, PathBuf};

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

fn main() {
    let mut files = sources(Path::new("src/parser")).expect("read parser sources");
    files.push(PathBuf::from("Cargo.lock"));
    println!("cargo:rerun-if-changed=src/parser");

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
        digest(
            &mut hash,
            &std::fs::read(path).expect("read build-id input"),
        );
    }

    println!("cargo:rustc-env=MOG_BUILD_ID={hash:016x}");
}

fn digest(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(PRIME);
    }
}

fn sources(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(sources(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(files)
}

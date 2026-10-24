//! A directory of its own, for a store to be written into.
//!
//! It is under the crate rather than under the system's temporary directory, because that is the
//! project's rule for scratch files: they start with `__` and stay inside the tree, so what a tool
//! leaves behind is one directory somebody can see and delete rather than something in `%TEMP%` that
//! nobody ever will. `__scratch/` is ignored by git.
//!
//! Each call is a directory no other call uses: the process id and a counter make the name, so a run
//! that measures several stores in a row — or a test binary running its cases at once — never shares
//! one. A name left over from an earlier run of the same process is emptied first.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Where the scratch directories are kept: under the crate's own directory.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/__scratch");

/// A new, empty directory named after `label`.
///
/// # Panics
///
/// Panics if the directory cannot be made. A measurement with nowhere to write is not a measurement
/// with a degraded answer: it is one that cannot be taken, and every caller treats it that way.
pub fn at(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    let path = PathBuf::from(ROOT).join(format!(
        "{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));

    let _ = fs::remove_dir_all(&path);

    if let Err(error) = fs::create_dir_all(&path) {
        // PANIC: the `# Panics` section above is the contract — a measurement with no writable
        // directory is not one that can be taken — so this stops rather than report from nowhere.
        panic!("making {}: {error}", path.display());
    }

    path
}

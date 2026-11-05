//! Writing a tree structure out as work on disk.
//!
//! A structure is what a version holds; the work is what a directory shows. Landing a tree, restoring
//! one and settling a merge all put a structure back as work, so they put it back the same way: a
//! directory for every directory, the stored content for every file, and the target for every link.

use std::path::Path;

use librorolala::storage::{RorolalaStorage, StorageBackend as _};
use librorolala::tree::{EntryKind, TreeStructure};

/// Writes `structure` under `root`, making the way to each entry.
///
/// It is what a version looks like as work, and it does not touch what the structure does not name:
/// what the work holds and the version does not is the caller's to decide about.
///
/// # Errors
///
/// Returns the failure as a string, since every caller reports it in its own words.
pub fn write(
    root: &Path,
    structure: &TreeStructure,
    store: &RorolalaStorage,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;

    for entry in structure.entries() {
        let at = root.join(entry.path());

        match entry.kind() {
            EntryKind::Directory => {
                std::fs::create_dir_all(&at).map_err(|error| error.to_string())?;
            }
            EntryKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };
                if let Some(parent) = at.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                runtime
                    .block_on(store.extract_file(storage, &at))
                    .map_err(|error| error.to_string())?;
            }
            EntryKind::Symlink => {
                let Some(target) = entry.target() else {
                    continue;
                };
                if let Some(parent) = at.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                if at.exists() {
                    let _ = std::fs::remove_file(&at);
                }
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, &at).map_err(|error| error.to_string())?;
                #[cfg(windows)]
                std::os::windows::fs::symlink_file(target, &at)
                    .map_err(|error| error.to_string())?;
            }
        }
    }

    Ok(())
}

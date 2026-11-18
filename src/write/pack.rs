//! Laying a store and a VCS index out afresh.

use std::path::Path;

use rorolala_storage::{Lockable as _, RorolalaStorage};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use serde::Serialize;

use crate::write::{WriteError, progress::WriteProgress};

/// What a pack changed.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Packed {
    /// Whether anything was rewritten.
    pub changed: bool,
}

/// Lays a store out afresh under its lock.
///
/// # Errors
///
/// Returns [`WriteError::Locked`] when the store is held elsewhere, and [`WriteError::Storage`]
/// when the pack fails.
pub fn pack_storage(store: &RorolalaStorage) -> Result<bool, WriteError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let guard = runtime
        .block_on(store.lock())
        .map_err(|error| WriteError::Locked {
            path: format!("store: {error}"),
        })?;
    let progress = Progress::to(WriteProgress::new());

    runtime
        .block_on(guard.repack(&progress))
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })
}

/// Lays a VCS index out afresh under its lock.
///
/// # Errors
///
/// Returns [`WriteError::Locked`] when the index is held elsewhere, and [`WriteError::Index`] when
/// the pack fails.
pub fn pack_index(index: &VCSIndex) -> Result<bool, WriteError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let guard = runtime
        .block_on(index.lock())
        .map_err(|error| WriteError::Locked {
            path: format!("index: {error}"),
        })?;
    let progress = Progress::to(WriteProgress::new());

    runtime
        .block_on(guard.repack(&progress))
        .map_err(|error| WriteError::Index {
            reason: error.to_string(),
        })
}

/// Lays a Workspace's store and index out afresh.
///
/// # Errors
///
/// Returns [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the parts are missing, and the
/// failures [`pack_storage`] and [`pack_index`] name.
pub fn pack_workspace(
    workspace: &Workspace,
    storage: bool,
    index: bool,
) -> Result<Packed, WriteError> {
    let mut changed = false;

    if storage {
        let store = workspace
            .get_current_rola_storage()
            .ok_or(WriteError::NoStore)?;
        changed |= pack_storage(&store)?;
    }
    if index {
        let index = VCSIndex::locate(workspace.get_root()).ok_or(WriteError::NoIndex)?;
        changed |= pack_index(&index)?;
    }

    Ok(Packed { changed })
}

/// Lays the store and index of the Workspace a directory is in out afresh.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"changed":false}`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`pack_workspace`] names.
#[lazyffi(export = rola_write_pack)]
pub fn pack_at(directory: &Path, storage: bool, index: bool) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let packed = pack_workspace(&workspace, storage, index)?;

    serde_json::to_string(&packed).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::pack_workspace;
    use crate::write::track::track_file;

    #[test]
    fn a_workspace_packs_its_store_and_index() {
        let dir = std::env::temp_dir().join(format!("rola-write-pack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        track_file(&workspace, &dir, "a.txt", "alice", "first", "", None).unwrap();

        let _packed = pack_workspace(&workspace, true, true).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}

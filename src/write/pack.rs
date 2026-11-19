//! Laying a store and a VCS index out afresh.

use std::path::Path;

use rorolala_storage::{Lockable as _, RorolalaStorage};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rorolala_vault::Vault;
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
    let progress = Progress::to(WriteProgress::new());
    pack_storage_with(store, &progress)
}

/// Lays a store out afresh under its lock, saying progress to `progress`.
///
/// # Errors
///
/// Returns [`WriteError::Locked`] when the store is held elsewhere, and [`WriteError::Storage`]
/// when the pack fails.
pub fn pack_storage_with(store: &RorolalaStorage, progress: &Progress) -> Result<bool, WriteError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let guard = runtime
        .block_on(store.lock())
        .map_err(|error| WriteError::Locked {
            path: format!("store: {error}"),
        })?;

    runtime
        .block_on(guard.repack(progress))
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
    let progress = Progress::to(WriteProgress::new());
    pack_index_with(index, &progress)
}

/// Lays a VCS index out afresh under its lock, saying progress to `progress`.
///
/// # Errors
///
/// Returns [`WriteError::Locked`] when the index is held elsewhere, and [`WriteError::Index`] when
/// the pack fails.
pub fn pack_index_with(index: &VCSIndex, progress: &Progress) -> Result<bool, WriteError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let guard = runtime
        .block_on(index.lock())
        .map_err(|error| WriteError::Locked {
            path: format!("index: {error}"),
        })?;

    runtime
        .block_on(guard.repack(progress))
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
        let set = workspace.storage_set()?;
        changed |= pack_storage(set.write_target())?;
    }
    if index {
        let index = VCSIndex::locate(workspace.get_root()).ok_or(WriteError::NoIndex)?;
        changed |= pack_index(&index)?;
    }

    Ok(Packed { changed })
}

/// Lays a Vault's store and index out afresh.
///
/// # Errors
///
/// Returns [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the parts are missing, and the
/// failures [`pack_storage`] and [`pack_index`] name.
pub fn pack_vault(vault: &Vault, storage: bool, index: bool) -> Result<Packed, WriteError> {
    let mut changed = false;

    if storage {
        let set = vault.storage_set()?;
        changed |= pack_storage(set.write_target())?;
    }
    if index {
        let index = VCSIndex::locate(vault.get_root()).ok_or(WriteError::NoIndex)?;
        changed |= pack_index(&index)?;
    }

    Ok(Packed { changed })
}

/// Lays the store a directory is in out afresh.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"changed":false}`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoStore`] when no store is there, and the failures [`pack_storage`] names.
#[lazyffi(export = rola_write_pack_storage)]
pub fn pack_storage_at(directory: &Path) -> Result<String, WriteError> {
    let set = crate::storage_set::for_directory(directory)
        .map_err(WriteError::from)?
        .ok_or(WriteError::NoStore)?;
    let changed = pack_storage(set.write_target())?;

    serde_json::to_string(&Packed { changed }).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Lays the VCS index a directory is in out afresh.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"changed":false}`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoIndex`] when no index is there, and the failures [`pack_index`] names.
#[lazyffi(export = rola_write_pack_index)]
pub fn pack_index_at(directory: &Path) -> Result<String, WriteError> {
    let index = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let changed = pack_index(&index)?;

    serde_json::to_string(&Packed { changed }).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
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

/// Lays the store and index of the Vault a directory is in out afresh.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"changed":false}`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoVault`] when no Vault holds `directory`, and the failures
/// [`pack_vault`] names.
#[lazyffi(export = rola_write_pack_vault)]
pub fn pack_vault_at(directory: &Path, storage: bool, index: bool) -> Result<String, WriteError> {
    let vault = Vault::locate(directory).ok_or(WriteError::NoVault)?;
    let packed = pack_vault(&vault, storage, index)?;

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
    use crate::write::track::{TrackOptions, track_file};

    #[test]
    fn a_workspace_packs_its_store_and_index() {
        let dir = std::env::temp_dir().join(format!("rola-write-pack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        let _packed = pack_workspace(&workspace, true, true).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}

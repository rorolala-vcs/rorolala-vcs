//! What a write leaves behind for the next reading: the tree's fingerprint cache.

use std::path::Path;

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;
use rorolala_workspace_analyze::{Cache, cache_path, entry_of, walk};

use crate::write::WriteError;

/// What a cache rebuild found.
#[derive(Debug, Clone, Copy)]
pub struct CacheReport {
    /// How many paths were written down.
    pub paths: usize,
    /// How many bytes the paths hold.
    pub bytes: u64,
}

/// Reads the whole tree and writes down what each path holds.
///
/// It is the one operation that reads a tree to write the cache; the reads that only want to know
/// never write it. A write flow calls it after it has finished, or uses
/// [`tree_diff_remembering`](rorolala_workspace_analyze::tree_diff_remembering) for the reading it
/// already made.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds the directory, and
/// [`WriteError::Layout`] when the Layout or the cache cannot be read or written.
pub fn rebuild_tree_cache(workspace: &Workspace) -> Result<CacheReport, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let root = workspace.get_root();
    let (found, _failed) = walk(root);
    let mut cache = Cache::empty();
    let mut bytes = 0_u64;
    let mut paths = 0_usize;

    for file in found {
        if file.kind != rorolala_layout::EntryKind::File {
            continue;
        }

        let Ok(entry) = entry_of(&file.disk) else {
            continue;
        };
        bytes = bytes.saturating_add(entry.size());
        paths += 1;
        cache.insert(file.path, entry);
    }

    cache
        .write(&cache_path(root, &layout))
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })?;

    Ok(CacheReport { paths, bytes })
}

/// Makes the Layout track a Vault.
///
/// # Errors
///
/// Returns [`WriteError::Layout`] when the Layout cannot be read or the track file cannot be written.
pub fn set_track(workspace: &Workspace, track: &str) -> Result<(), WriteError> {
    workspace
        .set_layout_track(track)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })
}

/// Makes the Layout track no Vault.
///
/// # Errors
///
/// Returns [`WriteError::Layout`] when the track file cannot be removed.
pub fn clear_track(workspace: &Workspace) -> Result<(), WriteError> {
    workspace
        .un_track_layout()
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })
}

/// Makes the Layout of the Workspace holding a directory track a Vault.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and
/// [`WriteError::Layout`] when the track file cannot be written.
#[lazyffi(export = rola_write_workspace_set_track)]
pub fn set_track_at(directory: &Path, track: &str) -> Result<(), WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;

    set_track(&workspace, track)
}

/// Makes the Layout of the Workspace holding a directory track no Vault.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and
/// [`WriteError::Layout`] when the track file cannot be removed.
#[lazyffi(export = rola_write_workspace_clear_track)]
pub fn clear_track_at(directory: &Path) -> Result<(), WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;

    clear_track(&workspace)
}

/// Rebuilds the tree cache of the Workspace holding a directory.
///
/// # FFI
///
/// A `RolaWriteWorkspaceCache *` the caller releases with `free_rola_write_workspace_cache`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and
/// [`WriteError::Layout`] when the cache cannot be written.
#[lazyffi(export = rola_write_workspace_rebuild_cache)]
pub fn rebuild_tree_cache_at(directory: &Path) -> Result<WriteWorkspaceCache, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let report = rebuild_tree_cache(&workspace)?;

    Ok(WriteWorkspaceCache {
        paths: u64::try_from(report.paths).unwrap_or(u64::MAX),
        bytes: report.bytes,
    })
}

/// What a cache rebuild found, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaWriteWorkspaceCache *` the caller releases with `free_rola_write_workspace_cache`.
/// `_paths` is how many paths were written down, and `_bytes` how many bytes they hold.
#[lazyffi(export = RolaWriteWorkspaceCache)]
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteWorkspaceCache {
    /// How many paths were written down.
    paths: u64,
    /// How many bytes the paths hold.
    bytes: u64,
}

#[lazyffi(export = rola_)]
impl WriteWorkspaceCache {
    /// How many paths were written down.
    #[must_use]
    pub const fn paths(&self) -> u64 {
        self.paths
    }

    /// How many bytes the paths hold.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        WriteWorkspaceCache, clear_track, clear_track_at, rebuild_tree_cache,
        rebuild_tree_cache_at, set_track, set_track_at,
    };
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    #[test]
    fn a_rebuild_writes_every_file_into_the_cache() {
        let dir = std::env::temp_dir().join(format!("rola-write-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("one.txt"), b"one").unwrap();
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("nested/two.txt"), b"two").unwrap();

        let report = rebuild_tree_cache(&workspace).unwrap();

        assert_eq!(report.paths, 2);
        assert_eq!(report.bytes, 6);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cache_answer_hands_over_what_it_found() {
        let answer = WriteWorkspaceCache { paths: 2, bytes: 6 };

        assert_eq!(answer.paths(), 2);
        assert_eq!(answer.bytes(), 6);
        assert_eq!(WriteWorkspaceCache::default().paths(), 0);
    }

    #[test]
    fn a_layout_tracks_a_vault_and_lets_it_go() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-workspace-track-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        set_track(&workspace, "alice").unwrap();
        clear_track(&workspace).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_directory_exports_track_clear_and_rebuild() {
        let dir = std::env::temp_dir().join(format!("rola-cov-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        fs::write(dir.join("one.txt"), b"one").unwrap();

        let answer = rebuild_tree_cache_at(&dir).unwrap();
        assert_ne!(answer.paths(), 0);
        assert_ne!(answer.bytes(), 0);

        set_track_at(&dir, "alice").unwrap();
        clear_track_at(&dir).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}

//! Recording the content of one file as a version, and naming it in the Layout.

use std::path::Path;

use rorolala_layout::{EntryKind, LayoutPath};
use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use serde::Serialize;
use uuid::Uuid;

use crate::write::{
    WriteError,
    index::write_file_version,
    layout::{create_path, write_entry},
    storage::put_file,
    workspace::rebuild_tree_cache,
};

/// What tracking one file wrote.
#[derive(Debug, Clone, Serialize)]
pub struct TrackedFile {
    /// The Layout entry the path names.
    pub id: Uuid,
    /// The content's key.
    pub storage: Key,
    /// The variant the version points at.
    pub variant: Key,
    /// The version the Layout records.
    pub version: Key,
}

/// Records the content of `path` as a version and binds it in the Layout.
///
/// `path` is relative to `cwd`, or absolute; it must be inside the Workspace. `creator` and
/// `message` become the version's creator and message, and `policy` names a chunking policy or is
/// empty for the store's own choice. A `None` owner keeps the owner the entry already has.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Workspace or names a directory,
/// [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the parts are missing, and
/// [`WriteError::Storage`], [`WriteError::Index`] or [`WriteError::Layout`] when a write fails.
pub fn track_file(
    workspace: &Workspace,
    cwd: &Path,
    path: &str,
    creator: &str,
    message: &str,
    policy: &str,
    owner: Option<&str>,
) -> Result<TrackedFile, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = workspace
        .get_current_rola_storage()
        .ok_or(WriteError::NoStore)?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(workspace.get_root());
    let resolved = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = normalize(&resolved);
    let relative = resolved
        .strip_prefix(&root)
        .map_err(|_| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path is outside the Workspace".to_owned(),
        })?;
    if !resolved.is_file() {
        return Err(WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path is not a file".to_owned(),
        });
    }

    let layout_path =
        LayoutPath::from_relative(relative).map_err(|error| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
    let existing = layout.id_of(&layout_path);
    let existing_owner = existing
        .and_then(|id| layout.entry(id))
        .and_then(|entry| entry.owner().map(str::to_owned));
    if let Some(id) = existing
        && layout
            .entry(id)
            .is_some_and(|entry| entry.kind() != EntryKind::File)
    {
        return Err(WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path is not a file".to_owned(),
        });
    }

    let base = match existing.and_then(|id| layout.entry(id)) {
        Some(entry) if entry.version() != [0; 32] => {
            let version = index
                .read_version(&Key::new(entry.version()).to_string())
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?;
            *version.variant()
        }
        _ => [0; 32],
    };

    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let stored = runtime.block_on(put_file(
        &store,
        &resolved,
        (!policy.is_empty()).then_some(policy),
    ))?;

    let written = write_file_version(&index, &stored.key, &base, None, creator, message)?;
    let id = existing.unwrap_or_else(Uuid::new_v4);
    write_entry(
        &layout,
        id,
        *written.version.digest(),
        owner.map(str::to_owned).or(existing_owner),
        String::new(),
        existing.is_some(),
    )?;
    if existing.is_none() {
        create_path(&layout, &layout_path, id)?;
    }
    rebuild_tree_cache(workspace)?;

    Ok(TrackedFile {
        id,
        storage: stored.key,
        variant: written.variant,
        version: written.version,
    })
}

/// Records one file's content as a version in the Workspace a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","storage":"...","variant":"...","version":"..."}`, or a `RolaWriteError *` the
/// caller releases with `free_rola_write_error`. `policy` is empty for the store's own choice, and
/// `owner` is empty when the entry has no owner.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`track_file`] names.
#[lazyffi(export = rola_write_track_file)]
pub fn track_file_at(
    directory: &Path,
    path: &str,
    creator: &str,
    message: &str,
    policy: &str,
    owner: &str,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let tracked = track_file(
        &workspace,
        directory,
        path,
        creator,
        message,
        policy,
        (!owner.is_empty()).then_some(owner),
    )?;

    serde_json::to_string(&tracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::track_file;

    #[test]
    fn a_file_is_recorded_and_named() {
        let dir = std::env::temp_dir().join(format!("rola-write-track-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let tracked = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            "",
            Some("alice"),
        )
        .unwrap();

        let layout = workspace.layout().unwrap();
        let path = rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        assert_eq!(layout.id_of(&path), Some(tracked.id));
        assert_ne!(tracked.version.to_string(), "");

        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_file(&workspace, &dir, "a.txt", "alice", "second", "", None).unwrap();
        assert_eq!(
            layout
                .entry(tracked.id)
                .and_then(|entry| entry.owner().map(str::to_owned)),
            Some("alice".to_owned())
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

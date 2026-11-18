//! Recording the content of one file as a version, and naming it in the Layout.

use std::path::{Path, PathBuf};

use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_storage::{Key, RorolalaStorage};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vault::Vault;
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

/// How a file is to be tracked.
#[derive(Debug, Clone, Default)]
pub struct TrackOptions {
    /// The chunking policy to write with, or empty for the store's own choice.
    pub policy: String,
    /// The account that holds the entry, or nothing to keep the existing owner.
    pub owner: Option<String>,
    /// What the entry says about itself.
    pub description: String,
}

/// The parts a tracked file is written through.
struct Place<'a> {
    layout: &'a Layout,
    store: &'a RorolalaStorage,
    index: &'a VCSIndex,
}

/// Records one file's content as a version and binds it in a Layout.
fn record(
    place: &Place<'_>,
    resolved: &Path,
    relative: &str,
    path: &str,
    creator: &str,
    message: &str,
    options: TrackOptions,
) -> Result<TrackedFile, WriteError> {
    let layout = place.layout;
    let layout_path = LayoutPath::from_relative(Path::new(relative)).map_err(|error| {
        WriteError::InvalidPath {
            path: path.to_owned(),
            reason: error.to_string(),
        }
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

    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let (base, base_num) = match existing.and_then(|id| layout.entry(id)) {
        Some(entry) if entry.version() != [0; 32] => {
            let version = place
                .index
                .read_version(&Key::new(entry.version()).to_string())
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?;
            let number = runtime
                .block_on(place.index.version_num(&version))
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?;

            (entry.version(), number)
        }
        _ => ([0; 32], rorolala_vcs::ROOT_VERSION),
    };
    let stored = runtime.block_on(put_file(
        place.store,
        resolved,
        (!options.policy.is_empty()).then_some(options.policy.as_str()),
    ))?;

    let written = write_file_version(
        place.index,
        &stored.key,
        &base,
        base_num,
        None,
        creator,
        message,
    )?;
    let id = existing.unwrap_or_else(Uuid::new_v4);
    write_entry(
        layout,
        id,
        *written.version.digest(),
        options.owner.or(existing_owner),
        options.description,
        existing.is_some(),
    )?;
    if existing.is_none() {
        create_path(layout, &layout_path, id)?;
    }

    Ok(TrackedFile {
        id,
        storage: stored.key,
        variant: written.variant,
        version: written.version,
    })
}

/// Records the content of `path` as a version and binds it in the Layout.
///
/// `path` is relative to `cwd`, or absolute; it must be inside the Workspace and outside a tree.
/// `creator` and `message` become the version's creator and message. `options` names the chunking
/// policy, the owner and the entry's description; a `None` owner keeps the owner the entry already
/// has.
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
    options: TrackOptions,
) -> Result<TrackedFile, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = workspace
        .get_current_rola_storage()
        .ok_or(WriteError::NoStore)?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(workspace.get_root());
    let (resolved, relative) = locate_file(&root, cwd, path)?;
    let place = Place {
        layout: &layout,
        store: &store,
        index: &index,
    };
    let tracked = record(
        &place, &resolved, &relative, path, creator, message, options,
    )?;
    rebuild_tree_cache(workspace)?;

    Ok(tracked)
}

/// Records the content of `path` in a Vault and binds it in its Layout.
///
/// `path` is relative to `cwd`, or absolute; it must be inside the Vault and outside a tree.
/// `creator` and `message` become the version's creator and message. `options` names the chunking
/// policy, the owner and the entry's description; a `None` owner keeps the owner the entry already
/// has.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Vault or names a directory,
/// [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the parts are missing, and
/// [`WriteError::Storage`], [`WriteError::Index`] or [`WriteError::Layout`] when a write fails.
pub fn track_vault_file(
    vault: &Vault,
    cwd: &Path,
    path: &str,
    creator: &str,
    message: &str,
    options: TrackOptions,
) -> Result<TrackedFile, WriteError> {
    let layout = vault.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = vault
        .get_current_rola_storage()
        .ok_or(WriteError::NoStore)?;
    let index = VCSIndex::locate(vault.get_root()).ok_or(WriteError::NoIndex)?;
    let root = normalize(vault.get_root());
    let (resolved, relative) = locate_file(&root, cwd, path)?;
    let place = Place {
        layout: &layout,
        store: &store,
        index: &index,
    };

    record(
        &place, &resolved, &relative, path, creator, message, options,
    )
}

/// The file `path` names, and its path inside the scope./// The file `path` names, and its path inside the scope.
fn locate_file(root: &Path, cwd: &Path, path: &str) -> Result<(PathBuf, String), WriteError> {
    let resolved = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = normalize(&resolved);
    let relative = resolved
        .strip_prefix(root)
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
    let mut above = resolved.parent();
    while let Some(directory) = above {
        if !directory.starts_with(root) {
            break;
        }
        if directory.join(rorolala_tree::consts::MARK).is_dir() {
            return Err(WriteError::InvalidPath {
                path: path.to_owned(),
                reason: "the path is inside a tree".to_owned(),
            });
        }
        above = directory.parent();
    }

    let relative = relative
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join("/"))
        .ok_or_else(|| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path cannot be written".to_owned(),
        })?;

    Ok((resolved, relative))
}

/// Records one file's content as a version in the Workspace a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","storage":"...","variant":"...","version":"..."}`, or a `RolaWriteError *` the
/// caller releases with `free_rola_write_error`. `policy` is empty for the store's own choice,
/// `owner` is empty when the entry has no owner, and `description` is the entry's description.
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
    description: &str,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let options = TrackOptions {
        policy: policy.to_owned(),
        owner: (!owner.is_empty()).then(|| owner.to_owned()),
        description: description.to_owned(),
    };
    let tracked = track_file(&workspace, directory, path, creator, message, options)?;

    serde_json::to_string(&tracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Records one file's content as a version in the Vault a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","storage":"...","variant":"...","version":"..."}`, or a `RolaWriteError *` the
/// caller releases with `free_rola_write_error`. `policy` is empty for the store's own choice,
/// `owner` is empty when the entry has no owner, and `description` is the entry's description.
///
/// # Errors
///
/// Returns [`WriteError::NoVault`] when no Vault holds `directory`, and the failures
/// [`track_vault_file`] names.
#[lazyffi(export = rola_write_track_vault_file)]
pub fn track_vault_file_at(
    directory: &Path,
    path: &str,
    creator: &str,
    message: &str,
    policy: &str,
    owner: &str,
    description: &str,
) -> Result<String, WriteError> {
    let vault = Vault::locate(directory).ok_or(WriteError::NoVault)?;
    let options = TrackOptions {
        policy: policy.to_owned(),
        owner: (!owner.is_empty()).then(|| owner.to_owned()),
        description: description.to_owned(),
    };
    let tracked = track_vault_file(&vault, directory, path, creator, message, options)?;

    serde_json::to_string(&tracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;

    use super::{TrackOptions, track_file, track_vault_file};

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
            TrackOptions {
                owner: Some("alice".to_owned()),
                description: "note".to_owned(),
                ..TrackOptions::default()
            },
        )
        .unwrap();

        let layout = workspace.layout().unwrap();
        let path = rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        assert_eq!(layout.id_of(&path), Some(tracked.id));
        assert_ne!(tracked.version.to_string(), "");

        fs::write(dir.join("a.txt"), b"two").unwrap();
        let again = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();
        assert_eq!(
            layout
                .entry(tracked.id)
                .and_then(|entry| entry.owner().map(str::to_owned)),
            Some("alice".to_owned())
        );
        assert_eq!(
            layout
                .entry(tracked.id)
                .map(|entry| entry.description().to_owned()),
            Some("note".to_owned())
        );

        let index = rorolala_vcs::VCSIndex::locate(&dir).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let numbered = |version: rorolala_storage::Key| {
            let version = index
                .read_version(&version.to_string())
                .expect("the version is in the index");
            runtime
                .block_on(index.version_num(&version))
                .expect("the chain reads")
        };
        assert_eq!(numbered(tracked.version), 0);
        assert_eq!(numbered(again.version), 1);

        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::write(dir.join("tree/inside.txt"), b"inside").unwrap();
        assert!(
            track_file(
                &workspace,
                &dir,
                "tree/inside.txt",
                "alice",
                "tree",
                TrackOptions::default()
            )
            .is_err()
        );
        assert!(
            track_file(
                &workspace,
                &dir,
                "../outside.txt",
                "alice",
                "outside",
                TrackOptions::default()
            )
            .is_err()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_file_is_recorded_and_named() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-track-vault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Vault::create(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let tracked = track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        let layout = vault.layout().unwrap();
        let path = rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        assert_eq!(layout.id_of(&path), Some(tracked.id));
        assert_ne!(tracked.version.to_string(), "");

        let _ = fs::remove_dir_all(&dir);
    }
}

//! Moving one path's record to a version, and its work with it.

use std::path::Path;
use std::str::FromStr;

use rorolala_layout::{LayoutPath, MutableData};
use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use serde::Serialize;
use uuid::Uuid;

use crate::write::{
    WriteError,
    checkout::restore,
    layout::{set_version, switch_entry},
    tree::relative_to,
};

/// What retracking one path wrote.
#[derive(Debug, Clone, Serialize)]
pub struct Retracked {
    /// The Layout entry the path names.
    pub id: Uuid,
    /// The version the Layout now records.
    pub version: Key,
}

/// Moves the record of `path` to `target`, or one version back when it is not named.
///
/// `fork` gives the path a fresh `Uuid` holding what it records now, and cannot be given with a
/// target. `only_index` leaves the work on disk alone; otherwise the content of the target version
/// is written back before the Layout names it.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Workspace or names no entry,
/// [`WriteError::Precondition`] when there is no version to move to, [`WriteError::Index`] when the
/// history cannot be read, and the failures [`restore`] names.
pub fn retrack_file(
    workspace: &Workspace,
    cwd: &Path,
    path: &str,
    target: Option<&str>,
    fork: bool,
    only_index: bool,
) -> Result<Retracked, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(workspace.get_root());
    let resolved = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = normalize(&resolved);
    let relative = relative_to(&root, &resolved).ok_or_else(|| WriteError::InvalidPath {
        path: path.to_owned(),
        reason: "the path is outside the Workspace".to_owned(),
    })?;
    let layout_path = LayoutPath::new(&relative).map_err(|error| WriteError::InvalidPath {
        path: path.to_owned(),
        reason: error.to_string(),
    })?;
    let id = layout
        .id_of(&layout_path)
        .ok_or_else(|| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the Layout names no such path".to_owned(),
        })?;
    let entry = layout.entry(id).ok_or_else(|| WriteError::NotFound {
        what: format!("Layout entry {id}"),
    })?;
    let current = Key::new(entry.version());
    if fork {
        if target.is_some() {
            return Err(WriteError::Precondition {
                what: path.to_owned(),
                reason: "a fork names no target version".to_owned(),
            });
        }
        let data = MutableData::new(
            entry.owner().map(str::to_owned),
            entry.version(),
            entry.description().to_owned(),
        )
        .with_preference(entry.preference().map(str::to_owned));
        let forked = Uuid::new_v4();
        switch_entry(&layout, &layout_path, forked, data)?;

        return Ok(Retracked {
            id: forked,
            version: current,
        });
    }
    let named = if let Some(text) = target {
        Key::from_str(text).map_err(|error| WriteError::Precondition {
            what: format!("version `{text}`"),
            reason: error.to_string(),
        })?
    } else {
        let version =
            index
                .read_version(&current.to_string())
                .map_err(|error| WriteError::Index {
                    reason: format!("reading version: {error:?}"),
                })?;
        let variant = index
            .read_variant(&Key::new(*version.variant()).to_string())
            .map_err(|error| WriteError::Index {
                reason: format!("reading variant: {error:?}"),
            })?;
        let base = variant.base_version();
        if *base == [0; 32] {
            return Err(WriteError::Precondition {
                what: path.to_owned(),
                reason: "the version has no earlier one".to_owned(),
            });
        }

        Key::new(*base)
    };

    index
        .read_version(&named.to_string())
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;
    if only_index {
        set_version(&layout, id, *named.digest())?;
    } else {
        restore(workspace, cwd, path, &named.to_string())?;
    }

    Ok(Retracked { id, version: named })
}

/// Moves the record of a path under a Workspace to a version.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. An empty `target` moves one version back, `fork` gives the path a fresh
/// `Uuid`, and `only_index` leaves the work on disk alone.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`retrack_file`] names.
#[lazyffi(export = rola_write_retrack_file)]
pub fn retrack_file_at(
    directory: &Path,
    path: &str,
    target: &str,
    fork: bool,
    only_index: bool,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let target = (!target.is_empty()).then_some(target);
    let retracked = retrack_file(&workspace, directory, path, target, fork, only_index)?;

    serde_json::to_string(&retracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::retrack_file;
    use crate::write::track::track_file;

    #[test]
    fn a_path_moves_back_one_version() {
        let dir = std::env::temp_dir().join(format!("rola-write-retrack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_file(&workspace, &dir, "a.txt", "alice", "first", "", None).unwrap();
        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_file(&workspace, &dir, "a.txt", "alice", "second", "", None).unwrap();

        let back = retrack_file(&workspace, &dir, "a.txt", None, false, false).unwrap();

        assert_eq!(back.version, first.version);
        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");

        let forked = retrack_file(&workspace, &dir, "a.txt", None, true, true).unwrap();
        assert_ne!(forked.id, first.id);
        assert_eq!(
            workspace
                .layout()
                .unwrap()
                .id_of(&rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap()),
            Some(forked.id)
        );

        let absent = rorolala_storage::Key::new([9; 32]).to_string();
        assert!(retrack_file(&workspace, &dir, "a.txt", Some(&absent), false, true).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}

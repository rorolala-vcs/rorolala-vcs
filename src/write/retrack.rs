//! Moving one path's record to a version, and its work with it.

use std::path::Path;
use std::str::FromStr;

use rorolala_layout::{LayoutPath, MutableData};
use rorolala_storage::{Key, key_of_file};
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

/// What a retrack may do beside moving the version.
#[derive(Debug, Clone, Copy, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent choice a caller makes about one retrack"
)]
pub struct RetrackOptions {
    /// Give the path a fresh `Uuid` holding what it records now.
    pub fork: bool,
    /// Leave the work on disk alone.
    pub only_index: bool,
    /// Go past changes the record does not name.
    pub force: bool,
    /// Refuse a target off the path's chain.
    pub deny_jump: bool,
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
    options: RetrackOptions,
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
    let current_version =
        index
            .read_version(&current.to_string())
            .map_err(|error| WriteError::Index {
                reason: format!("{error:?}"),
            })?;
    let current_variant = index
        .read_variant(&Key::new(*current_version.variant()).to_string())
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;
    let current_storage = Key::new(*current_variant.storage_hash());
    if options.fork {
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
    let named = resolve_target(&current_variant, target, path)?;

    index
        .read_version(&named.to_string())
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;
    if options.deny_jump && named != current && !is_ancestor(&index, &named, &current)? {
        return Err(WriteError::Precondition {
            what: path.to_owned(),
            reason: "the target is not on the path's chain".to_owned(),
        });
    }
    if !options.only_index && !options.force {
        let on_disk = key_of_file(&resolved).map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })?;
        if on_disk.digest() != current_storage.digest() {
            return Err(WriteError::Precondition {
                what: path.to_owned(),
                reason: "dirty".to_owned(),
            });
        }
    }
    if options.only_index {
        set_version(&layout, id, *named.digest())?;
    } else {
        restore(workspace, cwd, path, &named.to_string())?;
    }

    Ok(Retracked { id, version: named })
}

/// Whether `ancestor` is `target` or is on the chain behind it.
fn is_ancestor(index: &VCSIndex, target: &Key, ancestor: &Key) -> Result<bool, WriteError> {
    let mut at = *target;
    let mut seen = 0_usize;

    loop {
        if at == *ancestor {
            return Ok(true);
        }

        let version = index
            .read_version(&at.to_string())
            .map_err(|error| WriteError::Index {
                reason: format!("{error:?}"),
            })?;
        let variant = index
            .read_variant(&Key::new(*version.variant()).to_string())
            .map_err(|error| WriteError::Index {
                reason: format!("{error:?}"),
            })?;
        let base = *variant.base_version();
        if base == [0; 32] || base == *at.digest() {
            return Ok(false);
        }

        at = Key::new(base);
        seen += 1;
        if seen > 10_000 {
            return Ok(false);
        }
    }
}

/// The version a run asked for, or the one behind the one the path records.
fn resolve_target(
    current_variant: &rorolala_vcs::Variant,
    target: Option<&str>,
    path: &str,
) -> Result<Key, WriteError> {
    if let Some(text) = target {
        return Key::from_str(text).map_err(|error| WriteError::Precondition {
            what: format!("version `{text}`"),
            reason: error.to_string(),
        });
    }

    let base = current_variant.base_version();
    if *base == [0; 32] {
        return Err(WriteError::Precondition {
            what: path.to_owned(),
            reason: "the version has no earlier one".to_owned(),
        });
    }

    Ok(Key::new(*base))
}

/// Moves the record of a path under a Workspace to a version.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. An empty `target` moves one version back, `fork` gives the path a fresh
/// `Uuid`, and `only_index` leaves the work on disk alone. `force` goes past changes the record does
/// not name, and `deny_jump` refuses a target off the path's chain.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`retrack_file`] names.
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "the C ABI spells each retrack flag as its own bool"
)]
#[lazyffi(export = rola_write_retrack_file)]
pub fn retrack_file_at(
    directory: &Path,
    path: &str,
    target: &str,
    fork: bool,
    only_index: bool,
    force: bool,
    deny_jump: bool,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let target = (!target.is_empty()).then_some(target);
    let options = RetrackOptions {
        fork,
        only_index,
        force,
        deny_jump,
    };
    let retracked = retrack_file(&workspace, directory, path, target, options)?;

    serde_json::to_string(&retracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::{RetrackOptions, retrack_file};
    use crate::write::track::{TrackOptions, track_file};

    #[test]
    fn only_index_leaves_the_work_alone() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-retrack-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();
        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();

        retrack_file(
            &workspace,
            &dir,
            "a.txt",
            Some(&first.version.to_string()),
            RetrackOptions {
                only_index: true,
                ..RetrackOptions::default()
            },
        )
        .unwrap();

        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"two");
        assert_eq!(
            workspace
                .layout()
                .unwrap()
                .entry(first.id)
                .map(|entry| entry.version()),
            Some(*first.version.digest())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_moves_back_one_version() {
        let dir = std::env::temp_dir().join(format!("rola-write-retrack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();
        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();

        let back =
            retrack_file(&workspace, &dir, "a.txt", None, RetrackOptions::default()).unwrap();

        assert_eq!(back.version, first.version);
        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");

        let forked = retrack_file(
            &workspace,
            &dir,
            "a.txt",
            None,
            RetrackOptions {
                fork: true,
                only_index: true,
                ..RetrackOptions::default()
            },
        )
        .unwrap();
        assert_ne!(forked.id, first.id);
        assert_eq!(
            workspace
                .layout()
                .unwrap()
                .id_of(&rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap()),
            Some(forked.id)
        );

        let absent = rorolala_storage::Key::new([9; 32]).to_string();
        assert!(
            retrack_file(
                &workspace,
                &dir,
                "a.txt",
                Some(&absent),
                RetrackOptions {
                    only_index: true,
                    ..RetrackOptions::default()
                }
            )
            .is_err()
        );

        fs::write(dir.join("b.txt"), b"other").unwrap();
        let other = track_file(
            &workspace,
            &dir,
            "b.txt",
            "alice",
            "other",
            TrackOptions::default(),
        )
        .unwrap();
        assert!(
            retrack_file(
                &workspace,
                &dir,
                "a.txt",
                Some(&other.version.to_string()),
                RetrackOptions {
                    deny_jump: true,
                    ..RetrackOptions::default()
                }
            )
            .is_err()
        );

        fs::write(dir.join("a.txt"), b"dirty").unwrap();
        assert!(
            retrack_file(
                &workspace,
                &dir,
                "a.txt",
                Some(&first.version.to_string()),
                RetrackOptions::default()
            )
            .is_err()
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

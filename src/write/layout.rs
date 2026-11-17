//! Writing one Layout, as a batch of changes.

use std::path::Path;
use std::str::FromStr;

use rorolala_layout::{EntryKind, Layout, LayoutPath, MutableData};
use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vault::Vault;
use rorolala_workspace::{Merging, Workspace};
use serde::Deserialize;
use uuid::Uuid;

use crate::write::WriteError;

/// One change to a Layout.
#[derive(Debug, Clone)]
pub enum LayoutOp {
    /// Makes an entry, and binds a path to it when one is named.
    CreateEntry {
        /// The Uuid the entry is known by.
        id: Uuid,
        /// The path to bind, when one is.
        path: Option<LayoutPath>,
        /// What the entry holds.
        data: MutableData,
    },
    /// Changes what an entry holds.
    UpdateEntry {
        /// The Uuid the entry is known by.
        id: Uuid,
        /// What the entry now holds.
        data: MutableData,
    },
    /// Removes an entry.
    RemoveEntry {
        /// The Uuid to remove.
        id: Uuid,
    },
    /// Binds a path to an entry.
    CreatePath {
        /// The path to bind.
        path: LayoutPath,
        /// The entry it names.
        id: Uuid,
    },
    /// Moves the entry at one path to another.
    MovePath {
        /// The path the entry is at.
        from: LayoutPath,
        /// The path it is to be at.
        to: LayoutPath,
    },
    /// Removes the path, leaving the entry.
    RemovePath {
        /// The path to remove.
        path: LayoutPath,
    },
    /// Changes who holds an entry, keeping what it holds.
    ChangeOwnership {
        /// The entry.
        id: Uuid,
        /// The account that now holds it, or nothing.
        owner: Option<String>,
    },
    /// Moves an entry to a new Uuid, keeping its path and what it holds.
    ChangeUuid {
        /// The Uuid it has.
        from: Uuid,
        /// The Uuid it is to have.
        to: Uuid,
    },
}

/// Applies a batch of changes, committing all of them or none.
///
/// # Errors
///
/// Returns [`WriteError::Layout`] when a change does not read or does not apply. The batch is not
/// committed, so a caller that sees a failure sees the Layout as it was.
pub fn apply(layout: &Layout, ops: &[LayoutOp]) -> Result<(), WriteError> {
    let mut batch = layout.batch();

    for op in ops {
        match op.clone() {
            LayoutOp::CreateEntry { id, path, data } => {
                batch
                    .create_entry(id, data)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
                if let Some(path) = path {
                    batch
                        .create_path(&path, id)
                        .map_err(|error| WriteError::Layout {
                            reason: error.to_string(),
                        })?;
                }
            }
            LayoutOp::UpdateEntry { id, data } => {
                batch
                    .update_entry(id, data)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
            }
            LayoutOp::RemoveEntry { id } => {
                batch.remove_entry(id).map_err(|error| WriteError::Layout {
                    reason: error.to_string(),
                })?;
            }
            LayoutOp::CreatePath { path, id } => {
                batch
                    .create_path(&path, id)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
            }
            LayoutOp::MovePath { from, to } => {
                batch
                    .move_path(&from, &to)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
            }
            LayoutOp::RemovePath { path } => {
                batch
                    .remove_path(&path)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
            }
            LayoutOp::ChangeOwnership { id, owner } => {
                let data = batch.entry(id).ok_or_else(|| WriteError::NotFound {
                    what: format!("Layout entry {id}"),
                })?;
                let renewed = data.renewed(owner, data.version());
                batch
                    .update_entry(id, renewed)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
            }
            LayoutOp::ChangeUuid { from, to } => {
                let data = batch.entry(from).ok_or_else(|| WriteError::NotFound {
                    what: format!("Layout entry {from}"),
                })?;
                let path = layout.path_of(from);
                batch
                    .create_entry(to, data)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
                if let Some(path) = path {
                    batch
                        .create_path(&path, to)
                        .map_err(|error| WriteError::Layout {
                            reason: error.to_string(),
                        })?;
                    batch
                        .remove_path(&path)
                        .map_err(|error| WriteError::Layout {
                            reason: error.to_string(),
                        })?;
                }
                batch
                    .remove_entry(from)
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?;
            }
        }
    }

    batch.commit().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })
}

/// Moves the entry at one path to another, and takes a waiting merge with it.
///
/// The Layout is moved first and the merge record follows, because the record names the entry by
/// the path it is at: a failure to follow leaves the Layout's move in place, and the next merge
/// attempt sees the path it now has.
///
/// # Errors
///
/// Returns [`WriteError::Layout`] when the move does not apply, and [`WriteError::NotFound`] when
/// the path names nothing.
pub fn move_path(
    layout: &Layout,
    workspace: Option<&Workspace>,
    from: &LayoutPath,
    to: &LayoutPath,
) -> Result<(), WriteError> {
    layout.move_path(from, to).map_err(|error| match error {
        rorolala_layout::LayoutError::NotFound => WriteError::NotFound {
            what: format!("Layout path `{}`", from.as_str()),
        },
        other => WriteError::Layout {
            reason: other.to_string(),
        },
    })?;

    let Some(workspace) = workspace else {
        return Ok(());
    };
    let Some(target) = layout.id_of(to) else {
        return Ok(());
    };
    let merging = Merging::read(workspace.get_root());
    merging
        .follow(workspace.get_root(), target, from, to)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })
}

/// Moves one path of a Layout a directory holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`,
/// [`WriteError::NoLayout`] when it has no Layout, [`WriteError::InvalidPath`] when a path is not
/// one a Layout accepts, and [`WriteError::Layout`] when the move does not apply.
#[lazyffi(export = rola_write_layout_move_path)]
pub fn move_path_at(
    directory: &Path,
    layout: &str,
    from: &str,
    to: &str,
) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let from = path_of(from)?;
    let to = path_of(to)?;
    let workspace = Workspace::locate(directory);

    move_path(&target, workspace.as_ref(), &from, &to)
}

/// Binds a path to an entry, or reports what stopped it.
///
/// # Errors
///
/// Returns [`WriteError::Layout`] when the path is already bound or the entry is not there.
pub fn create_path(layout: &Layout, path: &LayoutPath, id: Uuid) -> Result<(), WriteError> {
    layout
        .create_path(path, id)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })
}

/// Unbinds a path, taking a tree's one entry with it.
///
/// A tree is one entry rather than a directory of entries, so unbinding its path would leave a tree
/// the Layout holds and no path can reach. What the path is removed from is the tree's entry, then.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the path names nothing, and [`WriteError::Layout`] when the
/// removal does not apply.
pub fn remove_path(layout: &Layout, path: &LayoutPath) -> Result<(), WriteError> {
    let tree = layout
        .id_of(path)
        .and_then(|id| layout.entry(id).map(|data| (id, data)))
        .filter(|(_, data)| data.kind() == EntryKind::Tree);

    let removed = match tree {
        Some((id, _)) => layout.remove_entry(id),
        None => layout.remove_path(path),
    };

    removed.map_err(|error| match error {
        rorolala_layout::LayoutError::NotFound => WriteError::NotFound {
            what: format!("Layout path `{}`", path.as_str()),
        },
        other => WriteError::Layout {
            reason: other.to_string(),
        },
    })
}

/// Binds a path of a Layout a directory holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`,
/// [`WriteError::NoLayout`] when it has no Layout, [`WriteError::InvalidPath`] when a path or a
/// `Uuid` is not one the Layout accepts, and [`WriteError::Layout`] when the binding does not apply.
#[lazyffi(export = rola_write_layout_create_path)]
pub fn create_path_at(
    directory: &Path,
    layout: &str,
    path: &str,
    uuid: &str,
) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let path = path_of(path)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;

    create_path(&target, &path, id)
}

/// Removes a path of a Layout a directory holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`,
/// [`WriteError::NoLayout`] when it has no Layout, [`WriteError::InvalidPath`] when a path is not
/// one a Layout accepts, and [`WriteError::Layout`] when the removal does not apply.
#[lazyffi(export = rola_write_layout_remove_path)]
pub fn remove_path_at(directory: &Path, layout: &str, path: &str) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let path = path_of(path)?;

    remove_path(&target, &path)
}

/// One change, as a request document names it.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum RawOp {
    /// Makes an entry.
    CreateEntry {
        uuid: String,
        path: Option<String>,
        version: String,
        owner: Option<String>,
        description: Option<String>,
        kind: Option<String>,
    },
    /// Changes what an entry holds.
    UpdateEntry {
        uuid: String,
        version: String,
        owner: Option<String>,
        description: Option<String>,
    },
    /// Removes an entry.
    RemoveEntry { uuid: String },
    /// Binds a path.
    CreatePath { path: String, uuid: String },
    /// Moves a path.
    MovePath { from: String, to: String },
    /// Removes a path.
    RemovePath { path: String },
    /// Changes who holds an entry.
    ChangeOwnership { uuid: String, owner: Option<String> },
    /// Changes an entry's Uuid.
    ChangeUuid { from: String, to: String },
}

/// Applies a batch of changes named as JSON to the Layout a directory is in.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`] when the place has no Layout, [`WriteError::Json`] when
/// `ops` does not read, [`WriteError::InvalidPath`] or [`WriteError::NotFound`] when a name does not
/// read, and [`WriteError::Layout`] when a change does not apply.
#[lazyffi(export = rola_write_layout_apply)]
pub fn apply_json(directory: &Path, layout: &str, ops: &str) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let raw: Vec<RawOp> = serde_json::from_str(ops).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })?;
    let ops = raw.into_iter().map(op_of).collect::<Result<Vec<_>, _>>()?;

    apply(&target, &ops)
}

/// The Layout a name opens, making the one being worked in when it is not there.
fn open(directory: &Path, name: &str) -> Result<Layout, WriteError> {
    if let Some(vault) = Vault::locate(directory)
        && (name.is_empty() || name == "truth")
    {
        return vault.layout().map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        });
    }

    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    if name.is_empty() || name == "main" {
        return workspace.layout().map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        });
    }

    Err(WriteError::Precondition {
        what: format!("Layout `{name}`"),
        reason: "only `main` and `truth` are opened for a batch".to_owned(),
    })
}

/// Turns one request document into a typed change.
fn op_of(raw: RawOp) -> Result<LayoutOp, WriteError> {
    match raw {
        RawOp::CreateEntry {
            uuid,
            path,
            version,
            owner,
            description,
            kind,
        } => {
            let id = Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?;
            let path = path.map(|path| path_of(&path)).transpose()?;
            let mut data = mutable(&version, owner, description)?;
            if kind.as_deref() == Some("tree") {
                data = data.with_kind(EntryKind::Tree);
            }

            Ok(LayoutOp::CreateEntry { id, path, data })
        }
        RawOp::UpdateEntry {
            uuid,
            version,
            owner,
            description,
        } => {
            let id = Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?;
            let data = mutable(&version, owner, description)?;

            Ok(LayoutOp::UpdateEntry { id, data })
        }
        RawOp::RemoveEntry { uuid } => Ok(LayoutOp::RemoveEntry {
            id: Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?,
        }),
        RawOp::CreatePath { path, uuid } => Ok(LayoutOp::CreatePath {
            path: path_of(&path)?,
            id: Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?,
        }),
        RawOp::MovePath { from, to } => Ok(LayoutOp::MovePath {
            from: path_of(&from)?,
            to: path_of(&to)?,
        }),
        RawOp::RemovePath { path } => Ok(LayoutOp::RemovePath {
            path: path_of(&path)?,
        }),
        RawOp::ChangeOwnership { uuid, owner } => Ok(LayoutOp::ChangeOwnership {
            id: Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?,
            owner,
        }),
        RawOp::ChangeUuid { from, to } => Ok(LayoutOp::ChangeUuid {
            from: Uuid::from_str(&from).map_err(|error| WriteError::InvalidPath {
                path: from,
                reason: error.to_string(),
            })?,
            to: Uuid::from_str(&to).map_err(|error| WriteError::InvalidPath {
                path: to,
                reason: error.to_string(),
            })?,
        }),
    }
}

/// A path as the request writes it.
fn path_of(text: &str) -> Result<LayoutPath, WriteError> {
    LayoutPath::new(text).map_err(|error| WriteError::InvalidPath {
        path: text.to_owned(),
        reason: error.to_string(),
    })
}

/// What an entry holds, from the parts a request names.
fn mutable(
    version: &str,
    owner: Option<String>,
    description: Option<String>,
) -> Result<MutableData, WriteError> {
    let key = Key::from_str(version).map_err(|error| WriteError::Precondition {
        what: format!("version `{version}`"),
        reason: error.to_string(),
    })?;

    Ok(MutableData::new(
        owner,
        *key.digest(),
        description.unwrap_or_default(),
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{LayoutOp, apply};
    use rorolala_layout::{LayoutPath, MutableData};
    use rorolala_utils_location::Locate as _;
    use uuid::Uuid;

    #[test]
    fn a_batch_binds_and_moves_a_path() {
        let dir = std::env::temp_dir().join(format!("rola-write-layout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        rorolala_workspace::Workspace::create(&dir).unwrap();
        let workspace = rorolala_workspace::Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        let from = LayoutPath::new("a").unwrap();
        let to = LayoutPath::new("b").unwrap();
        let id = Uuid::from_u128(1);

        apply(
            &layout,
            &[
                LayoutOp::CreateEntry {
                    id,
                    path: Some(from.clone()),
                    data: MutableData::new(None, [1; 32], "first".to_owned()),
                },
                LayoutOp::MovePath {
                    from: from.clone(),
                    to: to.clone(),
                },
            ],
        )
        .unwrap();

        let read = workspace.layout().unwrap();
        assert_eq!(read.id_of(&to), Some(id));
        assert_eq!(read.id_of(&from), None);

        let _ = fs::remove_dir_all(&dir);
    }
}

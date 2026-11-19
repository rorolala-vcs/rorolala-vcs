//! Writing one Layout, as a batch of changes.

use std::path::Path;
use std::str::FromStr;

use rorolala_layout::{Batch, EntryKind, Layout, LayoutError, LayoutPath, MutableData};
use rorolala_storage::Key;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
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

/// Makes or changes what an entry holds.
///
/// A preference the entry already had is kept: setting the holder, the version or the words is not
/// saying anything about how the content is cut, and clearing it would quietly change what the next
/// write does.
///
/// # Errors
///
/// Returns [`WriteError::Layout`] when the entry is already there for `update == false`, is not
/// there for `update == true`, or the write itself fails.
pub fn write_entry(
    layout: &Layout,
    id: Uuid,
    version: [u8; 32],
    owner: Option<String>,
    description: String,
    update: bool,
) -> Result<(), WriteError> {
    let data = MutableData::new(owner, version, description);
    let data = match layout.entry(id) {
        Some(existing) => data.with_preference(existing.preference().map(str::to_owned)),
        None => data,
    };

    let written = if update {
        layout.update_entry(id, data)
    } else {
        layout.create_entry(id, data)
    };

    written.map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })
}

/// Changes the account that holds an entry, leaving its content and description as they are.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the entry is not there, and [`WriteError::Layout`] when the
/// write does not apply.
pub fn set_owner(layout: &Layout, id: Uuid, owner: Option<&str>) -> Result<(), WriteError> {
    let entry = layout.entry(id).ok_or_else(|| WriteError::NotFound {
        what: format!("Layout entry {id}"),
    })?;

    write_entry(
        layout,
        id,
        entry.version(),
        owner.map(str::to_owned),
        entry.description().to_owned(),
        true,
    )
}

/// Changes the description of an entry, leaving its content and owner as they are.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the entry is not there, and [`WriteError::Layout`] when the
/// write does not apply.
pub fn set_description(layout: &Layout, id: Uuid, description: &str) -> Result<(), WriteError> {
    let entry = layout.entry(id).ok_or_else(|| WriteError::NotFound {
        what: format!("Layout entry {id}"),
    })?;

    write_entry(
        layout,
        id,
        entry.version(),
        entry.owner().map(str::to_owned),
        description.to_owned(),
        true,
    )
}

/// Changes the cut an entry prefers, leaving its content, owner and description as they are.
///
/// An empty `preference` clears it, which is what a run does when the store may choose.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the entry is not there, and [`WriteError::Layout`] when the
/// write does not apply.
pub fn set_preference(
    layout: &Layout,
    id: Uuid,
    preference: Option<&str>,
) -> Result<(), WriteError> {
    let entry = layout.entry(id).ok_or_else(|| WriteError::NotFound {
        what: format!("Layout entry {id}"),
    })?;
    let data = MutableData::new(
        entry.owner().map(str::to_owned),
        entry.version(),
        entry.description().to_owned(),
    )
    .with_preference(preference.map(str::to_owned));

    layout
        .update_entry(id, data)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })
}

/// Changes the version an entry names, leaving its owner, description and preference as they are.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the entry is not there, and [`WriteError::Layout`] when the
/// write does not apply.
pub fn set_version(layout: &Layout, id: Uuid, version: [u8; 32]) -> Result<(), WriteError> {
    let entry = layout.entry(id).ok_or_else(|| WriteError::NotFound {
        what: format!("Layout entry {id}"),
    })?;
    let data = MutableData::new(
        entry.owner().map(str::to_owned),
        version,
        entry.description().to_owned(),
    )
    .with_preference(entry.preference().map(str::to_owned));

    layout
        .update_entry(id, data)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })
}

/// Changes everything an entry holds, in one write.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the entry is not there, and [`WriteError::Layout`] when the
/// write does not apply.
pub fn set_all(
    layout: &Layout,
    id: Uuid,
    version: [u8; 32],
    owner: Option<&str>,
    description: &str,
    preference: Option<&str>,
) -> Result<(), WriteError> {
    let data = MutableData::new(owner.map(str::to_owned), version, description.to_owned())
        .with_preference(preference.map(str::to_owned));
    layout
        .update_entry(id, data)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })
}

/// Drops an entry, and the path that named it.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the entry is not there, and [`WriteError::Layout`] when the
/// removal does not apply.
pub fn remove_entry(layout: &Layout, id: Uuid) -> Result<(), WriteError> {
    layout.remove_entry(id).map_err(|error| match error {
        rorolala_layout::LayoutError::NotFound => WriteError::NotFound {
            what: format!("Layout entry {id}"),
        },
        other => WriteError::Layout {
            reason: other.to_string(),
        },
    })
}

/// Makes or changes an entry of a Layout a directory holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
/// `owner` and `description` are empty when the entry has neither. `update` says whether the entry
/// must already be there.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`,
/// [`WriteError::NoLayout`] when it has no Layout, [`WriteError::InvalidPath`] when the `Uuid` or the
/// version is not one the Layout accepts, and [`WriteError::Layout`] when the write does not apply.
#[lazyffi(export = rola_write_layout_write_entry)]
pub fn write_entry_at(
    directory: &Path,
    layout: &str,
    uuid: &str,
    version: &str,
    owner: &str,
    description: &str,
    update: bool,
) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    let key = Key::from_str(version).map_err(|error| WriteError::Precondition {
        what: format!("version `{version}`"),
        reason: error.to_string(),
    })?;

    write_entry(
        &target,
        id,
        *key.digest(),
        (!owner.is_empty()).then(|| owner.to_owned()),
        description.to_owned(),
        update,
    )
}

/// Changes the owner of one entry of a Layout a directory holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in; an
/// empty `owner` clears the owner.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`] when the Layout is not there, [`WriteError::InvalidPath`] when
/// the entry id does not read, and the failures [`set_owner`] names.
#[lazyffi(export = rola_write_layout_set_owner)]
pub fn set_owner_at(
    directory: &Path,
    layout: &str,
    uuid: &str,
    owner: &str,
) -> Result<String, WriteError> {
    let target = open(directory, layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    set_owner(&target, id, (!owner.is_empty()).then_some(owner))?;

    Ok(String::new())
}

/// Changes the description of one entry of a Layout a directory holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`] when the Layout is not there, [`WriteError::InvalidPath`] when
/// the entry id does not read, and the failures [`set_description`] names.
#[lazyffi(export = rola_write_layout_set_description)]
pub fn set_description_at(
    directory: &Path,
    layout: &str,
    uuid: &str,
    description: &str,
) -> Result<String, WriteError> {
    let target = open(directory, layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    set_description(&target, id, description)?;

    Ok(String::new())
}

/// Changes the cut one entry of a Layout a directory holds prefers.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in; an
/// empty `preference` clears it.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`] when the Layout is not there, [`WriteError::InvalidPath`] when
/// the entry id does not read, and the failures [`set_preference`] names.
#[lazyffi(export = rola_write_layout_set_preference)]
pub fn set_preference_at(
    directory: &Path,
    layout: &str,
    uuid: &str,
    preference: &str,
) -> Result<String, WriteError> {
    let target = open(directory, layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    set_preference(&target, id, (!preference.is_empty()).then_some(preference))?;

    Ok(String::new())
}

/// Changes the version one entry of a Layout a directory holds names.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`] when the Layout is not there, [`WriteError::InvalidPath`] when
/// the entry id does not read, [`WriteError::Precondition`] when the version key does not read, and
/// the failures [`set_version`] names.
#[lazyffi(export = rola_write_layout_set_version)]
pub fn set_version_at(
    directory: &Path,
    layout: &str,
    uuid: &str,
    version: &str,
) -> Result<String, WriteError> {
    let target = open(directory, layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    let key = Key::from_str(version).map_err(|error| WriteError::Precondition {
        what: format!("version `{version}`"),
        reason: error.to_string(),
    })?;
    set_version(&target, id, *key.digest())?;

    Ok(String::new())
}

/// Changes everything an entry of a Layout a directory holds holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in; an
/// empty `owner` or `preference` clears it.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`] when the Layout is not there, [`WriteError::InvalidPath`] when
/// the entry id does not read, [`WriteError::Precondition`] when the version key does not read, and
/// the failures [`set_all`] names.
#[lazyffi(export = rola_write_layout_set_all)]
pub fn set_all_at(
    directory: &Path,
    layout: &str,
    uuid: &str,
    version: &str,
    owner: &str,
    description: &str,
    preference: &str,
) -> Result<String, WriteError> {
    let target = open(directory, layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    let key = Key::from_str(version).map_err(|error| WriteError::Precondition {
        what: format!("version `{version}`"),
        reason: error.to_string(),
    })?;
    set_all(
        &target,
        id,
        *key.digest(),
        (!owner.is_empty()).then_some(owner),
        description,
        (!preference.is_empty()).then_some(preference),
    )?;

    Ok(String::new())
}

/// Drops an entry of a Layout a directory holds.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`,
/// [`WriteError::NoLayout`] when it has no Layout, [`WriteError::InvalidPath`] when the `Uuid` is not
/// one the Layout accepts, and [`WriteError::Layout`] when the removal does not apply.
#[lazyffi(export = rola_write_layout_remove_entry)]
pub fn remove_entry_at(directory: &Path, layout: &str, uuid: &str) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;

    remove_entry(&target, id)
}

/// Gives the entry at a path a new `Uuid`, keeping what it holds.
///
/// The old entry is dropped and an entry under `id` is made or changed in one batch, so a reader sees
/// the path naming one of them and never neither.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the path names nothing, and [`WriteError::Layout`] when the
/// batch does not apply.
pub fn switch_entry(
    layout: &Layout,
    path: &LayoutPath,
    id: Uuid,
    data: MutableData,
) -> Result<(), WriteError> {
    let Some(old) = layout.id_of(path) else {
        return Err(WriteError::NotFound {
            what: format!("Layout path `{}`", path.as_str()),
        });
    };
    if old == id {
        return Ok(());
    }

    let mut batch = layout.batch();
    batch
        .remove_entry(old)
        .map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })?;

    let written = if batch.entry(id).is_some() {
        batch
            .update_entry(id, data)
            .and_then(|()| batch.create_path(path, id))
    } else {
        batch.create_file(id, data, path)
    };
    written.map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;

    batch.commit().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })
}

/// Gives the entry at a path of a Layout a directory holds a new `Uuid`.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `layout` is `main`, `truth`, or empty for the one being worked in.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`,
/// [`WriteError::NoLayout`] when it has no Layout, [`WriteError::InvalidPath`] when a path, a `Uuid`
/// or a version does not read, and [`WriteError::Layout`] when the batch does not apply.
#[lazyffi(export = rola_write_layout_switch_entry)]
// The export is the ABI's own shape: a caller writing one entry by hand names each of these, and
// folding them into a struct would only move the same arguments across one more boundary.
#[allow(clippy::too_many_arguments)]
pub fn switch_entry_at(
    directory: &Path,
    layout: &str,
    path: &str,
    uuid: &str,
    version: &str,
    owner: &str,
    description: &str,
    preference: &str,
) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let path = path_of(path)?;
    let id = Uuid::from_str(uuid).map_err(|error| WriteError::InvalidPath {
        path: uuid.to_owned(),
        reason: error.to_string(),
    })?;
    let data = mutable(
        version,
        (!owner.is_empty()).then(|| owner.to_owned()),
        (!description.is_empty()).then(|| description.to_owned()),
    )?
    .with_preference((!preference.is_empty()).then(|| preference.to_owned()));

    switch_entry(&target, &path, id, data)
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
pub(crate) fn path_of(text: &str) -> Result<LayoutPath, WriteError> {
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

/// The copy of a Vault's truth Layout a fetch left in a Workspace.
///
/// # Errors
///
/// Returns [`LayoutError`] when the copy is there but does not read.
pub fn cached_copy(workspace: &Workspace, vault: &str) -> Result<Option<Layout>, LayoutError> {
    let Some(dir) = workspace.readonly_layout_copy(vault, VAULT_LAYOUT_NAME) else {
        return Ok(None);
    };

    // A directory the Layout's own files are not in is not a copy, and reading it must not make one.
    match Layout::open_existing(&dir) {
        Ok(copy) => Ok(Some(copy)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(error),
    }
}

/// Adds an entry to a fetched copy, unless the copy already holds the Uuid or names the path.
///
/// # Errors
///
/// Returns [`LayoutError`] when the entry cannot be made.
pub fn add_cached_entry(
    copy: &mut Batch<'_>,
    id: Uuid,
    path: &LayoutPath,
    version: [u8; 32],
    owner: Option<String>,
) -> Result<(), LayoutError> {
    if copy.entry(id).is_some() || copy.names(path) {
        return Ok(());
    }

    copy.create_file(id, MutableData::new(owner, version, String::new()), path)
}

/// Moves an entry of a fetched copy to a version, leaving its path and holder alone.
///
/// # Errors
///
/// Returns [`LayoutError`] when the entry cannot be changed.
pub fn set_cached_version(
    copy: &mut Batch<'_>,
    id: Uuid,
    version: [u8; 32],
) -> Result<(), LayoutError> {
    let Some(data) = copy.entry(id) else {
        return Ok(());
    };

    copy.update_entry(id, data.renewed(data.owner().map(str::to_owned), version))
}

/// Moves entries of a fetched copy to new owners, leaving their paths and versions alone.
///
/// # Errors
///
/// Returns [`LayoutError`] when an entry cannot be changed.
pub fn set_cached_owners(
    copy: &mut Batch<'_>,
    changes: &[(Uuid, Option<String>)],
) -> Result<(), LayoutError> {
    for (id, owner) in changes {
        let Some(data) = copy.entry(*id) else {
            continue;
        };

        copy.update_entry(*id, data.renewed(owner.clone(), data.version()))?;
    }

    Ok(())
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

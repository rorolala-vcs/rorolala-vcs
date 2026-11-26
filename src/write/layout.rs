//! Writing one Layout, as a batch of changes.

use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use rorolala_layout::{Batch, EntryKind, Layout, LayoutError, LayoutPath, MutableData};
use rorolala_storage::Key;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vault::Vault;
use rorolala_workspace::{Merging, Workspace};
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
    /// Moves the path under `@/removed/<date>/`, leaving the entry where a reader can find it.
    SoftDeletePath {
        /// The path to archive.
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
            LayoutOp::SoftDeletePath { path } => soft_delete(&mut batch, &path)?,
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

/// Where a soft delete puts what it takes away: under the Vault's own archived marker.
///
/// The value is the one every reader in this tree tests a path against to read an entry as
/// deprecated, so a soft delete is one every reader already understands.
const SOFT_DELETED_PREFIX: &str = "@/removed/";

/// The path a soft delete of `path` moves it to: `@/removed/<date>/<path>`.
///
/// The date is the day the caller runs, in UTC, so that two machines in two zones archive the same
/// work under the same day. What is kept under it is the path as the Layout writes it, so a reader
/// sees where the entry came from.
///
/// # Panics
///
/// Panics when `@/removed/` and a date do not make a path a Layout accepts, which cannot happen:
/// both are this function's own and `path` was accepted already.
#[must_use]
pub fn soft_delete_target(path: &LayoutPath) -> LayoutPath {
    // UNWRAP: `@/removed/`, a date of digits and dashes, and a path a Layout already accepted make
    // a path a Layout accepts, which is what the tests beside this state.
    LayoutPath::new(&format!(
        "{SOFT_DELETED_PREFIX}{}/{}",
        utc_date(),
        path.as_str()
    ))
    .expect("a path under `@/removed/` with a date in it")
}

/// Whether a soft delete of `path` is one this accepts.
///
/// A path already under the archived marker is refused: the target would be
/// `@/removed/<date>/@/removed/...`, nesting the marker, and no reader reads a nested one as where
/// an entry is.
#[must_use]
pub fn soft_deletable(path: &LayoutPath) -> bool {
    !path.as_str().starts_with(SOFT_DELETED_PREFIX)
}

/// Today, in UTC, as `YYYY-MM-DD`.
///
/// The clock is read here rather than taken from a dependency: the workspace carries no date crate,
/// and a civil date is the one arithmetic below. A clock that reads before 1970 dates the day the
/// epoch did, since nothing this archives is older than the machine's own fault.
fn utc_date() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
        / 86_400;

    civil_from_days(days)
}

/// The civil date `days` days after 1970-01-01, as `YYYY-MM-DD`.
///
/// Howard Hinnant's days-to-civil algorithm, whose epochs are shifted here so that zero is the Unix
/// one. It is the shape of the arithmetic rather than a table, so it holds before and after a leap
/// year and a century without a second rule.
fn civil_from_days(days: u64) -> String {
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_of_year = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_of_year + 2) / 5 + 1;
    let month = if month_of_year < 10 {
        month_of_year + 3
    } else {
        month_of_year - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02}")
}

/// Moves `path` under `@/removed/`, refusing a path that is already there.
fn soft_delete(batch: &mut Batch<'_>, path: &LayoutPath) -> Result<(), WriteError> {
    if !soft_deletable(path) {
        return Err(WriteError::InvalidPath {
            path: path.as_str().to_owned(),
            reason: "a path under `@/removed/` is not archived again".to_owned(),
        });
    }

    batch
        .move_path(path, &soft_delete_target(path))
        .map_err(|error| WriteError::Layout {
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

/// Moves a path of a Layout a directory holds under `@/removed/<date>/`.
///
/// The date is the day the call is made, in UTC, and the path is kept under it as the Layout writes
/// it. The entry stays, so what reads the Layout afterwards finds it under the marker as deprecated
/// rather than gone.
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
/// one a Layout accepts or is already under `@/removed/`, [`WriteError::NotFound`] when the path
/// names nothing, and [`WriteError::Layout`] when the move does not apply.
#[lazyffi(export = rola_write_layout_soft_delete_path)]
pub fn soft_delete_path_at(directory: &Path, layout: &str, path: &str) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let path = path_of(path)?;

    if !soft_deletable(&path) {
        return Err(WriteError::InvalidPath {
            path: path.as_str().to_owned(),
            reason: "a path under `@/removed/` is not archived again".to_owned(),
        });
    }

    let to = soft_delete_target(&path);
    let workspace = Workspace::locate(directory);

    move_path(&target, workspace.as_ref(), &path, &to)
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

/// Changes the version an entry names, leaving its kind, owner, description and preference as they
/// are.
///
/// # Errors
///
/// Returns [`WriteError::NotFound`] when the entry is not there, and [`WriteError::Layout`] when the
/// write does not apply.
pub fn set_version(layout: &Layout, id: Uuid, version: [u8; 32]) -> Result<(), WriteError> {
    let entry = layout.entry(id).ok_or_else(|| WriteError::NotFound {
        what: format!("Layout entry {id}"),
    })?;
    // `renewed` is what keeps an entry the kind it was made: a tree whose version moved with
    // `new` would be recorded as a file from the change on.
    let data = entry.renewed(entry.owner().map(str::to_owned), version);

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
#[allow(
    clippy::too_many_arguments,
    reason = "the export is the ABI's own shape, and folding these arguments into a struct would only move them across one more boundary"
)]
#[lazyffi(export = rola_write_layout_switch_entry)]
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

/// One change, as a C caller names it.
///
/// It mirrors the request document the command line reads field for field, except that a name the
/// change does not use is the empty string rather than absent.
///
/// # FFI
///
/// A `RolaWriteLayoutOp` the caller fills: `tag` selects the change and the payload companion it
/// names holds that change's strings. Every `const char *` is borrowed for the call and released by
/// the caller, so nothing here is freed when the call returns.
#[lazyffi(export = RolaWriteLayoutOp)]
#[derive(Debug, Clone)]
pub enum WriteLayoutOp {
    /// Makes an entry.
    CreateEntry {
        /// The entry's `Uuid`, as text.
        uuid: String,
        /// The path it is named at, empty to leave it unplaced.
        path: String,
        /// The version it holds.
        version: String,
        /// The account that holds it, empty for none.
        owner: String,
        /// What the entry says about itself, empty for nothing.
        description: String,
        /// `tree` to make a tree entry, empty for a file.
        kind: String,
    },
    /// Changes what an entry holds.
    UpdateEntry {
        /// The entry's `Uuid`, as text.
        uuid: String,
        /// The version it now holds.
        version: String,
        /// The account that holds it, empty for none.
        owner: String,
        /// What the entry says about itself, empty for nothing.
        description: String,
    },
    /// Removes an entry.
    RemoveEntry {
        /// The entry's `Uuid`, as text.
        uuid: String,
    },
    /// Binds a path.
    CreatePath {
        /// The path.
        path: String,
        /// The entry's `Uuid`, as text.
        uuid: String,
    },
    /// Moves a path.
    MovePath {
        /// The path it is at.
        from: String,
        /// The path it is to be at.
        to: String,
    },
    /// Removes a path.
    RemovePath {
        /// The path.
        path: String,
    },
    /// Moves the path under `@/removed/<date>/`, leaving the entry.
    SoftDeletePath {
        /// The path.
        path: String,
    },
    /// Changes who holds an entry.
    ChangeOwnership {
        /// The entry's `Uuid`, as text.
        uuid: String,
        /// The account that holds it, empty for none.
        owner: String,
    },
    /// Changes an entry's `Uuid`.
    ChangeUuid {
        /// The `Uuid` it is at, as text.
        from: String,
        /// The `Uuid` it is to be at, as text.
        to: String,
    },
}

/// Applies a batch of changes to the Layout a directory is in.
///
/// # FFI
///
/// `directory` and `layout` are borrowed for the call, and `layout` is `main`, `truth`, or empty
/// for the one being worked in. `ops` and `ops_len` are the caller's array of changes, borrowed for
/// the call and released by the caller. An empty answer on success, or a `RolaWriteError *` the
/// caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`] when the place has no Layout, [`WriteError::InvalidPath`] or
/// [`WriteError::NotFound`] when a name does not read, and [`WriteError::Layout`] when a change does
/// not apply.
#[lazyffi(export = rola_write_layout_apply)]
pub fn apply_ops(directory: &Path, layout: &str, ops: &[WriteLayoutOp]) -> Result<(), WriteError> {
    let target = open(directory, layout)?;
    let ops = ops
        .iter()
        .cloned()
        .map(op_of)
        .collect::<Result<Vec<_>, _>>()?;

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

/// Turns one change into a typed one, with an empty name read as absent.
fn op_of(raw: WriteLayoutOp) -> Result<LayoutOp, WriteError> {
    match raw {
        WriteLayoutOp::CreateEntry {
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
            let path = (!path.is_empty()).then(|| path_of(&path)).transpose()?;
            let mut data = mutable(&version, none_if_empty(owner), none_if_empty(description))?;
            if kind == "tree" {
                data = data.with_kind(EntryKind::Tree);
            }

            Ok(LayoutOp::CreateEntry { id, path, data })
        }
        WriteLayoutOp::UpdateEntry {
            uuid,
            version,
            owner,
            description,
        } => {
            let id = Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?;
            let data = mutable(&version, none_if_empty(owner), none_if_empty(description))?;

            Ok(LayoutOp::UpdateEntry { id, data })
        }
        WriteLayoutOp::RemoveEntry { uuid } => Ok(LayoutOp::RemoveEntry {
            id: Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?,
        }),
        WriteLayoutOp::CreatePath { path, uuid } => Ok(LayoutOp::CreatePath {
            path: path_of(&path)?,
            id: Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?,
        }),
        WriteLayoutOp::MovePath { from, to } => Ok(LayoutOp::MovePath {
            from: path_of(&from)?,
            to: path_of(&to)?,
        }),
        WriteLayoutOp::RemovePath { path } => Ok(LayoutOp::RemovePath {
            path: path_of(&path)?,
        }),
        WriteLayoutOp::SoftDeletePath { path } => Ok(LayoutOp::SoftDeletePath {
            path: path_of(&path)?,
        }),
        WriteLayoutOp::ChangeOwnership { uuid, owner } => Ok(LayoutOp::ChangeOwnership {
            id: Uuid::from_str(&uuid).map_err(|error| WriteError::InvalidPath {
                path: uuid,
                reason: error.to_string(),
            })?,
            owner: none_if_empty(owner),
        }),
        WriteLayoutOp::ChangeUuid { from, to } => Ok(LayoutOp::ChangeUuid {
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

/// The text, or nothing when it names none.
fn none_if_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
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

    use super::{
        LayoutOp, apply, apply_ops, civil_from_days, set_version, soft_deletable,
        soft_delete_target,
    };
    use crate::write::WriteError;
    use rorolala_layout::{EntryKind, LayoutPath, MutableData};
    use rorolala_utils_location::Locate as _;
    use uuid::Uuid;

    #[test]
    fn a_version_change_keeps_a_tree_a_tree() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-layout-tree-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        rorolala_workspace::Workspace::create(&dir).unwrap();
        let workspace = rorolala_workspace::Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        let id = Uuid::from_u128(2);
        let path = LayoutPath::new("art").unwrap();

        let mut batch = layout.batch();
        batch
            .create_file(
                id,
                MutableData::new(None, [1; 32], "first".to_owned()).with_kind(EntryKind::Tree),
                &path,
            )
            .unwrap();
        batch.commit().unwrap();
        set_version(&layout, id, [2; 32]).unwrap();

        let data = layout.entry(id).unwrap();
        assert_eq!(data.kind(), EntryKind::Tree);
        assert_eq!(data.version(), [2; 32]);
        assert_eq!(data.description(), "first");

        let _ = fs::remove_dir_all(&dir);
    }

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

    #[test]
    fn a_soft_delete_op_moves_the_path_and_keeps_the_entry() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-soft-delete-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        rorolala_workspace::Workspace::create(&dir).unwrap();
        let workspace = rorolala_workspace::Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::new("art/a.psd").unwrap();
        let id = Uuid::from_u128(3);

        apply(
            &layout,
            &[
                LayoutOp::CreateEntry {
                    id,
                    path: Some(path.clone()),
                    data: MutableData::new(None, [1; 32], "first".to_owned()),
                },
                LayoutOp::SoftDeletePath { path: path.clone() },
            ],
        )
        .unwrap();

        let read = workspace.layout().unwrap();
        let target = soft_delete_target(&path);

        assert_eq!(read.id_of(&target), Some(id));
        assert_eq!(read.id_of(&path), None);
        assert!(read.entry(id).is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_soft_delete_op_refuses_a_path_already_archived() {
        let dir = std::env::temp_dir().join(format!("rola-write-archived-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        rorolala_workspace::Workspace::create(&dir).unwrap();
        let workspace = rorolala_workspace::Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::new("@/removed/2026-01-01/a.psd").unwrap();

        assert!(matches!(
            apply(&layout, &[LayoutOp::SoftDeletePath { path }]),
            Err(WriteError::InvalidPath { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_soft_delete_target_keeps_the_path_under_the_day() {
        let target = soft_delete_target(&LayoutPath::new("art/a.psd").unwrap());
        let text = target.as_str();
        let (marker, rest) = text.split_once("@/removed/").unwrap();

        assert_eq!(marker, "");

        // The day is the run's own, so what is checked is its shape rather than the day itself.
        let (day, path) = rest.split_once('/').unwrap();

        assert_eq!(day.len(), 10);
        assert_eq!(path, "art/a.psd");
    }

    #[test]
    fn a_day_counts_from_the_unix_epoch() {
        assert_eq!(civil_from_days(0), "1970-01-01");
        assert_eq!(civil_from_days(11_016), "2000-02-29");
        assert_eq!(civil_from_days(11_017), "2000-03-01");
        assert_eq!(civil_from_days(20_454), "2026-01-01");
    }

    #[test]
    fn a_path_already_archived_is_not_soft_deleted_again() {
        assert!(!soft_deletable(
            &LayoutPath::new("@/removed/2026-01-01/a.psd").unwrap()
        ));
        assert!(soft_deletable(&LayoutPath::new("@/new/a.psd").unwrap()));
        assert!(soft_deletable(&LayoutPath::new("art/a.psd").unwrap()));
    }

    #[test]
    fn a_batch_needs_a_workspace_and_a_layout_it_knows() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-layout-open-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        assert!(matches!(
            apply_ops(&dir, "", &[]),
            Err(WriteError::NoWorkspace)
        ));

        rorolala_workspace::Workspace::create(&dir).unwrap();
        assert!(matches!(
            apply_ops(&dir, "other", &[]),
            Err(WriteError::Precondition { .. })
        ));
        assert!(apply_ops(&dir, "", &[]).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod op_tests {
    use std::fs;

    use rorolala_layout::EntryKind;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{
        LayoutOp, LayoutPath, WriteLayoutOp, add_cached_entry, cached_copy, none_if_empty, op_of,
        path_of, set_cached_owners, set_cached_version,
    };

    fn version() -> String {
        "a".repeat(64)
    }

    #[test]
    fn every_write_op_becomes_the_typed_one() {
        let id = Uuid::from_u128(1);
        let text = id.to_string();

        let created = op_of(WriteLayoutOp::CreateEntry {
            uuid: text.clone(),
            path: "a/b.psd".to_owned(),
            version: version(),
            owner: "alice".to_owned(),
            description: "first".to_owned(),
            kind: "tree".to_owned(),
        })
        .unwrap();
        let LayoutOp::CreateEntry {
            id: got,
            path,
            data,
        } = created
        else {
            panic!("a create was made");
        };
        assert_eq!(got, id);
        assert_eq!(path.unwrap().as_str(), "a/b.psd");
        assert_eq!(data.owner(), Some("alice"));
        assert_eq!(data.kind(), EntryKind::Tree);

        let updated = op_of(WriteLayoutOp::UpdateEntry {
            uuid: text.clone(),
            version: version(),
            owner: String::new(),
            description: String::new(),
        })
        .unwrap();
        assert!(matches!(updated, LayoutOp::UpdateEntry { id: got, .. } if got == id));

        let removed = op_of(WriteLayoutOp::RemoveEntry { uuid: text.clone() }).unwrap();
        assert!(matches!(removed, LayoutOp::RemoveEntry { id: got } if got == id));

        let bound = op_of(WriteLayoutOp::CreatePath {
            path: "a/b.psd".to_owned(),
            uuid: text.clone(),
        })
        .unwrap();
        assert!(matches!(bound, LayoutOp::CreatePath { id: got, .. } if got == id));

        let moved = op_of(WriteLayoutOp::MovePath {
            from: "a".to_owned(),
            to: "b".to_owned(),
        })
        .unwrap();
        assert!(matches!(
            moved,
            LayoutOp::MovePath { from, to } if from.as_str() == "a" && to.as_str() == "b"
        ));

        let unbound = op_of(WriteLayoutOp::RemovePath {
            path: "a".to_owned(),
        })
        .unwrap();
        assert!(matches!(unbound, LayoutOp::RemovePath { path } if path.as_str() == "a"));

        let archived = op_of(WriteLayoutOp::SoftDeletePath {
            path: "a".to_owned(),
        })
        .unwrap();
        assert!(matches!(archived, LayoutOp::SoftDeletePath { path } if path.as_str() == "a"));

        let held = op_of(WriteLayoutOp::ChangeOwnership {
            uuid: text.clone(),
            owner: String::new(),
        })
        .unwrap();
        assert!(matches!(held, LayoutOp::ChangeOwnership { id: got, owner: None } if got == id));

        let changed = op_of(WriteLayoutOp::ChangeUuid {
            from: text,
            to: Uuid::from_u128(2).to_string(),
        })
        .unwrap();
        assert!(
            matches!(changed, LayoutOp::ChangeUuid { from, to } if from == id && to == Uuid::from_u128(2))
        );
    }

    #[test]
    fn a_name_that_does_not_read_is_refused() {
        assert!(
            op_of(WriteLayoutOp::RemoveEntry {
                uuid: "not a uuid".to_owned(),
            })
            .is_err()
        );
        assert!(
            op_of(WriteLayoutOp::RemovePath {
                path: String::new(),
            })
            .is_err()
        );
        assert!(
            op_of(WriteLayoutOp::UpdateEntry {
                uuid: Uuid::from_u128(1).to_string(),
                version: "not a version".to_owned(),
                owner: String::new(),
                description: String::new(),
            })
            .is_err()
        );

        assert_eq!(none_if_empty(String::new()), None);
        assert_eq!(none_if_empty("a".to_owned()), Some("a".to_owned()));
        assert!(path_of("").is_err());
    }

    #[test]
    fn a_fetched_copy_is_written_and_read_back() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-layout-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let id = Uuid::from_u128(1);
        let path = LayoutPath::new("a.psd").unwrap();

        assert!(cached_copy(&workspace, "alpha").unwrap().is_none());

        let layout = workspace.layout().unwrap();
        let mut batch = layout.batch();
        add_cached_entry(&mut batch, id, &path, [1; 32], Some("alice".to_owned())).unwrap();
        add_cached_entry(&mut batch, id, &path, [1; 32], None).unwrap();
        batch.commit().unwrap();

        let layout = workspace.layout().unwrap();
        let mut batch = layout.batch();
        set_cached_version(&mut batch, id, [2; 32]).unwrap();
        set_cached_version(&mut batch, Uuid::from_u128(2), [2; 32]).unwrap();
        set_cached_owners(
            &mut batch,
            &[(id, Some("bob".to_owned())), (Uuid::from_u128(2), None)],
        )
        .unwrap();
        batch.commit().unwrap();

        let read = workspace.layout().unwrap();
        let entry = read.entry(id).unwrap();
        assert_eq!(entry.version(), [2; 32]);
        assert_eq!(entry.owner(), Some("bob"));

        let _ = fs::remove_dir_all(&dir);
    }
}

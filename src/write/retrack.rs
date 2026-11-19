//! Moving a path's record to a version, and its work with it.
//!
//! A Layout names one version for each path, and the content behind a version stays in the index
//! whether or not anything points at it. What a run names after the path is what the path is to
//! record: a version (a step back, a number, or a hash) which moves the entry's pointer, or another
//! entry (a `Uuid`, or a path of the Vault's own Layout) which makes the path record that entry in
//! place of the one it recorded. A fork is the third: a fresh `Uuid` holding what the path records
//! now.
//!
//! Moving the pointer and leaving the file alone would leave the two apart, the file holds what a
//! newer version did and the Layout now names an older one, so the version's content is written back
//! over the file as well and the reading is told the two agree again. `only_index` is the run that
//! wants the record moved and the work left alone.
//!
//! A file with changes that were never recorded, and one a variant is waiting to be joined into, are
//! not states a retrack that writes content may write over: both are refused. `force` is how a run
//! says to go ahead anyway, the changes are discarded and the merge ended with its variant file.
//!
//! A tree is one Layout entry rather than content at a path: what moves back is the whole structure,
//! and what the work holds is put back from it instead of from a content object. A path inside a
//! tree names the tree, since that is the entry the Layout actually has.
//!
//! What a run may write is read before anything is written, so a refused run leaves the Layout, the
//! tree and the merge exactly as they were. The one exception is a run whose store turns out to be
//! short of the content after the Layout was written: it leaves a record naming a version the file
//! does not hold, which a reading reports as a change and a second run puts right.
//!
//! This module also owns the narrower one-file operation the C ABI exports, which reads no remote
//! copy and knows no tree; see [`retrack_file`].

use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rorolala_auth::Account;
use rorolala_errors::Failure as _;
use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::{EntryKind as LayoutEntryKind, Layout, LayoutPath, MutableData};
use rorolala_protocol::{ActionError, VaultAddress};
use rorolala_storage::{Blake3Hash, Key, Resolved, ShortHash, StorageSet, key_of_file};
use rorolala_utils_constants::{VAULT_LAYOUT_NAME, WORKSPACE_IGNORE_RULES_DIR};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_utils_progress::Progress;
use rorolala_vault::Vault;
use rorolala_vcs::{VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version};
use rorolala_workspace::{Merging, Pending, Workspace};
use rorolala_workspace_analyze::{Cache, cache_path, entry_of, tree_diff_remembering};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon::{Chain, action_bring_remote_index_async, action_fetch_layout};
use crate::fetch::Sources;
use crate::status::variant_moves;
use crate::tree::{
    EntryKind as TreeEntryKind, Staging, Tree, TreeStructure, compare, conflict_copy,
    read_structure,
};
use crate::write::checkout::{forget, remember, restore as restore_path, restore_vault};
use crate::write::layout::{set_version, switch_entry};
use crate::write::progress::{WriteProgress, watching};
use crate::write::request::Common;
use crate::write::tree::relative_to;
use crate::write::{Cancellation, WriteError};

/// How alike two text files have to be to count as the same file moved.
///
/// It is the same default the reading uses, since what this acts on is what the reading found.
const DEFAULT_ALIKE: f32 = 0.6;

/// What retracking one path wrote.
#[derive(Debug, Clone, Serialize)]
pub struct Retracked {
    /// The Layout entry the path names.
    pub id: Uuid,
    /// The version the Layout now records.
    pub version: Key,
}

/// What a retrack may do beside moving the version.
///
/// `fork` is the one-file operation's spelling of [`Recording::Fork`]: a workflow run states the
/// same choice in what it named, so it reads the other three and leaves this one to
/// [`retrack_file`].
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
/// history cannot be read, and the failures [`restore_path`] names.
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
    let resolved = resolve(cwd, &root, path, "the path is outside the Workspace")?;
    let relative = relative_to(&root, &resolved).ok_or_else(|| WriteError::InvalidPath {
        path: path.to_owned(),
        reason: "the path is outside the Workspace".to_owned(),
    })?;
    let place = RetrackPlace {
        layout: &layout,
        index: &index,
    };

    retrack_in(
        &place,
        &resolved,
        &relative,
        path,
        target,
        options,
        |version| restore_path(workspace, cwd, path, version).map(|_| ()),
        || forget(&layout, &root, &relative),
    )
}

/// Moves the record of `path` in a Vault to `target`, or one version back when it is not named.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Vault or names no entry,
/// [`WriteError::Precondition`] when there is no version to move to, [`WriteError::Index`] when the
/// history cannot be read, and the failures [`restore_vault`] names.
pub fn retrack_vault_file(
    vault: &Vault,
    cwd: &Path,
    path: &str,
    target: Option<&str>,
    options: RetrackOptions,
) -> Result<Retracked, WriteError> {
    let layout = vault.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(vault.get_root());
    let resolved = resolve(cwd, &root, path, "the path is outside the Vault")?;
    let relative = relative_to(&root, &resolved).ok_or_else(|| WriteError::InvalidPath {
        path: path.to_owned(),
        reason: "the path is outside the Vault".to_owned(),
    })?;
    let place = RetrackPlace {
        layout: &layout,
        index: &index,
    };

    retrack_in(
        &place,
        &resolved,
        &relative,
        path,
        target,
        options,
        |version| restore_vault(vault, cwd, path, version).map(|_| ()),
        || Ok(()),
    )
}

/// The parts a retrack works over.
struct RetrackPlace<'a> {
    layout: &'a Layout,
    index: &'a VCSIndex,
}

/// Moves one path's record, through the scope's own restore and forget.
#[allow(
    clippy::too_many_arguments,
    reason = "the scope's restore and forget are handed in beside the path they work on"
)]
fn retrack_in<F, G>(
    place: &RetrackPlace<'_>,
    resolved: &Path,
    relative: &str,
    path: &str,
    target: Option<&str>,
    options: RetrackOptions,
    restore: F,
    forget: G,
) -> Result<Retracked, WriteError>
where
    F: FnOnce(&str) -> Result<(), WriteError>,
    G: FnOnce() -> Result<(), WriteError>,
{
    let layout = place.layout;
    let index = place.index;
    let layout_path = LayoutPath::new(relative).map_err(|error| WriteError::InvalidPath {
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
        switch_entry(layout, &layout_path, forked, data)?;

        return Ok(Retracked {
            id: forked,
            version: current,
        });
    }
    let named = asked_version(&current_variant, target, path)?;

    index
        .read_version(&named.to_string())
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;
    if options.deny_jump && named != current && !is_ancestor(index, &named, &current)? {
        return Err(WriteError::Precondition {
            what: path.to_owned(),
            reason: "the target is not on the path's chain".to_owned(),
        });
    }
    if !options.only_index && !options.force {
        let on_disk = key_of_file(resolved).map_err(|error| WriteError::Storage {
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
        set_version(layout, id, *named.digest())?;
        forget()?;
    } else {
        restore(&named.to_string())?;
    }

    Ok(Retracked { id, version: named })
}

/// The resolved path, refused when it is outside the scope.
fn resolve(
    cwd: &Path,
    root: &Path,
    path: &str,
    outside: &str,
) -> Result<std::path::PathBuf, WriteError> {
    let resolved = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = normalize(&resolved);
    if !resolved.starts_with(root) {
        return Err(WriteError::InvalidPath {
            path: path.to_owned(),
            reason: outside.to_owned(),
        });
    }

    Ok(resolved)
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

/// The version a one-file retrack asked for, or the one behind the one the path records.
fn asked_version(
    current_variant: &Variant,
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

/// Moves the record of a path under a Vault to a version.
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
/// Returns [`WriteError::NoVault`] when no Vault holds `directory`, and the failures
/// [`retrack_vault_file`] names.
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "the C ABI spells each retrack flag as its own bool"
)]
#[lazyffi(export = rola_write_retrack_vault_file)]
pub fn retrack_vault_file_at(
    directory: &Path,
    path: &str,
    target: &str,
    fork: bool,
    only_index: bool,
    force: bool,
    deny_jump: bool,
) -> Result<String, WriteError> {
    let vault = Vault::locate(directory).ok_or(WriteError::NoVault)?;
    let target = (!target.is_empty()).then_some(target);
    let options = RetrackOptions {
        fork,
        only_index,
        force,
        deny_jump,
    };
    let retracked = retrack_vault_file(&vault, directory, path, target, options)?;

    serde_json::to_string(&retracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// What a path outside the Workspace is refused with, whichever way the run named it.
const OUTSIDE_WORKSPACE: &str = "the path is outside the Workspace";

/// A retrack asked for as one value, so a command line and a C caller fill the same request.
///
/// [`Common`] comes flattened, so the Workspace, the account, the Vault, the offline and force
/// flags, the concurrency and the labels are the fields every write request carries. `record` is
/// what the path is to record, read the way the command reads it: `~N` is N versions back, a number
/// is the version numbered N, a hash is a version, a `Uuid` or a path of the Vault's Layout is
/// another entry, and nothing at all is one version back.
#[derive(Debug, Clone, Deserialize)]
pub struct RetrackRequest {
    /// The fields every write request carries.
    #[serde(flatten)]
    common: Common,
    /// The one file or tree to retrack, as the run wrote it.
    file: String,
    /// What the path is to record, empty when the run names nothing and wants one version back.
    #[serde(default)]
    record: String,
    /// Whether the record moves and the work on disk is left alone.
    #[serde(default)]
    only_index: bool,
    /// Whether a version or an entry off the way to what the path records is refused.
    #[serde(default)]
    deny_jump: bool,
    /// Whether the path is given a fresh `Uuid` holding what it records now.
    #[serde(default)]
    fork: bool,
}

impl RetrackRequest {
    /// Moves what the named path records, and its work with it, under the request's Workspace.
    ///
    /// The resources are resolved in the order the command resolves them, and what the run turns out
    /// not to need is allowed to be missing: a retrack that stays local reads no Vault and no
    /// account, so a Workspace that binds neither still retracks what it holds. The store and the
    /// index are resolved the same way, since a move of the record alone needs neither. A run that
    /// does need one is answered by [`retrack_path`] with the failure that need is.
    ///
    /// `file` is resolved against the Workspace root, relative or absolute, since a C caller has no
    /// process directory of its own to be relative to.
    ///
    /// `progress` is what the exchanges say themselves to, and `cancellation` is checked before the
    /// run touches anything and between its steps.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoWorkspace`] or [`WriteError::NoLayout`] when the request names none,
    /// [`WriteError::InvalidPath`] when `file` is outside the Workspace or names no entry,
    /// [`WriteError::Precondition`] when the request's flags disagree, when `record` names no target
    /// the run can use, or when the path may not be written over, [`WriteError::NoIndex`],
    /// [`WriteError::NoStore`], [`WriteError::NoAccount`], [`WriteError::NoTrack`],
    /// [`WriteError::NoVault`] or [`WriteError::Offline`] when the run needs one and it is not here,
    /// [`WriteError::Action`] when an exchange with a Vault fails, [`WriteError::Cancelled`] when
    /// `cancellation` was set, and the Layout, storage, index and tree failures [`retrack_path`]
    /// names.
    pub fn run(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<RetrackOutcome, WriteError> {
        // A fork records what the path holds now under a fresh `Uuid`, so a target beside it names
        // nothing the run could act on.
        if self.fork && !self.record.is_empty() {
            return Err(WriteError::Precondition {
                what: self.file.clone(),
                reason: "a fork names no target".to_owned(),
            });
        }
        // The two say opposite things about the work on disk: one moves the record and leaves the
        // file, the other is what may write over it. A request that gives both says neither.
        if self.only_index && self.common.force {
            return Err(WriteError::Precondition {
                what: self.file.clone(),
                reason: "a run that leaves the work alone cannot also go past changes to it"
                    .to_owned(),
            });
        }

        let workspace = self.common.workspace()?;
        let index = self.common.index(&workspace).ok();
        let recording = if self.fork {
            Recording::Fork
        } else {
            let record = (!self.record.is_empty()).then_some(self.record.as_str());

            recording_of(record, || version_keys(index.as_ref())).map_err(|refusal| {
                WriteError::Precondition {
                    what: format!("the target `{}`", self.record),
                    reason: refusal.to_string(),
                }
            })?
        };
        let layout = self.common.layout(&workspace)?;
        let root = normalize(workspace.get_root());
        // A caller has no process directory of its own, so a relative file is read against the
        // Workspace root, which is the only directory the request names.
        let resolved = resolve(&root, &root, &self.file, OUTSIDE_WORKSPACE)?;
        let relative = relative_to(&root, &resolved).ok_or_else(|| WriteError::InvalidPath {
            path: self.file.clone(),
            reason: OUTSIDE_WORKSPACE.to_owned(),
        })?;
        let path = LayoutPath::new(&relative).map_err(|error| WriteError::InvalidPath {
            path: self.file.clone(),
            reason: error.to_string(),
        })?;

        // The Vault the run reaches is the one it named, and the tracked one when it named none. A
        // Vault with no name is left out, since the copy of its Layout would be kept under the empty
        // name, and a Vault the Layout tracks is what that name is for.
        let reached = self
            .common
            .vault(&workspace)
            .ok()
            .filter(|reached| !reached.name.is_empty());
        let tracked = workspace.layout_track().ok().flatten();
        let vault = reached
            .as_ref()
            .map(|reached| reached.name.as_str())
            .or(tracked.as_deref());
        let reach = reached
            .as_ref()
            .map(|reached| (reached.name.clone(), reached.address.clone()));
        // Candidates and the account are read tolerantly: every Vault the Workspace bound is what
        // content may come from, and a run that needs none of them never asks.
        let candidates = self.common.candidates(&workspace).unwrap_or_default();
        let account = self.common.account(&workspace).ok();
        let store = self.common.store(&workspace).ok();
        // Only a tree is read by the ignore rules: the record of a file is not what they govern, and
        // loading them for a path no tree holds is a reading paid for and not used.
        let rules = if tree_above(&layout, &path).is_some() {
            tree_rules(&workspace)
        } else {
            IgnoreRuleSet::empty()
        };
        let messages = RetrackMessages::default();
        let request = Request {
            workspace: &workspace,
            layout: &layout,
            index: index.as_ref(),
            store: store.as_ref(),
            account: account.as_ref(),
            vault,
            reach: reach.as_ref(),
            candidates: &candidates,
            offline: self.common.offline,
            concurrency: self.common.concurrency,
            rules,
            messages: &messages,
        };
        let options = RetrackOptions {
            fork: self.fork,
            only_index: self.only_index,
            force: self.common.force,
            deny_jump: self.deny_jump,
        };

        retrack_path(&request, &path, &recording, options, progress, cancellation)
            .map_err(|refusal| refused(refusal, &self.record))
    }
}

/// The versions the index holds, for reading a short hash into the one it names.
///
/// A missing or unreadable index is an empty list: a short hash names a version only beside the
/// versions that are here, and a run that goes on to need the index is told it is missing by the
/// step that reads it rather than being refused the target it could have read.
fn version_keys(index: Option<&VCSIndex>) -> Vec<Key> {
    let Some(index) = index else {
        return Vec::new();
    };
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return Vec::new();
    };
    let Ok(objects) = runtime.block_on(index.read_objects()) else {
        return Vec::new();
    };

    objects
        .into_iter()
        .filter_map(|(key, object)| object.is_version().then_some(key))
        .collect()
}

/// The rules a tree is read by, which are the Workspace's own.
fn tree_rules(workspace: &Workspace) -> IgnoreRuleSet {
    IgnoreRuleSet::load(
        &workspace.get_root().join(WORKSPACE_IGNORE_RULES_DIR),
        workspace.get_root(),
    )
    .unwrap_or_else(|_| IgnoreRuleSet::empty())
}

/// The failure a refusal the run may not go past is told as.
///
/// The categories are the ones a caller acts on: a resource the run needed and did not have keeps
/// its own [`WriteError`], an exchange keeps [`WriteError::Action`], and what the run refused because
/// of where the path or the target stands is a [`WriteError::Precondition`] or an
/// [`WriteError::InvalidPath`], with the refusal's words as the reason.
fn refused(refusal: RetrackRefusal, record: &str) -> WriteError {
    let target = if record.is_empty() {
        "the version one step back".to_owned()
    } else {
        format!("the target `{record}`")
    };

    match refusal {
        RetrackRefusal::NotRecorded { path } => WriteError::InvalidPath {
            path,
            reason: "the Layout records no entry there".to_owned(),
        },
        RetrackRefusal::Nothing { path } => WriteError::Precondition {
            what: path,
            reason: "the path records no version".to_owned(),
        },
        RetrackRefusal::Modified { path } => WriteError::Precondition {
            what: path,
            reason: "the file has changes never recorded".to_owned(),
        },
        RetrackRefusal::Merging { path, variant } => WriteError::Precondition {
            what: path,
            reason: format!("the variant {variant} is waiting"),
        },
        RetrackRefusal::TreeDirty { path } => WriteError::Precondition {
            what: path,
            reason: "the tree holds work never recorded".to_owned(),
        },
        RetrackRefusal::Jump { path } => WriteError::Precondition {
            what: path,
            reason: "the target is on another chain".to_owned(),
        },
        RetrackRefusal::Target(kind) => WriteError::Precondition {
            what: target,
            reason: kind.to_string(),
        },
        RetrackRefusal::Entry(kind) => WriteError::Precondition {
            what: target,
            reason: kind.to_string(),
        },
        RetrackRefusal::Action(error) => WriteError::Action {
            reason: error.to_string(),
        },
        RetrackRefusal::Failed(error) => error,
    }
}

/// Retracks one path of a Workspace from a JSON request.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, or a `RolaWriteError *` the caller
/// releases with `free_rola_write_error`. A path is answered as
/// `{"kind":"path","path":"art/a.psd","version":"0f0f0f","number":3,"changed":true,
/// "switched":true,"from":null,"to":null,"forked":false,"took":[["origin",2]]}`, and a tree as
/// `{"kind":"tree","path":"art","version":"0f0f0f","took":[]}`. `version` is the digest in hex, and
/// `number` is the version's number in its chain, or null when the run does not know it; `from` and
/// `to` are the `Uuid`s a run that recorded another entry moved between, or null; and `took` names
/// each Vault that handed content over and how many objects it brought. `progress` is the handle the
/// caller polls with `rola_progress_next_json`, and `cancellation` is the flag it sets with
/// `rola_cancellation_cancel`.
///
/// `request` is the JSON of a [`RetrackRequest`]: its flattened `common` fields at the top level,
/// `file` and `record` as strings, and `only_index`, `deny_jump` and `fork` as bools. `file` is a
/// path in the Workspace, relative to its root or absolute. An empty or missing `record` moves one
/// version back.
///
/// # Errors
///
/// Returns [`WriteError::Json`] when `request` does not read as a [`RetrackRequest`] or the outcome
/// cannot be written, and the failures [`RetrackRequest::run`] names.
#[lazyffi(export = rola_write_retrack)]
pub fn retrack_at(
    request: &str,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<String, WriteError> {
    let request: RetrackRequest =
        serde_json::from_str(request).map_err(|error| WriteError::Json {
            reason: error.to_string(),
        })?;
    let watched = watching(progress);
    let outcome = request.run(&watched, cancellation)?;

    serde_json::to_string(&outcome).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// What a run asked the path to record.
#[derive(Debug, Clone)]
pub enum Recording {
    /// A version the entry is to be at, which moves its pointer.
    Version(VersionTarget),
    /// Another entry the path is to record, in place of the one it recorded.
    Entry(EntryTarget),
    /// A fresh `Uuid`, with what the path records now.
    Fork,
}

/// Where a version target moves the pointer to.
#[derive(Debug, Clone)]
pub enum VersionTarget {
    /// N versions back.
    Steps(u64),
    /// The version numbered N.
    Number(u64),
    /// The version a hash names.
    Hash(Key),
}

/// What an entry target names.
#[derive(Debug, Clone)]
pub enum EntryTarget {
    /// A `Uuid`, which needs no path to name an entry.
    Uuid(Uuid),
    /// A path of the Vault's own Layout.
    Upstream(LayoutPath),
}

/// The words a run that retracks says a failure in.
///
/// The library does not know the caller's language, so a caller fills these from its own
/// translations. Each carries `{}` where a value goes, filled left to right.
#[derive(Debug, Clone)]
pub struct RetrackMessages {
    /// An object the index holds that is not a version: `{}` is its hash.
    pub not_a_version: String,
    /// An object that is not the variant of its version: `{}` is its hash.
    pub not_a_variant: String,
    /// A hash nothing is stored under: `{}` is the hash.
    pub unstored: String,
    /// A Vault's Layout a fetch did not leave behind: `{}` is the Vault's name.
    pub copy_missing: String,
    /// An entry that is a file where the path is a tree: the first `{}` is the target, the second
    /// the path.
    pub kind_mismatch: String,
    /// A structure a step needed and did not read: `{}` is `target` or `recorded`.
    pub structure_missing: String,
}

impl Default for RetrackMessages {
    fn default() -> Self {
        Self {
            not_a_version: "the object {} is not a version".to_owned(),
            not_a_variant: "the object {} is not the variant of the version".to_owned(),
            unstored: "nothing is stored under the hash {}".to_owned(),
            copy_missing: "the copy of `{}`'s Layout was fetched and is not here".to_owned(),
            kind_mismatch: "`{}` names a file, and `{}` is a tree".to_owned(),
            structure_missing: "the {} structure was not read".to_owned(),
        }
    }
}

/// Fills the placeholders of `template` with `values`, left to right.
fn told(template: impl AsRef<str>, values: &[&str]) -> String {
    let mut said = template.as_ref().to_owned();

    for value in values {
        said = said.replacen("{}", value, 1);
    }

    said
}

/// What a retrack works over: the Workspace, the Layout, the index and the Vaults it may ask.
///
/// Everything that is a choice of the run's own is here rather than read from a configuration,
/// since how a Workspace reaches a Vault and which account it acts as are the caller's to resolve:
/// each of those failures has a category and an exit code of its own that a library cannot speak.
pub struct Request<'a> {
    /// The Workspace being worked in.
    pub workspace: &'a Workspace,
    /// The Layout being worked in.
    pub layout: &'a Layout,
    /// The index the versions are read from.
    pub index: Option<&'a VCSIndex>,
    /// The store the content is read from.
    pub store: Option<&'a StorageSet>,
    /// The account the run acts as, when it has one.
    pub account: Option<&'a Account>,
    /// The Vault the Layout tracks, as the Layout names it.
    pub vault: Option<&'a str>,
    /// Where that Vault answers, when the caller could resolve it.
    pub reach: Option<&'a (String, VaultAddress)>,
    /// Every Vault the run may bring content from, the one it named first.
    pub candidates: &'a [(String, VaultAddress)],
    /// Whether the run may reach a Vault at all.
    pub offline: bool,
    /// How many units of content a transfer carries at a time.
    pub concurrency: usize,
    /// The rules a tree is read by.
    pub rules: IgnoreRuleSet,
    /// The words a failure is said in.
    pub messages: &'a RetrackMessages,
}

/// What a retrack came to.
#[derive(Debug, Clone, Serialize)]
pub struct RetrackOutcome {
    /// What the path records now.
    #[serde(flatten)]
    pub record: Recorded,
    /// What each Vault handed over while the run went on, in the order they were asked.
    pub took: Vec<(String, usize)>,
}

/// What a retrack moved.
///
/// The `kind` an outcome is written with tells a path's record from a tree's: the two carry
/// different fields, and a caller that acts on one has to know which it was given.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recorded {
    /// The record of one path, and its work with it.
    Path(RetrackedPath),
    /// The record of a tree, and its work put back from the version.
    Tree(RetrackedTree),
}

/// What moving a path's record came to.
#[derive(Debug, Clone, Serialize)]
pub struct RetrackedPath {
    /// Where the entry sits in the Layout.
    pub path: String,
    /// The version it records now, as hex.
    pub version: String,
    /// That version's number in its chain, when it is known.
    pub number: Option<u64>,
    /// Whether what the path records is not what it recorded before.
    pub changed: bool,
    /// Whether the version's content was written back over the file.
    pub switched: bool,
    /// The `Uuid` the path recorded before, when the run recorded an entry.
    pub from: Option<String>,
    /// The `Uuid` the path records now, when the run recorded an entry.
    pub to: Option<String>,
    /// Whether the run gave the entry a fresh `Uuid` rather than recording another entry's.
    pub forked: bool,
}

/// What moving a tree's record came to.
#[derive(Debug, Clone, Serialize)]
pub struct RetrackedTree {
    /// The tree as a person knows it.
    pub path: String,
    /// The version it records now, as hex.
    pub version: String,
}

/// Why a retrack did not finish.
#[derive(Debug)]
pub enum RetrackRefusal {
    /// The Layout names nothing at the path, or holds no entry for what it names.
    NotRecorded {
        /// Where the entry was looked for.
        path: String,
    },
    /// The entry records no version, so there is nowhere to go.
    Nothing {
        /// Where the entry sits in the Layout.
        path: String,
    },
    /// The file has changes the Layout does not name.
    Modified {
        /// Where the file sits in the Layout.
        path: String,
    },
    /// A variant is waiting to be joined into the file.
    Merging {
        /// Where the file sits in the Layout.
        path: String,
        /// The variant that is waiting, as hex.
        variant: String,
    },
    /// A tree holds work that was never recorded.
    TreeDirty {
        /// The tree.
        path: String,
    },
    /// The two versions are on different chains, and the run refused the jump.
    Jump {
        /// Where the entry sits in the Layout.
        path: String,
    },
    /// The target names no version the run can move to.
    Target(TargetRefusal),
    /// The target names no entry the run can record.
    Entry(EntryRefusal),
    /// The exchange with a Vault failed.
    Action(ActionError),
    /// The write stopped: it was cancelled, or a store, an index, a Layout or a step of the run
    /// refused in a way that is no refusal of the run's own.
    Failed(WriteError),
}

impl fmt::Display for RetrackRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRecorded { path } => {
                write!(formatter, "the Layout records no entry at {path}")
            }
            Self::Nothing { path } => write!(formatter, "{path} was never recorded"),
            Self::Modified { path } => write!(formatter, "{path} has changes never recorded"),
            Self::Merging { path, variant } => {
                write!(formatter, "{path} has the variant {variant} waiting")
            }
            Self::TreeDirty { path } => write!(formatter, "{path} holds work never recorded"),
            Self::Jump { path } => write!(formatter, "{path} would jump to another chain"),
            Self::Target(refusal) => write!(formatter, "{refusal}"),
            Self::Entry(refusal) => write!(formatter, "{refusal}"),
            Self::Action(error) => write!(formatter, "{error}"),
            Self::Failed(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for RetrackRefusal {}

impl From<WriteError> for RetrackRefusal {
    fn from(error: WriteError) -> Self {
        Self::Failed(error)
    }
}

/// The ways a target names no version the run can move to.
#[derive(Debug, Clone)]
pub enum TargetRefusal {
    /// It is no target at all.
    NotATarget,
    /// It is a step count written with a dash, which the command line reads as a flag.
    DashNumber,
    /// It is a step or a number past an end of the chain the entry is the top of.
    OutOfChain {
        /// Where the entry sits in the Layout.
        path: String,
    },
    /// The hash names no version in the index.
    NotAVersion,
    /// The hash names more than one version in the index.
    AmbiguousHash {
        /// The versions it names, as hex.
        candidates: Vec<String>,
    },
    /// The hash names a version that is not on the way back from where the entry is.
    NotAncestor {
        /// Where the entry sits in the Layout.
        path: String,
    },
}

impl fmt::Display for TargetRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotATarget => formatter.write_str("the target is not something a path records"),
            Self::DashNumber => {
                formatter.write_str("a step count written with a dash is read as a flag")
            }
            Self::OutOfChain { path } => {
                write!(formatter, "the target is outside the chain {path} is at")
            }
            Self::NotAVersion => formatter.write_str("the hash names no version"),
            Self::AmbiguousHash { candidates } => {
                write!(formatter, "the hash names {}", candidates.join(", "))
            }
            Self::NotAncestor { path } => {
                write!(formatter, "the target is not on the way back from {path}")
            }
        }
    }
}

/// The ways a target names no entry the run can record.
#[derive(Debug, Clone)]
pub enum EntryRefusal {
    /// The target is a path, and the Layout tracks no Vault.
    NoVaultTracked,
    /// The Vault's Layout names no such path.
    NotUpstream {
        /// The Vault whose Layout names none.
        vault: String,
    },
    /// Neither this Layout nor the copy of the Vault's Layout holds the `Uuid`.
    UnknownUuid {
        /// The `Uuid` nothing holds.
        uuid: String,
    },
    /// The `Uuid` is the one the path records already.
    SameUuid {
        /// The `Uuid` the path records.
        uuid: String,
    },
    /// The `Uuid` is recorded under another path.
    Named {
        /// The `Uuid` another path records.
        uuid: String,
        /// The path that records it.
        path: String,
    },
    /// The entry records no version, so there is nothing to switch to.
    NoVersion {
        /// The `Uuid` whose entry records no version.
        uuid: String,
    },
}

impl fmt::Display for EntryRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVaultTracked => formatter.write_str("the Layout tracks no Vault"),
            Self::NotUpstream { vault } => write!(formatter, "{vault} names no such path"),
            Self::UnknownUuid { uuid } => write!(formatter, "nothing holds {uuid}"),
            Self::SameUuid { uuid } => write!(formatter, "the path records {uuid} already"),
            Self::Named { uuid, path } => write!(formatter, "{uuid} is recorded under {path}"),
            Self::NoVersion { uuid } => write!(formatter, "{uuid} records no version"),
        }
    }
}

/// Moves what a path records to a version, another entry or a fresh `Uuid`, and its work with it.
///
/// A path inside a tree names the tree, since a tree is one Layout entry rather than content at a
/// path. What a run may write is read before anything is written, and every step that reads a long
/// chain or writes many files checks `cancellation` between its steps.
///
/// # Errors
///
/// Returns [`RetrackRefusal`] when the run may not move the record, [`RetrackRefusal::Action`] when an exchange
/// with a Vault fails, and [`WriteError::Cancelled`] inside [`RetrackRefusal::Failed`] when `cancellation`
/// was set.
pub fn retrack_path(
    request: &Request<'_>,
    path: &LayoutPath,
    recording: &Recording,
    options: RetrackOptions,
    progress: &Progress,
    cancellation: &Cancellation,
) -> Result<RetrackOutcome, RetrackRefusal> {
    if cancellation.is_cancelled() {
        return Err(RetrackRefusal::Failed(WriteError::Cancelled));
    }

    // A tree is one Layout entry, so a path inside one names the tree's record: a retrack of a file
    // the tree holds is a retrack of the tree, which is the entry the Layout actually has.
    let path = tree_above(request.layout, path).unwrap_or_else(|| path.clone());
    let named = path.as_str().to_owned();
    let Some(id) = request.layout.id_of(&path) else {
        return Err(RetrackRefusal::NotRecorded { path: named });
    };
    let Some(data) = request.layout.entry(id) else {
        return Err(RetrackRefusal::NotRecorded { path: named });
    };

    // The root version is what an entry is at before anything was recorded for it, so it is no
    // version to go back to: a file the Layout names this way has no record yet.
    if data.version() == [0; 32] {
        return Err(RetrackRefusal::Nothing { path: named });
    }

    let Some(index) = request.index else {
        return Err(RetrackRefusal::Failed(WriteError::NoIndex));
    };
    let runtime = tokio::runtime::Runtime::new().map_err(|error| internal(error.to_string()))?;
    let run = Run {
        workspace: request.workspace,
        layout: request.layout,
        index,
        store: request.store,
        account: request.account,
        vault: request.vault,
        reach: request.reach,
        candidates: request.candidates,
        offline: request.offline,
        concurrency: request.concurrency,
        rules: &request.rules,
        messages: request.messages,
        runtime: &runtime,
        progress,
        cancellation,
        took: RefCell::new(Vec::new()),
    };

    // A tree is one version unit rather than content at a path: what moves back is the whole
    // structure, and what the work holds is put back from it rather than from a content object.
    if data.kind() == LayoutEntryKind::Tree {
        let Some(store) = run.store else {
            return Err(RetrackRefusal::Failed(WriteError::NoStore));
        };

        return retrack_tree(&run, store, &path, id, &data, recording, options);
    }

    match recording {
        Recording::Version(level) => record_version(&run, &path, id, &data, level, options),
        Recording::Entry(entry) => switch_uuid(&run, &path, id, &data, Some(entry), options),
        Recording::Fork => switch_uuid(&run, &path, id, &data, None, options),
    }
}

/// The request as the steps of one run read it, with the runtime, the watching and what has been
/// brought beside it.
struct Run<'a> {
    /// The Workspace being worked in.
    workspace: &'a Workspace,
    /// The Layout being worked in.
    layout: &'a Layout,
    /// The index the versions are read from.
    index: &'a VCSIndex,
    /// The store the content is read from.
    store: Option<&'a StorageSet>,
    /// The account the run acts as, when it has one.
    account: Option<&'a Account>,
    /// The Vault the Layout tracks, as the Layout names it.
    vault: Option<&'a str>,
    /// Where that Vault answers, when the caller could resolve it.
    reach: Option<&'a (String, VaultAddress)>,
    /// Every Vault the run may bring content from, the one it named first.
    candidates: &'a [(String, VaultAddress)],
    /// Whether the run may reach a Vault at all.
    offline: bool,
    /// How many units of content a transfer carries at a time.
    concurrency: usize,
    /// The rules a tree is read by.
    rules: &'a IgnoreRuleSet,
    /// The words a failure is said in.
    messages: &'a RetrackMessages,
    /// The runtime the asynchronous exchanges are waited on.
    runtime: &'a tokio::runtime::Runtime,
    /// What the exchanges say about themselves while they run.
    progress: &'a Progress,
    /// Whether the caller has asked the run to stop.
    cancellation: &'a Cancellation,
    /// What each Vault handed over, gathered for the caller to say once the run is done.
    took: RefCell<Vec<(String, usize)>>,
}

impl Run<'_> {
    /// Takes in what a set of sources brought, so the caller can say it.
    ///
    /// A fetch a run did not make is one it does not report, and what a set of sources did not
    /// bring is left out of what it answers with, so this is nothing when nothing crossed.
    fn absorb(&self, sources: &Sources<'_>) {
        self.took.borrow_mut().extend(sources.took());
    }

    /// What the Vaults handed over, taking it out of the run.
    fn took(&self) -> Vec<(String, usize)> {
        self.took.take()
    }

    /// Whether the caller has asked the run to stop, as the one answer every loop reads.
    fn stopped(&self) -> Result<(), RetrackRefusal> {
        if self.cancellation.is_cancelled() {
            return Err(RetrackRefusal::Failed(WriteError::Cancelled));
        }

        Ok(())
    }
}

/// What a run is to do to the file on disk, once the reading says it may be acted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Move the pointer and leave the file alone.
    Pointer,
    /// Write the version's content back over the file, the pointer moving with it.
    Content,
}

/// Moves the version pointer of the entry the path names to the version `level` names.
fn record_version(
    run: &Run<'_>,
    path: &LayoutPath,
    id: Uuid,
    data: &MutableData,
    level: &VersionTarget,
    options: RetrackOptions,
) -> Result<RetrackOutcome, RetrackRefusal> {
    let here = read_version(run, data.version())?;
    let chain = ancestors(run, &here)?;

    // The version the pointer goes to may be one this index never fetched. It is checked here
    // before the target is resolved, so that a hash the Vault holds is a version to go to rather
    // than one this side cannot name, and so that the chain left behind is one that can be read.
    if let VersionTarget::Hash(key) = level {
        match presence(run, *key.digest()) {
            Presence::Version => {}
            Presence::Elsewhere => {
                return Err(RetrackRefusal::Target(TargetRefusal::NotAVersion));
            }
            Presence::Missing => {
                // Nothing the run asked for is here, and an offline run may not go looking for it.
                if run.offline {
                    return Err(RetrackRefusal::Failed(WriteError::Offline));
                }

                bring(run, *key.digest())?;
            }
        }
    }

    let asked = Asked {
        level,
        path: path.as_str(),
        deny_jump: options.deny_jump,
    };
    let (target, number) = resolve_target(run, &chain, &asked)?;

    let current = data.version();
    let moved = target != current;

    // What the run is to do to the file is worked out before anything is written, so a run that is
    // refused leaves the Layout, the tree and the merge exactly as they were.
    let mut merging = Merging::read(run.workspace.get_root());
    let pending = merging.at(id).cloned();
    // Retracking moves a version pointer of an entry the Layout names, which the ignore rules have
    // no jurisdiction over, so the reading is taken unfiltered.
    let diff = tree_diff_remembering(
        run.layout,
        run.workspace,
        DEFAULT_ALIKE,
        &IgnoreRuleSet::empty(),
    )
    .map_err(|error| internal(error.to_string()))?;

    let action = match &pending {
        // A variant waiting for the file is a merge that was never recorded, and a pointer moved
        // under it would leave the merge with a file it does not match. `only_index` cannot end one,
        // so it is refused rather than answered with a merge still in progress.
        Some(pending) if options.only_index || !options.force => {
            return Err(RetrackRefusal::Merging {
                path: path.as_str().to_owned(),
                variant: hex(*pending.variant()),
            });
        }
        None if options.only_index => Action::Pointer,
        // Changes the Layout does not name are the run's to discard or to leave alone, and they are
        // not something a retrack may decide by itself.
        None if !options.force && diff.modified.contains(path) => {
            return Err(RetrackRefusal::Modified {
                path: path.as_str().to_owned(),
            });
        }
        _ => Action::Content,
    };

    if moved {
        let owner = data.owner().map(str::to_owned);

        if let Err(error) = run.layout.update_entry(id, data.renewed(owner, target)) {
            return Err(RetrackRefusal::Failed(WriteError::Layout {
                reason: error.to_string(),
            }));
        }
    }

    let switched = match action {
        Action::Pointer => {
            // What the file is to agree with now, which is the content the version moved to holds.
            // One the index does not hold is nothing to read, and the agreement is then taken back.
            let agreed = read_version(run, target)
                .ok()
                .and_then(|version| stored_hash(run, &version).ok());

            disagree(run, path, agreed)?;

            false
        }
        Action::Content => {
            write_version_over(run, path, target)?;

            // Only a forced run reaches here with a merge waiting, and forcing it is what ends it:
            // the variant file is the content that was just replaced, so the record and the file go
            // with it.
            if let Some(pending) = &pending {
                let disk = variant_disk(run, &merging, &diff.untagged, pending);

                end_merge(&mut merging, run.workspace.get_root(), id, disk)?;
            }

            true
        }
    };

    Ok(RetrackOutcome {
        record: Recorded::Path(RetrackedPath {
            path: path.as_str().to_owned(),
            version: hex(target),
            number,
            changed: moved,
            switched,
            from: None,
            to: None,
            forked: false,
        }),
        took: run.took(),
    })
}

/// Records another `Uuid` for the path, from a target that names an entry or from a fork.
///
/// What the path records is dropped and the entry it is to record takes its place, in one batch: an
/// entry is its `Uuid`, so the two are one change and not a removal and a creation a reader could
/// meet between. Nothing else is touched, the index, the stores and the working tree know nothing of
/// a Layout's paths, except the file the run writes back or leaves alone.
///
/// What is read is read before anything is written, so a refused run leaves the Layout, the tree and
/// the merge as they were.
fn switch_uuid(
    run: &Run<'_>,
    path: &LayoutPath,
    old: Uuid,
    data: &MutableData,
    entry: Option<&EntryTarget>,
    options: RetrackOptions,
) -> Result<RetrackOutcome, RetrackRefusal> {
    let mut merging = Merging::read(run.workspace.get_root());
    let pending = merging.at(old).cloned();

    // A variant waiting for the file is a merge that was never recorded, and an entry recorded under
    // another `Uuid` under it would leave the merge with a file it does not match. `only_index`
    // cannot end one, so only `force` goes past this.
    if let Some(pending) = &pending
        && !options.force
    {
        return Err(RetrackRefusal::Merging {
            path: path.as_str().to_owned(),
            variant: hex(*pending.variant()),
        });
    }

    // What the path is to record, and what that entry holds: a fresh `Uuid` with what the path
    // records now, or the entry the target names.
    let (target, source) = match entry {
        None => (Uuid::new_v4(), data.clone()),
        Some(entry) => {
            let found = recorded_entry(run, entry, old)?;

            (found.id, found.data)
        }
    };

    // The file is left alone unless the versions say otherwise: an entry that records the same
    // version, or one ahead of what the target records, keeps the work on disk as it is. Whether the
    // file may be written at all is what the reading of it says below.
    let action = match entry {
        None => SwitchAction::Keep,
        Some(_) => match switch_action(
            stand_of(run, data, &source)?,
            options.only_index,
            options.deny_jump,
        ) {
            Ok(action) => action,
            Err(SwitchRefusal::NotOnChain) => {
                return Err(RetrackRefusal::Jump {
                    path: path.as_str().to_owned(),
                });
            }
        },
    };

    // What the new entry records: the version the path records now when that is what the file
    // holds, and the version the entry to record holds when the content is switched.
    let version = match action {
        SwitchAction::Keep => data.version(),
        SwitchAction::Pointer | SwitchAction::Content => source.version(),
    };
    let new_data = switched_data(&source, data, version);

    // Whether the file is to be written, and where the variant a merge is waiting for lies, are both
    // read from the tree as it stands before the Layout changes: a tree read after the change has no
    // entry naming the variant's file, so a move of one is not found again.
    let diff = if pending.is_some() || action == SwitchAction::Content {
        Some(
            tree_diff_remembering(
                run.layout,
                run.workspace,
                DEFAULT_ALIKE,
                &IgnoreRuleSet::empty(),
            )
            .map_err(|error| internal(error.to_string()))?,
        )
    } else {
        None
    };
    let pending_disk = match (&pending, &diff) {
        (Some(pending), Some(diff)) => variant_disk(run, &merging, &diff.untagged, pending),
        _ => None,
    };

    // Changes the Layout does not name are the run's to discard or to leave alone: a content switch
    // over them is what `force` is for, and what a run without it is refused for.
    if action == SwitchAction::Content
        && !options.force
        && diff
            .as_ref()
            .is_some_and(|diff| diff.modified.contains(path))
    {
        return Err(RetrackRefusal::Modified {
            path: path.as_str().to_owned(),
        });
    }

    // The Layout is written before the content, so a run whose store is short of it leaves a record
    // naming a version the file does not hold, which a reading reports as a change, and a second
    // retrack puts right.
    if let Err(error) = switch_entry(run.layout, path, target, new_data) {
        return Err(RetrackRefusal::Failed(error));
    }

    let switched = match action {
        SwitchAction::Keep => false,
        SwitchAction::Pointer => {
            // What the file is to agree with now: the content the version it records holds, when
            // that version can be read, and nothing at all when it cannot.
            let agreed = read_version(run, version)
                .ok()
                .and_then(|version| stored_hash(run, &version).ok());

            disagree(run, path, agreed)?;

            false
        }
        SwitchAction::Content => {
            write_version_over(run, path, version)?;

            true
        }
    };

    // Only a forced run reaches here with a merge waiting, and forcing it is what ends it: the merge
    // was never recorded, so its record goes and the variant file it named goes with it.
    if pending.is_some() {
        end_merge(&mut merging, run.workspace.get_root(), old, pending_disk)?;
    }

    let number = read_version(run, version)
        .ok()
        .and_then(|version| run.runtime.block_on(run.index.version_num(&version)).ok());

    Ok(RetrackOutcome {
        record: Recorded::Path(RetrackedPath {
            path: path.as_str().to_owned(),
            version: hex(version),
            number,
            changed: true,
            switched,
            from: Some(old.to_string()),
            to: Some(target.to_string()),
            forked: entry.is_none(),
        }),
        took: run.took(),
    })
}

/// Reads what the entry a target names is to be recorded as.
///
/// A `Uuid` needs no path and no fetch: one this Layout holds at no path is the entry to record, and
/// the copy of the Vault's Layout is read only when it is here already. A path is read from that
/// copy, fetched first when it is not here, the one step of this that may leave the machine.
fn recorded_entry(
    run: &Run<'_>,
    target: &EntryTarget,
    old: Uuid,
) -> Result<EntryFound, RetrackRefusal> {
    let (id, source) = match target {
        EntryTarget::Uuid(id) => {
            // The copy is used when it is here, and never fetched for a `Uuid`: a `Uuid` gives no
            // reason to reach anywhere, and the entry may be one this Layout holds at no path.
            let copy = match run.vault {
                Some(name) => open_copy(run, name)?,
                None => None,
            };
            let source = copy
                .as_ref()
                .and_then(|copy| copy.entry(*id))
                .or_else(|| run.layout.entry(*id));

            (*id, source)
        }
        EntryTarget::Upstream(path) => {
            let Some(name) = run.vault else {
                return Err(RetrackRefusal::Entry(EntryRefusal::NoVaultTracked));
            };

            let copy = if let Some(copy) = open_copy(run, name)? {
                copy
            } else {
                if run.offline {
                    return Err(RetrackRefusal::Failed(WriteError::Offline));
                }

                fetch_copy(run)?;

                match open_copy(run, name)? {
                    Some(copy) => copy,
                    None => {
                        return Err(internal(told(&run.messages.copy_missing, &[name])));
                    }
                }
            };

            let Some(id) = copy.id_of(path) else {
                return Err(RetrackRefusal::Entry(EntryRefusal::NotUpstream {
                    vault: name.to_owned(),
                }));
            };

            (id, copy.entry(id))
        }
    };

    // The entry the path records already is the one case every other check would be told apart
    // from: it is named by a path, this one, so it is refused for being the entry it is.
    if id == old {
        return Err(RetrackRefusal::Entry(EntryRefusal::SameUuid {
            uuid: id.to_string(),
        }));
    }
    if let Some(named) = run.layout.path_of(id) {
        return Err(RetrackRefusal::Entry(EntryRefusal::Named {
            uuid: id.to_string(),
            path: named.as_str().to_owned(),
        }));
    }

    let Some(data) = source else {
        return Err(RetrackRefusal::Entry(EntryRefusal::UnknownUuid {
            uuid: id.to_string(),
        }));
    };

    // The root version is what an entry is at before anything was recorded for it, so an entry that
    // records it records no version to switch to.
    if data.version() == [0; 32] {
        return Err(RetrackRefusal::Entry(EntryRefusal::NoVersion {
            uuid: id.to_string(),
        }));
    }

    Ok(EntryFound { id, data })
}

/// The `Uuid` a run that records an entry is to record, and what that entry holds.
struct EntryFound {
    /// The `Uuid` the path is to record.
    id: Uuid,
    /// What it holds, from the copy of the Vault's Layout that has it or from this Layout.
    data: MutableData,
}

/// Writes a version's content back over the file, bringing it from a Vault when it is not here.
///
/// The store is asked first, and a Vault is bound to only when it is short of the content: a
/// retrack of content this Workspace has is local work, and asking for an account would make it fail
/// where there is nothing to fetch.
fn write_version_over(
    run: &Run<'_>,
    path: &LayoutPath,
    version: Blake3Hash,
) -> Result<(), RetrackRefusal> {
    let recorded = content_of(run, version)?;
    let account = if holds(run, recorded) {
        None
    } else {
        Some(
            run.account
                .ok_or(RetrackRefusal::Failed(WriteError::NoAccount))?,
        )
    };
    let sources = match &account {
        Some(account) => Some(sources_of(run, account)?),
        None => None,
    };

    restore_content(run, path, recorded, sources.as_ref())?;

    if let Some(sources) = &sources {
        run.absorb(sources);
    }

    Ok(())
}

/// Writes the version's content back over the file, and remembers that the two agree.
///
/// `recorded` is the content the version is of, which is what the store holds it under. The store
/// may not hold it: what is not here is brought from a Vault first, which is the one step of a
/// retrack that may leave the machine, and `sources` is the run that may do it, nothing when the
/// content was already here and there is nothing to ask for.
fn restore_content(
    run: &Run<'_>,
    path: &LayoutPath,
    recorded: Blake3Hash,
    sources: Option<&Sources<'_>>,
) -> Result<(), RetrackRefusal> {
    let Some(store) = run.store else {
        return Err(RetrackRefusal::Failed(WriteError::NoStore));
    };

    let root = run.workspace.get_root();
    let disk = root.join(path.to_path_buf());

    if let Some(parent) = disk.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return Err(internal(error.to_string()));
    }

    if let Some(sources) = sources {
        sources.bring(store, &[Key::new(recorded)]);
    }

    if let Err(error) = run
        .runtime
        .block_on(store.extract_file(&Key::new(recorded), &disk))
    {
        return Err(RetrackRefusal::Failed(WriteError::Storage {
            reason: error.to_string(),
        }));
    }

    // What was written is what the Layout names again, so the reading has to be told: a path left
    // marked as disagreed would be reported as changed by the very next reading.
    remember(run.layout, root, path.as_str())?;

    Ok(())
}

/// What changing the `Uuid` a path records does to the file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwitchAction {
    /// Leave the file alone: it still holds what the entry it records holds.
    Keep,
    /// Record the entry and leave the file alone, the reading told the two disagree.
    Pointer,
    /// Write the recorded version's content over the file.
    Content,
}

/// The ways the two versions standing apart refuse a change of `Uuid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwitchRefusal {
    /// The two are on different chains, and the run asked to stay on the chain.
    NotOnChain,
}

/// What changing the `Uuid` a path records does, for how the two versions stand.
///
/// The two versions being on one chain is what decides how little is done: the file already holds
/// what the path records, so an entry that records the same version or a later one is a change of
/// identity and nothing more, and one that records an earlier version has that version's content
/// written over the file. Whether the file may be written is what the caller's reading of it decides:
/// a file that stands as recorded is written without being asked, and one holding changes that were
/// never recorded is what `force` is for. Different chains are not a step back at all, so they are
/// what a run does unless it asked to stay on the chain with `deny_jump`.
const fn switch_action(
    relation: Relation,
    only_index: bool,
    deny_jump: bool,
) -> Result<SwitchAction, SwitchRefusal> {
    match relation {
        Relation::Equal | Relation::Ahead => Ok(SwitchAction::Keep),
        Relation::Apart if deny_jump => Err(SwitchRefusal::NotOnChain),
        Relation::Behind | Relation::Apart if only_index => Ok(SwitchAction::Pointer),
        Relation::Behind | Relation::Apart => Ok(SwitchAction::Content),
    }
}

/// The data the entry a path comes to record is given.
///
/// Who holds it and what it says come from the entry being recorded, since that is the entry the
/// path comes to be. The cut it prefers stays the one the path's own entry preferred: how content is
/// written is about the file, not about the `Uuid` it is recorded under.
fn switched_data(source: &MutableData, old: &MutableData, version: [u8; 32]) -> MutableData {
    MutableData::new(
        source.owner().map(str::to_owned),
        version,
        source.description().to_owned(),
    )
    .with_preference(old.preference().map(str::to_owned))
}

/// Whether the store here already holds the content `recorded`.
///
/// A store that cannot answer is read as holding none of it, the way a fetch reads one: an asking
/// that turns out to be unnecessary costs a look-up, while the other way round leaves content that
/// is needed behind.
fn holds(run: &Run<'_>, recorded: Blake3Hash) -> bool {
    let Some(store) = run.store else {
        return false;
    };

    run.runtime
        .block_on(store.contains_keys(&[Key::new(recorded)]))
        .is_ok_and(|presence| presence.held(0))
}

/// The stored content the version `version` is of.
fn content_of(run: &Run<'_>, version: Blake3Hash) -> Result<Blake3Hash, RetrackRefusal> {
    let version = read_version(run, version)?;

    stored_hash(run, &version)
}

/// Ends the merge waiting for `target`: the record goes, and the variant file with it.
///
/// The record is written out before the file is removed, since a record that outlived its variant
/// file is a merge a reading reports as broken, while a variant file left by a write that failed is
/// only a path the tree holds untagged, and the target's content is replaced by the caller.
///
/// `disk` is where the variant file was found before the caller changed anything, read before the
/// change because a change of `Uuid` takes away the entry the merge names its file by.
fn end_merge(
    merging: &mut Merging,
    root: &Path,
    target: Uuid,
    disk: Option<PathBuf>,
) -> Result<(), RetrackRefusal> {
    merging.remove(target);
    if let Err(error) = merging.write(root) {
        return Err(internal(error.to_string()));
    }

    // A variant file that is already gone is one the merge can still be ended from: what was brought
    // in is the content the record names, and the file was only how it was shown.
    if let Some(disk) = disk
        && disk.is_file()
        && let Err(error) = fs::remove_file(&disk)
    {
        return Err(internal(error.to_string()));
    }

    Ok(())
}

/// Where the variant file waiting for `pending` lies, when the tree still holds it.
///
/// The record names the place under the convention, but a run that moved the file with the
/// filesystem rather than with `rola fs-ops mv` leaves it elsewhere; it is found again by the
/// content it holds, the way a reading of the tree finds it.
fn variant_disk(
    run: &Run<'_>,
    merging: &Merging,
    untagged: &[LayoutPath],
    pending: &Pending,
) -> Option<PathBuf> {
    let root = run.workspace.get_root();
    let from = merging.variant_path(run.layout, pending)?;
    let disk = root.join(from.to_path_buf());
    if disk.is_file() {
        return Some(disk);
    }

    variant_moves(
        merging,
        run.layout,
        root,
        Some(run.index),
        run.runtime,
        untagged,
    )
    .into_iter()
    .find(|moved| moved.target == pending.target())
    .map(|moved| root.join(moved.to.to_path_buf()))
}

/// What a run asked the pointer to be moved to.
struct Asked<'a> {
    /// Where the pointer is to go.
    level: &'a VersionTarget,
    /// The path the entry is at, for the words a refusal uses.
    path: &'a str,
    /// Whether a version off the way back is refused.
    deny_jump: bool,
}

/// The version the pointer is to be moved to, and its number when it is known.
///
/// A step count and a number are worked out from the chain the entry is the top of; a hash is that
/// chain when it names a version on it, and a version of the index otherwise, which is a jump the
/// run makes unless it asked to stay on the chain.
fn resolve_target(
    run: &Run<'_>,
    chain: &[Version],
    asked: &Asked<'_>,
) -> Result<(Blake3Hash, Option<u64>), RetrackRefusal> {
    let out_of_chain = || {
        RetrackRefusal::Target(TargetRefusal::OutOfChain {
            path: asked.path.to_owned(),
        })
    };
    // A version with nothing behind it has no chain to count from, which is the same answer as a
    // step past the end: there is nothing there to move to.
    let Some(top) = chain.len().checked_sub(1) else {
        return Err(out_of_chain());
    };

    match asked.level {
        VersionTarget::Steps(back) => {
            let Ok(back) = usize::try_from(*back) else {
                return Err(out_of_chain());
            };
            let Some(version) = chain.get(back) else {
                return Err(out_of_chain());
            };

            Ok((*version.hash().digest(), Some(number_at(top, back))))
        }
        VersionTarget::Number(number) => {
            let Ok(number) = usize::try_from(*number) else {
                return Err(out_of_chain());
            };
            if number > top {
                return Err(out_of_chain());
            }

            let at = top - number;
            Ok((
                *chain[at].hash().digest(),
                Some(u64::try_from(number).unwrap_or(u64::MAX)),
            ))
        }
        VersionTarget::Hash(key) => {
            if let Some((at, version)) = chain
                .iter()
                .enumerate()
                .find(|(_, version)| version.hash().digest() == key.digest())
            {
                return Ok((*version.hash().digest(), Some(number_at(top, at))));
            }

            let Ok(version) = read_version(run, *key.digest()) else {
                return Err(RetrackRefusal::Target(TargetRefusal::NotAVersion));
            };

            if asked.deny_jump {
                return Err(RetrackRefusal::Target(TargetRefusal::NotAncestor {
                    path: asked.path.to_owned(),
                }));
            }

            let number = run.runtime.block_on(run.index.version_num(&version)).ok();

            Ok((*key.digest(), number))
        }
    }
}

/// The copy of `vault`'s own Layout the Workspace keeps, opened, or nothing when it was not fetched.
///
/// A copy that is not here is nothing rather than a failure: a run that has not fetched reads no
/// copy here, and whether one is fetched is the caller's to say.
fn open_copy(run: &Run<'_>, vault: &str) -> Result<Option<Layout>, RetrackRefusal> {
    let Some(dir) = run.workspace.readonly_layout_copy(vault, VAULT_LAYOUT_NAME) else {
        return Ok(None);
    };

    // A directory the Layout's own files are not in is not a copy, and reading it must not make one.
    match Layout::open_existing(&dir) {
        Ok(copy) => Ok(Some(copy)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(RetrackRefusal::Failed(WriteError::Layout {
            reason: error.to_string(),
        })),
    }
}

/// Fetches `vault`'s own Layout into the read-only copy the Workspace keeps.
fn fetch_copy(run: &Run<'_>) -> Result<(), RetrackRefusal> {
    if run.vault.is_none() {
        return Err(RetrackRefusal::Failed(WriteError::NoTrack));
    }
    let Some((name, address)) = run.reach else {
        return Err(RetrackRefusal::Failed(WriteError::NoVault));
    };
    let Some(account) = run.account else {
        return Err(RetrackRefusal::Failed(WriteError::NoAccount));
    };

    action_fetch_layout(run.workspace, account, address.to_string(), name.clone())
        .map_err(RetrackRefusal::Action)
}

/// Whether this index holds the version under a hash, holds something else there, or holds nothing.
enum Presence {
    /// The version is here, so a run that goes to it needs nothing brought.
    Version,
    /// The index holds an object under the hash, and it is not a version.
    Elsewhere,
    /// The index holds nothing under the hash.
    Missing,
}

/// Where a version this index does not hold is read from, and as whom.
struct VaultEnd<'a> {
    /// The address the Vault is reached at.
    address: String,
    /// The account the read is spoken as.
    account: &'a Account,
}

/// Reads the chain a version is the top of into this index, from the Vault that holds it.
///
/// A retrack may go to a version the Vault holds and this side never fetched. The chain under it,
/// the version, the variant that produced it, the words they are named by, and so on down to a
/// version this index already holds, is asked for in one exchange, and what the Vault walks is
/// written here as one pack: what is left behind is a chain as readable here as one a sync brought.
fn bring(run: &Run<'_>, hash: Blake3Hash) -> Result<(), RetrackRefusal> {
    let end = vault_end(run)?;
    let said = run
        .runtime
        .block_on(action_bring_remote_index_async(
            run.workspace,
            end.account,
            end.address,
            Key::new(hash).hex(),
            run.progress.clone(),
        ))
        .map_err(RetrackRefusal::Action)?;

    let chain: Chain = serde_json::from_str(&said).map_err(|error| internal(error.to_string()))?;

    match chain {
        Chain::Brought => Ok(()),
        Chain::NotAVersion => Err(RetrackRefusal::Target(TargetRefusal::NotAVersion)),
        Chain::NotAVariant(key) => Err(internal(told(&run.messages.not_a_variant, &[&key]))),
        Chain::Missing(key) => Err(internal(told(&run.messages.unstored, &[&key]))),
    }
}

/// Whether this index holds a version under `hash`.
fn presence(run: &Run<'_>, hash: Blake3Hash) -> Presence {
    match run.runtime.block_on(run.index.read(Key::new(hash))) {
        Ok(VCSIndexObject::Version(_)) => Presence::Version,
        Ok(_) => Presence::Elsewhere,
        Err(_) => Presence::Missing,
    }
}

/// Reads where a version missing here is brought from, and as whom.
///
/// A hash names no Vault, so the Vault the Layout tracks is the one asked.
fn vault_end<'a>(run: &Run<'a>) -> Result<VaultEnd<'a>, RetrackRefusal> {
    if run.vault.is_none() {
        return Err(RetrackRefusal::Failed(WriteError::NoTrack));
    }
    let Some((_, address)) = run.reach else {
        return Err(RetrackRefusal::Failed(WriteError::NoVault));
    };
    let Some(account) = run.account else {
        return Err(RetrackRefusal::Failed(WriteError::NoAccount));
    };

    Ok(VaultEnd {
        address: address.to_string(),
        account,
    })
}

/// The sources a run may bring content from, as itself: the Vaults it may ask and what it says.
fn sources_of<'a>(run: &Run<'a>, account: &'a Account) -> Result<Sources<'a>, RetrackRefusal> {
    let sources = Sources::new(run.workspace, account, run.candidates.to_vec(), run.offline)
        .map_err(internal)?;

    Ok(sources
        .reporting(run.progress.clone())
        .with_concurrency(run.concurrency))
}

/// Writes down what the file is to agree with now that the Layout names an older version.
///
/// Moving the pointer and leaving the file alone leaves the two apart, the file is what the newer
/// version held and the Layout names an older one, but the reading does not tell that from the
/// pointer: what it compares is the file against what the Layout was last known to agree with. So
/// the agreement is written afresh here rather than left standing, or the very next `rola status`
/// would call a file with changes never recorded one with nothing to record.
///
/// What is written is the content the new version holds, so a file that is then put back to it by
/// hand reads as agreed again, a plain "nothing" would leave such a file reported as changed with
/// `rola track` having nothing to say about it. The content of a version the index does not hold is
/// nothing that can be read, so the agreement is taken back rather than guessed at.
///
/// A path the tree does not hold is nothing to write down: it is read as lost however the cache
/// stands, and there is no file to read a stamp from.
fn disagree(
    run: &Run<'_>,
    path: &LayoutPath,
    agreed: Option<Blake3Hash>,
) -> Result<(), RetrackRefusal> {
    let root = run.workspace.get_root();
    let disk = root.join(path.to_path_buf());
    if !disk.is_file() {
        return Ok(());
    }

    let file = cache_path(root, run.layout);
    let mut cache = Cache::read(&file);
    let entry = entry_of(&disk).map_err(|error| internal(error.to_string()))?;

    cache.insert(path.clone(), entry.with_agreed(agreed));
    cache
        .write(&file)
        .map_err(|error| internal(error.to_string()))
}

/// The versions from `top` back to the first, the top first.
fn ancestors(run: &Run<'_>, top: &Version) -> Result<Vec<Version>, RetrackRefusal> {
    let mut chain = Vec::new();
    let mut current = top.clone();

    while !current.is_root() {
        run.stopped()?;

        let variant = read_variant(run, *current.variant())?;
        chain.push(current);
        current = read_version(run, *variant.base_version())?;
    }

    Ok(chain)
}

/// The number of the version `at` steps back from a chain whose top is numbered `top`.
fn number_at(top: usize, at: usize) -> u64 {
    u64::try_from(top.saturating_sub(at)).unwrap_or(u64::MAX)
}

/// The `Version` stored under `hash`.
fn read_version(run: &Run<'_>, hash: Blake3Hash) -> Result<Version, RetrackRefusal> {
    run.runtime
        .block_on(run.index.read(Key::new(hash)))
        .map_err(|error| {
            RetrackRefusal::Failed(WriteError::Index {
                reason: error.reason(),
            })
        })?
        .expect_version()
        .map_err(|error| {
            RetrackRefusal::Failed(WriteError::Index {
                reason: error.to_string(),
            })
        })
}

/// The `Variant` stored under `hash`.
fn read_variant(run: &Run<'_>, hash: Blake3Hash) -> Result<Variant, RetrackRefusal> {
    run.runtime
        .block_on(run.index.read(Key::new(hash)))
        .map_err(|error| {
            RetrackRefusal::Failed(WriteError::Index {
                reason: error.reason(),
            })
        })?
        .expect_variant()
        .map_err(|error| {
            RetrackRefusal::Failed(WriteError::Index {
                reason: error.to_string(),
            })
        })
}

/// The stored hash the variant of `version` points at.
fn stored_hash(run: &Run<'_>, version: &Version) -> Result<Blake3Hash, RetrackRefusal> {
    let variant = read_variant(run, *version.variant())?;

    Ok(*variant.storage_hash())
}

/// The tree whose Layout entry holds `path`, when one does.
///
/// It is the path itself when the path is a tree, and its nearest recorded ancestor otherwise: a file
/// inside a tree is not an entry of its own, and a retrack names the tree's record rather than a path
/// the Layout does not have.
#[must_use]
pub fn tree_above(layout: &Layout, path: &LayoutPath) -> Option<LayoutPath> {
    let parts: Vec<&str> = path.as_str().split('/').collect();

    for end in (1..=parts.len()).rev() {
        let Ok(candidate) = LayoutPath::new(&parts[..end].join("/")) else {
            continue;
        };
        let Some(id) = layout.id_of(&candidate) else {
            continue;
        };
        let Some(data) = layout.entry(id) else {
            continue;
        };

        if data.kind() == LayoutEntryKind::Tree {
            return Some(candidate);
        }
    }

    None
}

/// The stored hash `hash` is written as, in hex.
fn hex(hash: Blake3Hash) -> String {
    Key::new(hash).hex()
}

/// The answer a step that read something other than what it needed gives.
///
/// It is not a refusal of the run's own, and not a category a caller acts on: what reached here is
/// data that does not match what the step was told it would be, which is why the reason is the
/// message rather than a value the caller branches on.
fn internal(reason: impl Into<String>) -> RetrackRefusal {
    RetrackRefusal::Failed(WriteError::Internal {
        reason: reason.into(),
    })
}

/// Moves a tree's record to another version, and puts the work back from it.
///
/// What is put back is the target version's structure: a path it holds is written as it holds it,
/// and a path the tree recorded that the target does not hold is taken out of the work. What the
/// tree never recorded is left where it is, so a retrack does not reach beyond the version unit.
fn retrack_tree(
    run: &Run<'_>,
    store: &StorageSet,
    path: &LayoutPath,
    id: Uuid,
    data: &MutableData,
    recording: &Recording,
    options: RetrackOptions,
) -> Result<RetrackOutcome, RetrackRefusal> {
    let level = match recording {
        Recording::Fork => return retrack_tree_fork(run, path, data),
        Recording::Entry(entry) => {
            return retrack_tree_entry(run, store, path, id, data, entry, options);
        }
        Recording::Version(level) => level,
    };

    let current = read_version(run, data.version())?;
    let chain = ancestors(run, &current)?;
    let (target_hash, _number) = resolve_target(
        run,
        &chain,
        &Asked {
            level,
            path: path.as_str(),
            deny_jump: options.deny_jump,
        },
    )?;

    let root = run.workspace.get_root();
    let tree_root = root.join(path.to_path_buf());
    let tree = Tree::open(root, &tree_root).map_err(|error| tree_error(&error))?;

    let target = run
        .runtime
        .block_on(read_structure(run.index, store, &target_hash))
        .map_err(|error| tree_error(&error))?;

    let recorded = run
        .runtime
        .block_on(tree.recorded_structure(run.layout, run.index, store))
        .map_err(|error| tree_error(&error))?;

    let staging = Staging::new(&tree_root);
    let staged = match staging.structure() {
        Ok(Some(staged)) => staged,
        Ok(None) => recorded.clone(),
        Err(error) => return Err(tree_error(&error)),
    };
    let conflicts = staging.conflicts().map_err(|error| tree_error(&error))?;

    let rules = tree
        .rules(run.rules.clone())
        .map_err(|error| tree_error(&error))?;
    let scanned = tree
        .scan(&rules, false)
        .map_err(|error| tree_error(&error))?;
    let status = compare(&recorded, &staged, &scanned.structure, &conflicts);

    let dirty =
        !status.staged.is_empty() || !status.unstaged.is_empty() || !status.conflicts.is_empty();

    if dirty && !options.only_index && !options.force {
        return Err(RetrackRefusal::TreeDirty {
            path: path.as_str().to_owned(),
        });
    }

    if !options.only_index {
        bring_tree_content(run, store, &target, &recorded, !options.force)?;
        put_back(run, store, &tree_root, &target, &recorded, !options.force)?;

        let conflicts = staging.conflicts().map_err(|error| tree_error(&error))?;
        for conflict in &conflicts {
            let _ = fs::remove_file(tree_root.join(conflict_copy(conflict)));
        }
        staging.clear().map_err(|error| tree_error(&error))?;
    }

    let written = MutableData::new(
        data.owner().map(str::to_owned),
        target_hash,
        data.description().to_owned(),
    )
    .with_kind(LayoutEntryKind::Tree)
    .with_preference(data.preference().map(str::to_owned));

    let mut batch = run.layout.batch();
    if let Err(error) = batch.update_entry(id, written) {
        return Err(RetrackRefusal::Failed(WriteError::Layout {
            reason: error.to_string(),
        }));
    }
    if let Err(error) = batch.commit() {
        return Err(RetrackRefusal::Failed(WriteError::Layout {
            reason: error.to_string(),
        }));
    }

    let scanned = tree
        .scan(&rules, false)
        .map_err(|error| tree_error(&error))?;
    let baseline = tree.baseline();
    baseline
        .write(&target, &target_hash)
        .map_err(|error| tree_error(&error))?;
    baseline
        .write_entries(&scanned.cache)
        .map_err(|error| tree_error(&error))?;
    // A tree recorded before the mark held its id gets one the next time it is recorded.
    tree.write_local_id(id)
        .map_err(|error| tree_error(&error))?;

    Ok(RetrackOutcome {
        record: Recorded::Tree(RetrackedTree {
            path: path.as_str().to_owned(),
            version: hex(target_hash),
        }),
        took: run.took(),
    })
}

/// Writes `target` over the work at `root`, and takes out what `old` recorded and `target` does not.
fn put_back(
    run: &Run<'_>,
    store: &StorageSet,
    root: &Path,
    target: &TreeStructure,
    old: &TreeStructure,
    skip_unchanged: bool,
) -> Result<(), RetrackRefusal> {
    if let Err(error) = fs::create_dir_all(root) {
        return Err(internal(error.to_string()));
    }

    for entry in target.entries() {
        run.stopped()?;

        // What the old structure already held is what the work already holds: a move of the record
        // that leaves a path as it was does not read it, write it, or make its directory again.
        if skip_unchanged && old.entry(entry.path()) == Some(entry) {
            continue;
        }

        let disk = root.join(entry.path());

        match entry.kind() {
            TreeEntryKind::Directory => {
                if let Err(error) = fs::create_dir_all(&disk) {
                    return Err(internal(error.to_string()));
                }
            }
            TreeEntryKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };
                if let Some(parent) = disk.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(internal(error.to_string()));
                }
                if let Err(error) = run.runtime.block_on(store.extract_file(storage, &disk)) {
                    return Err(RetrackRefusal::Failed(WriteError::Storage {
                        reason: error.to_string(),
                    }));
                }
            }
            TreeEntryKind::Symlink => {
                let Some(target_path) = entry.target() else {
                    continue;
                };
                if disk.exists() {
                    let _ = fs::remove_file(&disk);
                }
                if let Some(parent) = disk.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(internal(error.to_string()));
                }
                #[cfg(unix)]
                {
                    if let Err(error) = std::os::unix::fs::symlink(target_path, &disk) {
                        return Err(internal(error.to_string()));
                    }
                }
                #[cfg(windows)]
                {
                    if let Err(error) = std::os::windows::fs::symlink_file(target_path, &disk) {
                        return Err(internal(error.to_string()));
                    }
                }
            }
        }
    }

    for entry in old.entries() {
        run.stopped()?;

        if target.entry(entry.path()).is_some() {
            continue;
        }

        let disk = root.join(entry.path());
        match entry.kind() {
            TreeEntryKind::Directory => {
                let _ = fs::remove_dir(&disk);
            }
            TreeEntryKind::File | TreeEntryKind::Symlink => {
                let _ = fs::remove_file(&disk);
            }
        }
    }

    Ok(())
}

/// Gives a tree's Layout entry a fresh `Uuid`, with what it records now.
///
/// The tree's own mark keeps the id the Layout names it by, so it is written with the new one: a
/// directory whose mark still named the old entry would be read as a loss and a gain.
fn retrack_tree_fork(
    run: &Run<'_>,
    path: &LayoutPath,
    data: &MutableData,
) -> Result<RetrackOutcome, RetrackRefusal> {
    let root = run.workspace.get_root();
    let tree =
        Tree::open(root, &root.join(path.to_path_buf())).map_err(|error| tree_error(&error))?;
    let target = Uuid::new_v4();

    if let Err(error) = switch_entry(run.layout, path, target, data.clone()) {
        return Err(RetrackRefusal::Failed(error));
    }
    tree.write_local_id(target)
        .map_err(|error| tree_error(&error))?;

    Ok(RetrackOutcome {
        record: Recorded::Tree(RetrackedTree {
            path: path.as_str().to_owned(),
            version: hex(data.version()),
        }),
        took: run.took(),
    })
}

/// Makes a tree record the entry a run names, in place of the one it records now.
// It is one ordered reading followed by one ordered writing, and what the order is is the whole of
// what this does: what the two versions stand at is read before the work is written over, and what
// the tree recorded is read before the entry it names changes. Splitting it would pass the same
// eight values through a second signature rather than make any step of it clearer.
#[allow(
    clippy::too_many_lines,
    reason = "the steps are one ordered reading and one ordered writing, which is what it has to say"
)]
fn retrack_tree_entry(
    run: &Run<'_>,
    store: &StorageSet,
    path: &LayoutPath,
    id: Uuid,
    data: &MutableData,
    entry: &EntryTarget,
    options: RetrackOptions,
) -> Result<RetrackOutcome, RetrackRefusal> {
    let root = run.workspace.get_root();
    let tree_root = root.join(path.to_path_buf());
    let tree = Tree::open(root, &tree_root).map_err(|error| tree_error(&error))?;
    let target = recorded_entry(run, entry, id)?;

    if target.data.kind() != LayoutEntryKind::Tree {
        let written = entry_written(entry);

        return Err(internal(told(
            &run.messages.kind_mismatch,
            &[written.as_str(), path.as_str()],
        )));
    }

    let current = data.version();
    let there = target.data.version();
    let relation = if current == there {
        Relation::Equal
    } else {
        let here = read_version(run, current)?;
        let wanted = read_version(run, there)?;
        let here_chain = digests(&ancestors(run, &here)?);
        let there_chain = digests(&ancestors(run, &wanted)?);

        relation(current, there, &here_chain, &there_chain)
    };
    let action = match switch_action(relation, options.only_index, options.deny_jump) {
        Ok(action) => action,
        Err(SwitchRefusal::NotOnChain) => {
            return Err(RetrackRefusal::Jump {
                path: path.as_str().to_owned(),
            });
        }
    };

    if action == SwitchAction::Content && !options.force && tree_has_work(run, store, &tree) {
        return Err(RetrackRefusal::TreeDirty {
            path: path.as_str().to_owned(),
        });
    }

    let version = match action {
        SwitchAction::Keep => current,
        SwitchAction::Pointer | SwitchAction::Content => there,
    };
    let written = switched_data(&target.data, data, version).with_kind(LayoutEntryKind::Tree);

    let wanted = if action == SwitchAction::Keep {
        None
    } else {
        Some(
            run.runtime
                .block_on(read_structure(run.index, store, &there))
                .map_err(|error| tree_error(&error))?,
        )
    };
    let recorded = if action == SwitchAction::Content {
        Some(
            run.runtime
                .block_on(tree.recorded_structure(run.layout, run.index, store))
                .map_err(|error| tree_error(&error))?,
        )
    } else {
        None
    };

    if action == SwitchAction::Content {
        let Some(wanted) = wanted.as_ref() else {
            return Err(internal(told(&run.messages.structure_missing, &["target"])));
        };
        let Some(recorded) = recorded.as_ref() else {
            return Err(internal(told(
                &run.messages.structure_missing,
                &["recorded"],
            )));
        };

        bring_tree_content(run, store, wanted, recorded, !options.force)?;
    }

    if let Err(error) = switch_entry(run.layout, path, target.id, written) {
        return Err(RetrackRefusal::Failed(error));
    }

    if action == SwitchAction::Content {
        // The two structures are read above exactly when the content is to be written.
        let (Some(wanted), Some(recorded)) = (wanted.as_ref(), recorded.as_ref()) else {
            return Err(internal(told(&run.messages.structure_missing, &["target"])));
        };

        put_back(run, store, &tree_root, wanted, recorded, !options.force)?;

        let staging = Staging::new(&tree_root);
        let conflicts = staging.conflicts().map_err(|error| tree_error(&error))?;
        for conflict in &conflicts {
            let _ = fs::remove_file(tree_root.join(conflict_copy(conflict)));
        }
        staging.clear().map_err(|error| tree_error(&error))?;
    }

    if action == SwitchAction::Pointer {
        // The target structure is read above for every switch that is not a keep.
        let Some(wanted) = wanted.as_ref() else {
            return Err(internal(told(&run.messages.structure_missing, &["target"])));
        };

        tree.baseline()
            .write(wanted, &there)
            .map_err(|error| tree_error(&error))?;
    }

    if action == SwitchAction::Content
        && let Some(wanted) = wanted.as_ref()
    {
        tree.baseline()
            .write(wanted, &there)
            .map_err(|error| tree_error(&error))?;
    }
    tree.write_local_id(target.id)
        .map_err(|error| tree_error(&error))?;

    Ok(RetrackOutcome {
        record: Recorded::Tree(RetrackedTree {
            path: path.as_str().to_owned(),
            version: hex(version),
        }),
        took: run.took(),
    })
}

/// Brings the content a tree structure names from a Vault when the store here is short of it.
///
/// A tree's work is many files, and the version moved to may be one this side has never carried. What
/// is fetched is every file's content key, one request rather than one per file, before anything is
/// written over the work.
///
/// A run with no account bound fetches nothing: it is a local run, and what it then writes is what
/// the store already holds.
fn bring_tree_content(
    run: &Run<'_>,
    store: &StorageSet,
    structure: &TreeStructure,
    old: &TreeStructure,
    changed_only: bool,
) -> Result<(), RetrackRefusal> {
    let keys: Vec<Key> = structure
        .entries()
        .iter()
        .filter(|entry| !changed_only || old.entry(entry.path()) != Some(entry))
        .filter_map(|entry| entry.storage().copied())
        .collect();

    if keys.is_empty() {
        return Ok(());
    }

    let Some(account) = run.account else {
        return Ok(());
    };
    let sources = sources_of(run, account)?;

    sources.bring(store, &keys);
    run.absorb(&sources);

    Ok(())
}

/// Whether the tree holds work a move of its record would write over.
fn tree_has_work(run: &Run<'_>, store: &StorageSet, tree: &Tree) -> bool {
    run.runtime
        .block_on(tree.status(run.layout, run.index, store, run.rules.clone(), false))
        .map_or(true, |status| {
            !status.staged.is_empty() || !status.unstaged.is_empty() || !status.conflicts.is_empty()
        })
}

/// The answer a tree that cannot be read or written gives.
fn tree_error(error: &crate::tree::TreeError) -> RetrackRefusal {
    RetrackRefusal::Failed(WriteError::Tree {
        reason: error.to_string(),
    })
}

/// The way a run wrote the entry a target names, for the words a refusal uses.
///
/// A `Uuid` is written as itself; a path is written as the path of the Vault's Layout it names, which
/// is what the run said and what a reader can compare against the Layout it read.
fn entry_written(entry: &EntryTarget) -> String {
    match entry {
        EntryTarget::Uuid(id) => id.to_string(),
        EntryTarget::Upstream(path) => path.as_str().to_owned(),
    }
}

/// Where the two versions stand beside one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Relation {
    /// They are the same version.
    Equal,
    /// The version the path records is ahead: the other is behind it on its own chain.
    Ahead,
    /// The version the path records is behind: it is behind the other on the other's chain.
    Behind,
    /// They are on different chains.
    Apart,
}

/// The relation `target` has to `cur`, from the chains each version is the top of.
///
/// Both chains are the hashes of what is under each version, the nearest first, as [`ancestors`]
/// answers them, and each holds what is behind its own version rather than the version itself: what
/// is asked is whether one version is on the way down from the other.
fn relation(
    cur: [u8; 32],
    target: [u8; 32],
    cur_chain: &[[u8; 32]],
    target_chain: &[[u8; 32]],
) -> Relation {
    if cur == target {
        return Relation::Equal;
    }

    // The target is behind `cur` on the way down, so what the path records is the newer one.
    if cur_chain.contains(&target) {
        return Relation::Ahead;
    }
    if target_chain.contains(&cur) {
        return Relation::Behind;
    }

    Relation::Apart
}

/// How the version a path records stands beside the version the entry to record holds.
///
/// A version the index does not hold is brought from the Vault the Layout tracks, since a chain that
/// cannot be read cannot be compared, which is the one step of this that may leave the machine.
fn stand_of(
    run: &Run<'_>,
    data: &MutableData,
    target: &MutableData,
) -> Result<Relation, RetrackRefusal> {
    let cur = data.version();
    let target_version = target.version();
    if cur == target_version {
        return Ok(Relation::Equal);
    }

    match presence(run, target_version) {
        Presence::Version => {}
        Presence::Elsewhere => {
            return Err(internal(told(
                &run.messages.not_a_version,
                &[&hex(target_version)],
            )));
        }
        Presence::Missing => {
            // Nothing the run asked for is here, and an offline run may not go looking for it.
            if run.offline {
                return Err(RetrackRefusal::Failed(WriteError::Offline));
            }

            bring(run, target_version)?;
        }
    }

    let here = read_version(run, cur)?;
    let there = read_version(run, target_version)?;
    let cur_chain = digests(&ancestors(run, &here)?);
    let target_chain = digests(&ancestors(run, &there)?);

    Ok(relation(cur, target_version, &cur_chain, &target_chain))
}

/// The hashes `chain` holds, in the order it holds them.
fn digests(chain: &[Version]) -> Vec<[u8; 32]> {
    chain
        .iter()
        .map(|version| *version.hash().digest())
        .collect()
}

/// Reads what a run said the path is to record.
///
/// The shapes are read in the order that cannot be mistaken: a step count and a version number are
/// digits, a `Uuid` is the one shape that names an entry without a path, a hash is settled among the
/// versions `versions` names, and anything left is a path of the Vault's own Layout. A hash that is
/// well formed but names no version is refused rather than read as a path, since a run that wrote one
/// meant the version.
///
/// `versions` is asked for only when the word is the head of a hash rather than a whole one, so a run
/// that names a step count does not pay for listing the index.
///
/// # Errors
///
/// Returns [`TargetRefusal`] when `text` is no version the run can move to, and when it is a step
/// count written with a dash.
pub fn recording_of(
    text: Option<&str>,
    versions: impl FnOnce() -> Vec<Key>,
) -> Result<Recording, TargetRefusal> {
    let Some(text) = text else {
        return Ok(Recording::Version(VersionTarget::Steps(1)));
    };

    // Nothing at all names nothing either: a target is something, and an empty word is not.
    if text.is_empty() {
        return Err(TargetRefusal::NotATarget);
    }

    // A sign that is not a step count is a word the parser would read as an option, and one written
    // this way names no level: `~N` is how a step count is written as a word.
    if text.strip_prefix('-').is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return Err(TargetRefusal::DashNumber);
    }
    if text.starts_with('-') || text.starts_with('+') {
        return Err(TargetRefusal::NotATarget);
    }

    if let Some(rest) = text.strip_prefix('~') {
        return rest
            .parse::<u64>()
            .map(VersionTarget::Steps)
            .map(Recording::Version)
            .map_err(|_| TargetRefusal::NotATarget);
    }

    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        return text
            .parse::<u64>()
            .map(VersionTarget::Number)
            .map(Recording::Version)
            .map_err(|_| TargetRefusal::NotATarget);
    }

    if let Ok(id) = Uuid::from_str(text) {
        return Ok(Recording::Entry(EntryTarget::Uuid(id)));
    }

    match resolve_hash(text, versions) {
        Ok(key) => return Ok(Recording::Version(VersionTarget::Hash(key))),
        Err(HashMiss::Malformed) => {}
        Err(HashMiss::Unknown) => return Err(TargetRefusal::NotAVersion),
        Err(HashMiss::Ambiguous(candidates)) => {
            return Err(TargetRefusal::AmbiguousHash { candidates });
        }
    }

    LayoutPath::new(text)
        .map(|path| Recording::Entry(EntryTarget::Upstream(path)))
        .map_err(|_| TargetRefusal::NotATarget)
}

/// Why a word named no one hash.
enum HashMiss {
    /// It reads as neither a hash nor the head of one.
    Malformed,
    /// It reads as a head, and nothing the run can reach starts with it.
    Unknown,
    /// It reads as a head, and more than one thing the run can reach starts with it.
    Ambiguous(Vec<String>),
}

/// Reads `text` as a whole hash, or as the head of one among the versions there are.
///
/// A whole hash is read on its own, so a run that names one pays no listing.
fn resolve_hash(text: &str, versions: impl FnOnce() -> Vec<Key>) -> Result<Key, HashMiss> {
    if let Ok(key) = Key::from_str(text) {
        return Ok(key);
    }

    let Ok(head) = ShortHash::new(text) else {
        return Err(HashMiss::Malformed);
    };

    match head.resolve(versions().iter()) {
        Resolved::ExactlyOne(key) => Ok(key),
        Resolved::NoMatch => Err(HashMiss::Unknown),
        Resolved::Ambiguous(keys) => Err(HashMiss::Ambiguous(keys.iter().map(Key::hex).collect())),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_layout::MutableData;
    use rorolala_storage::Key;
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{
        EntryTarget, HashMiss, Recorded, Recording, Relation, RetrackOptions, RetrackOutcome,
        RetrackedPath, RetrackedTree, SwitchAction, SwitchRefusal, TargetRefusal, VersionTarget,
        recording_of, relation, resolve_hash, retrack_at, retrack_file, retrack_vault_file,
        switch_action, switched_data, told,
    };
    use crate::write::track::{TrackOptions, track_file, track_vault_file};
    use crate::write::{Cancellation, WriteProgress};

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

        let layout = workspace.layout().unwrap();
        let cache = rorolala_workspace_analyze::Cache::read(
            &rorolala_workspace_analyze::cache_path(&dir, &layout),
        );
        assert!(
            cache
                .get(&rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap())
                .is_none()
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

    #[test]
    fn a_vault_path_moves_back_one_version() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-retrack-vault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Vault::create(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();

        let retracked = retrack_vault_file(
            &vault,
            &dir,
            "a.txt",
            Some(&first.version.to_string()),
            RetrackOptions::default(),
        )
        .unwrap();

        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");
        assert_eq!(retracked.version, first.version);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Saying nothing is one version back, and a step count is how that is said as a target.
    #[test]
    fn one_step_back_is_the_default_and_the_word_for_it() {
        assert!(matches!(
            recording_of(None, Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(1)))
        ));
        assert!(matches!(
            recording_of(Some("~1"), Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(1)))
        ));
    }

    /// A step count is `~N`, a version number is digits, a hash is a hash, and anything left that a
    /// Layout may hold is a path of the Vault's own Layout.
    #[test]
    fn each_shape_is_read_as_the_target_it_is() {
        assert!(matches!(
            recording_of(Some("~3"), Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(3)))
        ));
        assert!(matches!(
            recording_of(Some("~0"), Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(0)))
        ));
        assert!(matches!(
            recording_of(Some("0"), Vec::new),
            Ok(Recording::Version(VersionTarget::Number(0)))
        ));
        assert!(matches!(
            recording_of(Some("12"), Vec::new),
            Ok(Recording::Version(VersionTarget::Number(12)))
        ));
        assert!(matches!(
            recording_of(Some("art/hero.psd"), Vec::new),
            Ok(Recording::Entry(EntryTarget::Upstream(path)))
                if path.as_str() == "art/hero.psd"
        ));

        let hex = "0f".repeat(32);
        let Ok(Recording::Version(VersionTarget::Hash(key))) = recording_of(Some(&hex), Vec::new)
        else {
            panic!("a hash is read as a hash");
        };
        assert_eq!(key, Key::new([0x0f; 32]));
    }

    /// A `Uuid` names an entry, and is read as one before anything else is tried: it is also a path
    /// a Layout may hold, so reading it as one would send the run looking upstream for an entry this
    /// Layout may hold at no path.
    #[test]
    fn a_uuid_is_read_as_an_entry_before_it_is_read_as_a_path() {
        let id = Uuid::from_u128(7);

        assert!(matches!(
            recording_of(Some(&id.to_string()), Vec::new),
            Ok(Recording::Entry(EntryTarget::Uuid(read))) if read == id
        ));
    }

    /// The head of a hash is read against the versions there are: one match is that version, none
    /// is a version that is not here, and more than one is a question that needs more digits.
    #[test]
    fn a_head_is_read_against_the_versions_there_are() {
        let one = Key::new([0x0f; 32]);
        let mut digest = [0x0f; 32];
        digest[31] = 0x1f;
        let two = Key::new(digest);

        assert!(matches!(
            recording_of(Some("0f0f"), || vec![one]),
            Ok(Recording::Version(VersionTarget::Hash(key))) if key == one
        ));
        assert!(matches!(
            recording_of(Some("0f0f"), Vec::new),
            Err(TargetRefusal::NotAVersion)
        ));
        assert!(matches!(
            recording_of(Some("0f0f"), || vec![one, two]),
            Err(TargetRefusal::AmbiguousHash { .. })
        ));
    }

    /// What is no target is refused, with a step count written with a dash refused for the spelling
    /// it is: the command line reads `-N` as a flag, so `~N` is the word for one.
    #[test]
    fn a_target_that_is_not_one_is_refused() {
        assert!(recording_of(Some("~"), Vec::new).is_err());
        assert!(recording_of(Some("~x"), Vec::new).is_err());
        assert!(recording_of(Some("+1"), Vec::new).is_err());
        assert!(recording_of(Some(""), Vec::new).is_err());
        assert!(matches!(
            recording_of(Some("-1"), Vec::new),
            Err(TargetRefusal::DashNumber)
        ));
    }

    /// A word that is neither a whole hash nor the head of one names no hash at all, and one that is
    /// a whole hash is read on its own, without the versions being asked for.
    #[test]
    fn only_a_head_is_read_against_the_versions() {
        assert!(matches!(
            resolve_hash("not-a-hash", || panic!(
                "a word that is no hash asks for none"
            )),
            Err(HashMiss::Malformed)
        ));

        let whole = Key::new([0x2a; 32]).to_string();
        let read = resolve_hash(&whole, || panic!("a whole hash asks for none"));

        assert_eq!(read.ok(), Some(Key::new([0x2a; 32])));
    }

    /// Where two versions stand is read from the chains each is the top of: the same version, one
    /// behind the other on its own chain, or two chains that meet nowhere.
    #[test]
    fn the_relation_of_two_versions_is_read_from_their_chains() {
        let old = [1; 32];
        let cur = [3; 32];
        let other = [9; 32];
        let cur_chain = [[2; 32], old];
        let target_chain = [[8; 32], old];

        assert_eq!(relation(cur, cur, &[], &[]), Relation::Equal);
        assert_eq!(
            relation(cur, old, &cur_chain, &target_chain),
            Relation::Ahead
        );
        assert_eq!(relation(old, cur, &[], &target_chain), Relation::Behind);
        assert_eq!(
            relation(cur, other, &cur_chain, &[[7; 32]]),
            Relation::Apart
        );
    }

    /// What a change of `Uuid` does to the file is decided by how the two versions stand and by the
    /// flags the run gave: the same version or a later one keeps the file, an earlier one has its
    /// content written over it unless the record alone is asked for, and another chain is a jump
    /// unless the run refused one.
    #[test]
    fn what_a_change_of_uuid_does_to_the_file_follows_the_versions_and_the_flags() {
        assert_eq!(
            switch_action(Relation::Equal, false, false),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Equal, true, true),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Ahead, false, false),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Ahead, true, true),
            Ok(SwitchAction::Keep)
        );

        // A version behind the one the entry records has its content written over the file: what
        // decides whether the file may be written is whether it stands as recorded, not the flag.
        assert_eq!(
            switch_action(Relation::Behind, false, false),
            Ok(SwitchAction::Content)
        );
        assert_eq!(
            switch_action(Relation::Behind, true, false),
            Ok(SwitchAction::Pointer)
        );
        assert_eq!(
            switch_action(Relation::Behind, true, true),
            Ok(SwitchAction::Pointer)
        );

        // Another chain is a jump, and a jump is what a run makes unless it asked not to.
        assert_eq!(
            switch_action(Relation::Apart, false, false),
            Ok(SwitchAction::Content)
        );
        assert_eq!(
            switch_action(Relation::Apart, true, false),
            Ok(SwitchAction::Pointer)
        );
        // A run that refused the jump is refused however else it was asked to go.
        assert_eq!(
            switch_action(Relation::Apart, false, true),
            Err(SwitchRefusal::NotOnChain)
        );
        assert_eq!(
            switch_action(Relation::Apart, true, true),
            Err(SwitchRefusal::NotOnChain)
        );
    }

    /// The entry a path comes to record holds what the entry it records holds, and is cut the way
    /// the path's own entry asked for: what is written is about the file, not about its `Uuid`.
    #[test]
    fn the_entry_recorded_holds_what_it_held_and_the_preference_of_the_old_one() {
        let source = MutableData::new(Some("bob".to_owned()), [2; 32], "upstream".to_owned())
            .with_preference(Some("upstream-policy".to_owned()));
        let old = MutableData::new(Some("alice".to_owned()), [1; 32], "mine".to_owned())
            .with_preference(Some("maya-binary".to_owned()));

        let data = switched_data(&source, &old, [5; 32]);

        assert_eq!(data.owner(), Some("bob"));
        assert_eq!(data.description(), "upstream");
        assert_eq!(data.version(), [5; 32]);
        assert_eq!(data.preference(), Some("maya-binary"));
    }

    /// A message carries its values where the caller put the placeholders, left to right.
    #[test]
    fn a_message_is_filled_where_its_placeholders_are() {
        assert_eq!(
            told("`{}` names a file, and `{}` is a tree", &["a.psd", "art"]),
            "`a.psd` names a file, and `art` is a tree"
        );
        assert_eq!(told("no placeholders", &["x"]), "no placeholders");
    }

    /// A JSON request reads what a caller can fill, and answers with the fields a C caller maps: the
    /// kind, the path, the version, the flags, and the `took` the Vaults handed over.
    #[test]
    fn a_json_request_retracks_one_path_and_answers_with_its_kind() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-retrack-json-{}", std::process::id()));
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

        // Nothing else is said: the bools are false, `record` is empty, and a run that needs no
        // Vault, account or store resolution beyond the Workspace's own still retracks. The file is
        // relative, which the request reads against the Workspace root rather than a process cwd.
        let request = serde_json::json!({
            "workspace": dir.to_str().unwrap(),
            "file": "a.txt",
        });
        let answer = retrack_at(
            &request.to_string(),
            &WriteProgress::new(),
            &Cancellation::new(),
        )
        .unwrap();
        let answer: serde_json::Value = serde_json::from_str(&answer).unwrap();

        assert_eq!(answer["kind"], "path");
        assert_eq!(answer["path"], "a.txt");
        assert_eq!(answer["version"], first.version.hex());
        assert_eq!(answer["changed"], true);
        assert_eq!(answer["switched"], true);
        assert_eq!(answer["forked"], false);
        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");

        let _ = fs::remove_dir_all(&dir);
    }

    /// What a run came to is written with the kind a caller branches on and with every field the
    /// command's result carries, so a number and a `Uuid` the run does not know are null and not
    /// absent, and a tree is told apart by its kind rather than by which fields happen to be there.
    #[test]
    fn what_a_retrack_came_to_is_written_with_its_kind_and_fields() {
        let moved = RetrackOutcome {
            record: Recorded::Path(RetrackedPath {
                path: "art/a.psd".to_owned(),
                version: "0f".repeat(32),
                number: Some(3),
                changed: true,
                switched: false,
                from: None,
                to: None,
                forked: false,
            }),
            took: vec![("origin".to_owned(), 2)],
        };
        let written = serde_json::to_value(&moved).unwrap();

        assert_eq!(written["kind"], "path");
        assert_eq!(written["path"], "art/a.psd");
        assert_eq!(written["number"], 3_u64);
        assert_eq!(written["switched"], false);
        assert_eq!(written["from"], serde_json::Value::Null);
        assert_eq!(written["took"][0][0], "origin");
        assert_eq!(written["took"][0][1], 2_u64);

        let tree = RetrackOutcome {
            record: Recorded::Tree(RetrackedTree {
                path: "art".to_owned(),
                version: "0f".repeat(32),
            }),
            took: Vec::new(),
        };
        let written = serde_json::to_value(&tree).unwrap();

        assert_eq!(written["kind"], "tree");
        assert_eq!(written["path"], "art");
        assert_eq!(written["took"], serde_json::json!([]));
    }
}

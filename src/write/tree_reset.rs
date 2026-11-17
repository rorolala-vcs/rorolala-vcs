//! Putting a tree's staging area back to what the recorded version holds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rorolala_layout::Layout;
use rorolala_storage::{Lockable as _, RorolalaStorage};
use rorolala_tree::{
    EntryKind, Staging, Tree, TreeEntry, TreeStructure, conflict_copy,
};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use serde::Serialize;

use crate::write::{WriteError, tree::relative_to};

/// What a reset did.
#[derive(Debug, Clone, Serialize)]
pub struct Reset {
    /// Whether the whole staging area was cleared.
    pub whole: bool,
    /// The paths as the run wrote them, or `.` for the whole area.
    pub paths: Vec<String>,
}

/// Resets the staged paths `paths` name, or the whole staging area when none are named.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when a path names nothing the tree records, stages or holds
/// in conflict, and [`WriteError::Tree`] when the staging area cannot be read or written.
pub fn reset(
    workspace: &Workspace,
    cwd: &Path,
    target: &str,
    paths: &[String],
) -> Result<Reset, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = workspace
        .get_current_rola_storage()
        .ok_or(WriteError::NoStore)?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let at = if target.is_empty() {
        cwd.to_path_buf()
    } else {
        let target = Path::new(target);
        if target.is_absolute() {
            target.to_path_buf()
        } else {
            cwd.join(target)
        }
    };
    let tree = Tree::locate(workspace.get_root(), &at).map_err(|error| match error {
        rorolala_tree::TreeError::NotATree(_) => WriteError::NotATree {
            path: at.display().to_string(),
        },
        other => WriteError::Tree {
            reason: other.to_string(),
        },
    })?;

    reset_tree(&tree, &layout, &store, &index, cwd, paths)
}

/// The reset once the tree and its parts are known.
fn reset_tree(
    tree: &Tree,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    cwd: &Path,
    paths: &[String],
) -> Result<Reset, WriteError> {
    let staging = tree.staging();
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let _guard = runtime
        .block_on(staging.lock())
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

    if paths.is_empty() {
        return clear_staging(&staging, tree.root());
    }

    let recorded = runtime
        .block_on(tree.recorded_structure(layout, index, store))
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
    let staged_structure = staging.structure().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let staged = staged_structure.clone().unwrap_or_else(|| recorded.clone());
    let conflicts = staging.conflicts().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;

    let mut entries = entries_of(&staged);
    let recorded_by_path = entries_of(&recorded);
    let (targets, named) = targets(
        tree.root(),
        cwd,
        paths,
        &entries,
        &recorded_by_path,
        &conflicts,
    )
    .map_err(|()| WriteError::InvalidPath {
        path: paths.first().cloned().unwrap_or_default(),
        reason: "the path is not one the tree records, stages or holds in conflict".to_owned(),
    })?;

    if staged_structure.is_none() && conflicts.is_empty() {
        return Ok(Reset {
            whole: false,
            paths: named,
        });
    }

    put_back(&mut entries, &recorded_by_path, &targets);
    let conflicts = settle_conflicts(conflicts, &targets, tree.root());
    crate::write::tree_remove::prune_empty_directories(&mut entries);

    let structure = TreeStructure::build(None, entries.values().cloned()).map_err(|error| {
        WriteError::Tree {
            reason: error.to_string(),
        }
    })?;
    let description = staging.description().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let same_entries = structure.entries() == recorded.entries();
    let empty = conflicts.is_empty() && description.is_none() && same_entries;

    let written = if empty {
        staging.clear()
    } else {
        staging
            .write_structure(&structure)
            .and_then(|()| staging.write_conflicts(&conflicts))
    };
    written.map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;

    Ok(Reset {
        whole: false,
        paths: named,
    })
}

/// Puts the whole staging area back to nothing, taking the copies a merge left behind with it.
fn clear_staging(staging: &Staging, root: &Path) -> Result<Reset, WriteError> {
    let conflicts = staging.conflicts().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    for path in &conflicts {
        remove_conflict(root, path);
    }
    staging.clear().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;

    Ok(Reset {
        whole: true,
        paths: Vec::new(),
    })
}

/// The entries of a structure, by path.
fn entries_of(structure: &TreeStructure) -> BTreeMap<String, TreeEntry> {
    structure
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.clone()))
        .collect()
}

/// What the named paths stand for: every entry to put back, and the paths as the run wrote them.
fn targets(
    tree_root: &Path,
    cwd: &Path,
    paths: &[String],
    entries: &BTreeMap<String, TreeEntry>,
    recorded: &BTreeMap<String, TreeEntry>,
    conflicts: &[String],
) -> Result<(BTreeSet<String>, Vec<String>), ()> {
    let known: BTreeSet<String> = entries
        .keys()
        .chain(recorded.keys())
        .chain(conflicts.iter())
        .cloned()
        .collect();

    let mut targets: BTreeSet<String> = BTreeSet::new();
    let mut named: Vec<String> = Vec::with_capacity(paths.len());

    for given in paths {
        let resolved = if Path::new(given).is_absolute() {
            Path::new(given).to_path_buf()
        } else {
            cwd.join(given)
        };
        let relative = relative_to(tree_root, &resolved).ok_or(())?;

        if relative.is_empty() {
            targets.extend(known.iter().cloned());
            named.push(".".to_owned());

            continue;
        }

        if is_directory(entries, recorded, &relative) {
            targets.extend(known.iter().filter(|path| under(&relative, path)).cloned());
        } else if known.contains(&relative) {
            targets.insert(relative.clone());
        } else {
            return Err(());
        }

        named.push(relative);
    }

    Ok((targets, named))
}

/// Puts the entries at `targets` back to what the recorded structure holds, or takes them out.
fn put_back(
    entries: &mut BTreeMap<String, TreeEntry>,
    recorded: &BTreeMap<String, TreeEntry>,
    targets: &BTreeSet<String>,
) {
    for path in targets {
        match recorded.get(path) {
            Some(entry) => {
                entries.insert(path.clone(), entry.clone());
            }
            None => {
                entries.remove(path);
            }
        }
    }
}

/// Drops the conflicts a reset resolves, and the copies they left beside them.
fn settle_conflicts(
    conflicts: Vec<String>,
    targets: &BTreeSet<String>,
    root: &Path,
) -> Vec<String> {
    conflicts
        .into_iter()
        .filter(|path| {
            let reset = targets
                .iter()
                .any(|target| target == path || under(target, path));

            if reset {
                remove_conflict(root, path);
            }

            !reset
        })
        .collect()
}

/// Whether `path` is a directory of the staged or the recorded structure.
fn is_directory(
    staged: &BTreeMap<String, TreeEntry>,
    recorded: &BTreeMap<String, TreeEntry>,
    path: &str,
) -> bool {
    staged
        .get(path)
        .or_else(|| recorded.get(path))
        .is_some_and(|entry| entry.kind() == EntryKind::Directory)
}

/// Removes the `~theirs` copy a merge left beside `path`, when it is there.
fn remove_conflict(root: &Path, path: &str) {
    let copy = root.join(conflict_copy(path));

    let _ = std::fs::remove_file(&copy);
    let _ = std::fs::remove_dir_all(&copy);
}

/// Whether `path` is `scope` itself or under it.
fn under(scope: &str, path: &str) -> bool {
    scope.is_empty()
        || path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Resets the staging area of the tree a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"whole":false,"paths":[...]}`, or
/// a `RolaWriteError *` the caller releases with `free_rola_write_error`. `paths` is a JSON array of
/// strings, and `target` is the tree's path, empty for the directory itself.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`reset`] names.
#[lazyffi(export = rola_write_tree_reset)]
pub fn reset_at(
    directory: &Path,
    target: &str,
    paths: &str,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let paths: Vec<String> = serde_json::from_str(paths).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })?;
    let tree_root = if target.is_empty() {
        directory.to_path_buf()
    } else {
        let target = Path::new(target);
        if target.is_absolute() {
            target.to_path_buf()
        } else {
            directory.join(target)
        }
    };
    let reset = reset(&workspace, &tree_root, "", &paths)?;

    serde_json::to_string(&reset).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

//! Writing a tree's description, which is the note the next version carries.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rorolala_ignore::IgnoreRuleSet;
use rorolala_storage::{Batch, Lockable as _, RorolalaStorage, StorageSet};
use rorolala_tree::{EntryKind, Staging, Tree, TreeEntry, TreeRules, TreeStructure, conflict_copy};
use rorolala_utils_constants::WORKSPACE_IGNORE_RULES_DIR;
use rorolala_utils_lazyffi::{StrSpan, lazyffi};
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;

use crate::write::{Cancellation, WriteError};

/// Writes the description of the next version of the tree at `target`.
///
/// An empty description clears it. The text is the caller's; this only puts it where the next
/// version reads it.
///
/// # Errors
///
/// Returns [`WriteError::Tree`] when no tree holds `target`, when its staging store cannot be made,
/// or when the description cannot be written.
pub fn write_description(
    workspace: &Workspace,
    target: &Path,
    text: &str,
) -> Result<usize, WriteError> {
    let tree = Tree::locate(workspace.get_root(), target).map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let staging = Staging::new(tree.root());
    staging.ensure_store().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    staging
        .write_description(Some(text))
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

    Ok(text.len())
}

/// Writes the description of the tree a directory is in.
///
/// # FFI
///
/// A `RolaWriteTreeDescription *` the caller releases with `free_rola_write_tree_description`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `target` is the tree's
/// path, relative to `directory`, and empty for the directory itself.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and
/// [`WriteError::Tree`] when the tree or its staging store cannot be written.
#[lazyffi(export = rola_write_tree_description)]
pub fn write_description_at(
    directory: &Path,
    target: &str,
    text: &str,
) -> Result<WriteTreeDescription, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let target = if target.is_empty() {
        directory.to_path_buf()
    } else {
        let target = Path::new(target);
        if target.is_absolute() {
            target.to_path_buf()
        } else {
            directory.join(target)
        }
    };
    let bytes = write_description(&workspace, &target, text)?;

    Ok(WriteTreeDescription {
        bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
    })
}

/// How many bytes the description a tree now waits to carry is.
///
/// # FFI
///
/// A `RolaWriteTreeDescription *` the caller releases with `free_rola_write_tree_description`.
/// `_bytes` is how many bytes the description is.
#[lazyffi(export = RolaWriteTreeDescription)]
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteTreeDescription {
    /// How many bytes the description is.
    bytes: u64,
}

#[lazyffi(export = rola_)]
impl WriteTreeDescription {
    /// How many bytes the description is.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// What happened to one path when work was staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddChangeKind {
    /// The path was not staged before.
    Added,
    /// The path was staged, and what it holds changed.
    Updated,
    /// The path is no longer in the work under a staged scope.
    Deleted,
}

/// One path a staging changed.
#[derive(Debug, Clone)]
pub struct AddChange {
    /// The path, as the tree writes it.
    pub path: String,
    /// What happened to it.
    pub kind: AddChangeKind,
}

/// Stages the work under `paths` into the tree at `target`.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when a path is outside the tree or names nothing, and
/// [`WriteError::Tree`] when the staging area or the store cannot be read or written.
pub fn stage_paths(
    workspace: &Workspace,
    cwd: &Path,
    target: &str,
    paths: &[String],
    include_ignored: bool,
) -> Result<Vec<AddChange>, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let set = workspace.storage_set()?;
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

    stage(&tree, &layout, &set, &index, cwd, paths, include_ignored)
}

/// Stages the paths, and answers what the staged structure changed.
fn stage(
    tree: &Tree,
    layout: &rorolala_layout::Layout,
    store: &StorageSet,
    index: &VCSIndex,
    cwd: &Path,
    paths: &[String],
    include_ignored: bool,
) -> Result<Vec<AddChange>, WriteError> {
    let staging = tree.staging();
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let _guard = runtime
        .block_on(staging.lock())
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
    let staging_store = staging.ensure_store().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let mut staging_batch = runtime.block_on(staging_store.begin_batch());

    let base = match staging.structure() {
        Ok(Some(staged)) => staged,
        Ok(None) => runtime
            .block_on(tree.recorded_structure(layout, index, store))
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?,
        Err(error) => {
            return Err(WriteError::Tree {
                reason: error.to_string(),
            });
        }
    };

    let rules = if include_ignored {
        TreeRules::empty(IgnoreRuleSet::empty())
    } else {
        let rules = IgnoreRuleSet::load(&tree.root().join(WORKSPACE_IGNORE_RULES_DIR), tree.root())
            .unwrap_or_else(|_| IgnoreRuleSet::empty());
        tree.rules(rules).map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?
    };
    let scanned = tree
        .scan(&rules, include_ignored)
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

    let mut entries: BTreeMap<String, TreeEntry> = base
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.clone()))
        .collect();
    let before = entries.clone();
    let mut conflicts = staging.conflicts().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let tree_root = tree.root().to_path_buf();
    let scopes = stage_named(
        &staging,
        &staging_store,
        &mut staging_batch,
        &tree_root,
        &scanned,
        &mut entries,
        &mut conflicts,
        cwd,
        paths,
        &runtime,
    )?;
    stage_scopes(
        &staging,
        &staging_store,
        &mut staging_batch,
        &tree_root,
        &scanned,
        &mut entries,
        &mut conflicts,
        &scopes,
        &runtime,
    )?;

    runtime
        .block_on(staging_batch.publish())
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

    let structure = TreeStructure::build(None, entries.values().cloned()).map_err(|error| {
        WriteError::Tree {
            reason: error.to_string(),
        }
    })?;
    staging
        .write_structure(&structure)
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
    staging
        .write_conflicts(&conflicts)
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

    Ok(changes(&before, &structure))
}

/// Stages the paths named on the command line, and answers the scopes they opened.
#[allow(
    clippy::too_many_arguments,
    reason = "staging one path needs the staging store, the batch and the scanned structure together"
)]
fn stage_named(
    staging: &Staging,
    staging_store: &RorolalaStorage,
    staging_batch: &mut Batch,
    tree_root: &Path,
    scanned: &rorolala_tree::Scanned,
    entries: &mut BTreeMap<String, TreeEntry>,
    conflicts: &mut Vec<String>,
    cwd: &Path,
    paths: &[String],
    runtime: &tokio::runtime::Runtime,
) -> Result<Vec<String>, WriteError> {
    let mut scopes: Vec<String> = Vec::new();

    for given in paths {
        let resolved = if Path::new(given).is_absolute() {
            PathBuf::from(given)
        } else {
            cwd.join(given)
        };
        let Some(relative) = relative_to(tree_root, &resolved) else {
            return Err(WriteError::InvalidPath {
                path: given.clone(),
                reason: "the path is outside the tree".to_owned(),
            });
        };

        if relative.is_empty() {
            scopes.push(String::new());
            continue;
        }

        if let Some(entry) = scanned.structure.entry(&relative).cloned() {
            let staged = stage_entry(
                staging,
                staging_store,
                staging_batch,
                tree_root,
                &entry,
                entries.get(&relative),
                runtime,
            )?;
            if entry.kind() == EntryKind::Directory {
                scopes.push(relative);
            } else {
                entries.insert(staged.path().to_owned(), staged);
                resolve_conflict(tree_root, conflicts, &relative);
            }
            continue;
        }

        if entries.contains_key(&relative) {
            entries.remove(&relative);
            scopes.push(relative);
            continue;
        }

        return Err(WriteError::InvalidPath {
            path: given.clone(),
            reason: "the tree holds no such path".to_owned(),
        });
    }

    Ok(scopes)
}

/// Brings every staged path under a scope up to what the work holds.
#[allow(
    clippy::too_many_arguments,
    reason = "bringing a scope up to date needs the staging store, the batch and the scanned structure together"
)]
fn stage_scopes(
    staging: &Staging,
    staging_store: &RorolalaStorage,
    staging_batch: &mut Batch,
    tree_root: &Path,
    scanned: &rorolala_tree::Scanned,
    entries: &mut BTreeMap<String, TreeEntry>,
    conflicts: &mut Vec<String>,
    scopes: &[String],
    runtime: &tokio::runtime::Runtime,
) -> Result<(), WriteError> {
    if scopes.is_empty() {
        return Ok(());
    }

    for entry in scanned.structure.entries() {
        if scopes.iter().any(|scope| under(scope, entry.path())) {
            let staged = stage_entry(
                staging,
                staging_store,
                staging_batch,
                tree_root,
                entry,
                entries.get(entry.path()),
                runtime,
            )?;
            entries.insert(staged.path().to_owned(), staged);
        }
    }

    let gone: Vec<String> = entries
        .keys()
        .filter(|path| {
            scopes.iter().any(|scope| under(scope, path)) && scanned.structure.entry(path).is_none()
        })
        .cloned()
        .collect();
    for path in &gone {
        entries.remove(path);
        resolve_conflict(tree_root, conflicts, path);
    }

    Ok(())
}

/// Hands one scanned entry into the staging store, unless what is already staged is the same work.
fn stage_entry(
    staging: &Staging,
    staging_store: &RorolalaStorage,
    staging_batch: &mut Batch,
    tree_root: &Path,
    entry: &TreeEntry,
    known: Option<&TreeEntry>,
    runtime: &tokio::runtime::Runtime,
) -> Result<TreeEntry, WriteError> {
    if known == Some(entry) {
        return Ok(entry.clone());
    }

    let disk = tree_root.join(entry.path());
    let result = match entry.kind() {
        EntryKind::File => runtime.block_on(staging.stage_file_batch(
            staging_store,
            staging_batch,
            &disk,
            entry.path(),
        )),
        EntryKind::Symlink => staging.stage_symlink(&disk, entry.path()),
        EntryKind::Directory => return Ok(entry.clone()),
    };

    result.map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })
}

/// Removes a path from the conflict list, and the copy a merge left beside it, when it was in it.
fn resolve_conflict(tree_root: &Path, conflicts: &mut Vec<String>, path: &str) {
    if let Some(at) = conflicts.iter().position(|held| held == path) {
        conflicts.remove(at);
        let _ = std::fs::remove_file(tree_root.join(conflict_copy(path)));
    }
}

/// The changes the staging made, from the structure before it to the one after.
fn changes(before: &BTreeMap<String, TreeEntry>, after: &TreeStructure) -> Vec<AddChange> {
    let mut changes = Vec::new();

    for entry in after.entries() {
        match before.get(entry.path()) {
            None => changes.push(AddChange {
                path: entry.path().to_owned(),
                kind: AddChangeKind::Added,
            }),
            Some(was) if was != entry => changes.push(AddChange {
                path: entry.path().to_owned(),
                kind: AddChangeKind::Updated,
            }),
            Some(_) => {}
        }
    }

    for path in before.keys() {
        if after.entry(path).is_none() {
            changes.push(AddChange {
                path: path.clone(),
                kind: AddChangeKind::Deleted,
            });
        }
    }

    changes
}

/// `path` inside `root`, or nothing when it is not.
pub(crate) fn relative_to(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        return Some(String::new());
    }

    relative
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

/// Whether `path` is `scope` itself or under it.
fn under(scope: &str, path: &str) -> bool {
    scope.is_empty()
        || path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Stages work into the tree a directory is in.
///
/// # FFI
///
/// A `RolaWriteTreeAdd *` the caller releases with `free_rola_write_tree_add`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `paths` is a
/// `RolaStrSpan` of the paths to stage, and `target` is the tree's path, empty for the directory
/// itself.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`stage_paths`] names.
#[lazyffi(export = rola_write_tree_add)]
pub fn stage_paths_at(
    directory: &Path,
    target: &str,
    paths: StrSpan,
    include_ignored: bool,
) -> Result<WriteTreeAdd, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let paths = paths.into_vec();
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
    let changes = stage_paths(&workspace, &tree_root, "", &paths, include_ignored)?;

    Ok(WriteTreeAdd {
        changes: changes.iter().map(WriteTreeAddChange::of).collect(),
    })
}

/// What staging work into a tree changed.
///
/// # FFI
///
/// A `RolaWriteTreeAdd *` the caller releases with `free_rola_write_tree_add`. `_change_count` and
/// `_change_at` answer what changed, each a `RolaWriteTreeAddChange *` released with
/// `free_rola_write_tree_add_change`.
#[lazyffi(export = RolaWriteTreeAdd)]
#[derive(Debug, Clone, Default)]
pub struct WriteTreeAdd {
    /// One change per path the staging touched, in the order the tree writes them.
    changes: Vec<WriteTreeAddChange>,
}

#[lazyffi(export = rola_)]
impl WriteTreeAdd {
    /// How many paths the staging changed.
    #[must_use]
    pub const fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// One path the staging changed, or null past the end.
    #[must_use]
    pub fn change_at(&self, index: usize) -> Option<WriteTreeAddChange> {
        self.changes.get(index).cloned()
    }
}

/// One path a staging changed, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaWriteTreeAddChange *` the caller releases with `free_rola_write_tree_add_change`.
/// `_path` and `_kind` each return a `char *` the caller releases with `free_string`; `_kind` is
/// `added`, `updated` or `deleted`.
#[lazyffi(export = RolaWriteTreeAddChange)]
#[derive(Debug, Clone, Default)]
pub struct WriteTreeAddChange {
    /// The path, as the tree writes it.
    path: String,
    /// What happened to it.
    kind: String,
}

impl WriteTreeAddChange {
    /// The answer one changed path is handed over as.
    fn of(change: &AddChange) -> Self {
        Self {
            path: change.path.clone(),
            kind: change_word(change.kind).to_owned(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteTreeAddChange {
    /// The path, as the tree writes it.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// What happened to it.
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }
}

/// The word the C ABI reports one change by.
const fn change_word(kind: AddChangeKind) -> &'static str {
    match kind {
        AddChangeKind::Added => "added",
        AddChangeKind::Updated => "updated",
        AddChangeKind::Deleted => "deleted",
    }
}

/// Writes `target` over the work at `root`, and takes out what `old` recorded and `target` does not.
///
/// What the old structure already held is what the work already holds, so an entry that did not
/// change is neither read nor written again. A run asked to stop leaves the entries it has written
/// and nothing more.
///
/// # Errors
///
/// Returns [`WriteError::Cancelled`] when `cancellation` is set, [`WriteError::Io`] when a
/// directory or a link cannot be made or taken out, and [`WriteError::Storage`] when a stored file
/// cannot be written.
pub(crate) fn put_back(
    runtime: &tokio::runtime::Runtime,
    store: &StorageSet,
    root: &Path,
    target: &TreeStructure,
    old: &TreeStructure,
    skip_unchanged: bool,
    cancellation: &Cancellation,
) -> Result<(), WriteError> {
    fs::create_dir_all(root).map_err(|error| io(root, &error))?;

    for entry in target.entries() {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        // What the old structure already held is what the work already holds: a move of the record
        // that leaves a path as it was does not read it, write it, or make its directory again.
        if skip_unchanged && old.entry(entry.path()) == Some(entry) {
            continue;
        }

        let disk = root.join(entry.path());

        match entry.kind() {
            EntryKind::Directory => {
                fs::create_dir_all(&disk).map_err(|error| io(&disk, &error))?;
            }
            EntryKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };
                if let Some(parent) = disk.parent() {
                    fs::create_dir_all(parent).map_err(|error| io(parent, &error))?;
                }
                runtime
                    .block_on(store.extract_file(storage, &disk))
                    .map_err(|error| WriteError::Storage {
                        reason: error.to_string(),
                    })?;
            }
            EntryKind::Symlink => {
                let Some(target_path) = entry.target() else {
                    continue;
                };
                if disk.exists() {
                    let _ = fs::remove_file(&disk);
                }
                if let Some(parent) = disk.parent() {
                    fs::create_dir_all(parent).map_err(|error| io(parent, &error))?;
                }
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(target_path, &disk)
                        .map_err(|error| io(&disk, &error))?;
                }
                #[cfg(windows)]
                {
                    std::os::windows::fs::symlink_file(target_path, &disk)
                        .map_err(|error| io(&disk, &error))?;
                }
            }
        }
    }

    for entry in old.entries() {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        if target.entry(entry.path()).is_some() {
            continue;
        }

        let disk = root.join(entry.path());
        match entry.kind() {
            EntryKind::Directory => {
                let _ = fs::remove_dir(&disk);
            }
            EntryKind::File | EntryKind::Symlink => {
                let _ = fs::remove_file(&disk);
            }
        }
    }

    Ok(())
}

/// The failure a path the work could not be written to is told as.
fn io(path: &Path, error: &std::io::Error) -> WriteError {
    WriteError::Io {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_storage::{Codec, RorolalaStorage, StorageSet};
    use rorolala_tree::{FileMode, Staging, TreeEntry, TreeStructure, conflict_copy};
    use rorolala_utils_lazyffi::StrSpan;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::{
        AddChangeKind, Cancellation, WriteError, put_back, stage_paths, write_description,
        write_description_at,
    };

    #[test]
    fn a_file_is_added_to_the_staged_structure() {
        let dir = std::env::temp_dir().join(format!("rola-write-add-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::write(dir.join("tree/one.txt"), b"one").unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        let answer = super::stage_paths_at(
            &dir,
            "tree",
            StrSpan::from(vec!["one.txt".to_owned()]),
            false,
        )
        .unwrap();
        assert_eq!(answer.change_count(), 1);
        assert_eq!(
            answer.change_at(0).expect("the change is there").path(),
            "one.txt"
        );

        let changes = super::stage_paths(
            &workspace,
            &dir.join("tree"),
            "",
            &["one.txt".to_owned()],
            false,
        )
        .unwrap();

        assert!(changes.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_description_is_written_into_the_staging_tree() {
        let dir = std::env::temp_dir().join(format!("rola-write-tree-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::create_dir_all(dir.join("tree")).unwrap();
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();

        let bytes = write_description(&workspace, &dir.join("tree"), "first").unwrap();

        assert_eq!(bytes, 5);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A description is written into the tree a directory names, however the target is written.
    #[test]
    fn a_description_is_written_by_relative_and_absolute_targets() {
        let dir = std::env::temp_dir().join(format!(
            "rola-cov-tree-description-at-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".rola-tree")).unwrap();
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        Workspace::create(&dir).unwrap();

        // The empty target names the directory itself, which is a tree here.
        let here = write_description_at(&dir, "", "today").unwrap();
        assert_eq!(here.bytes(), 5);

        let relative = write_description_at(&dir, "tree", "two").unwrap();
        assert_eq!(relative.bytes(), 3);
        let text = Staging::new(&dir.join("tree"))
            .description()
            .unwrap()
            .unwrap_or_default();
        assert_eq!(text, "two");

        let absolute =
            write_description_at(&dir, &dir.join("tree").display().to_string(), "one").unwrap();
        assert_eq!(absolute.bytes(), 3);

        let outside = std::env::temp_dir().join(format!(
            "rola-cov-tree-description-none-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&outside).unwrap();
        let error = write_description_at(&outside, "", "none").unwrap_err();
        assert!(matches!(error, WriteError::NoWorkspace));

        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A directory is staged through its whole work, and a file removed from under it is staged
    /// as gone.
    #[test]
    fn staging_a_directory_brings_every_file_under_it() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-tree-directory-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::create_dir_all(dir.join("tree/art/sub")).unwrap();
        fs::write(dir.join("tree/art/one.txt"), b"one").unwrap();
        fs::write(dir.join("tree/art/sub/two.txt"), b"two").unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let tree = dir.join("tree");

        let added = stage_paths(&workspace, &tree, "", &["art".to_owned()], false).unwrap();
        assert!(
            added
                .iter()
                .any(|change| change.path == "art/one.txt" && change.kind == AddChangeKind::Added),
            "the file under the directory is staged"
        );

        fs::write(tree.join("art/one.txt"), b"changed").unwrap();
        let updated = stage_paths(&workspace, &tree, "", &["art".to_owned()], false).unwrap();
        assert!(
            updated
                .iter()
                .any(|change| change.path == "art/one.txt" && change.kind == AddChangeKind::Updated),
            "a changed file under the directory is updated"
        );

        fs::remove_file(tree.join("art/sub/two.txt")).unwrap();
        let deleted = stage_paths(&workspace, &tree, "", &["art".to_owned()], false).unwrap();
        assert!(
            deleted
                .iter()
                .any(|change| change.path == "art/sub/two.txt"
                    && change.kind == AddChangeKind::Deleted),
            "a file the work no longer holds is staged as gone"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A path outside the tree, a path the tree never held, and a directory that is not a tree are
    /// each refused, rather than staged as something else.
    #[test]
    fn an_unstageable_path_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-tree-refused-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::create_dir_all(dir.join("plain")).unwrap();
        fs::write(dir.join("tree/one.txt"), b"one").unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let tree = dir.join("tree");

        let outside =
            stage_paths(&workspace, &tree, "", &["../plain".to_owned()], false).unwrap_err();
        assert!(matches!(outside, WriteError::InvalidPath { .. }));

        let absent =
            stage_paths(&workspace, &tree, "", &["absent.txt".to_owned()], false).unwrap_err();
        assert!(matches!(absent, WriteError::InvalidPath { .. }));

        let not_a_tree = stage_paths(&workspace, &tree, "../plain", &[], false).unwrap_err();
        assert!(matches!(not_a_tree, WriteError::NotATree { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    /// Staging a path a merge left in conflict takes the path and its copy out of the list.
    #[test]
    fn staging_a_path_takes_it_out_of_the_conflicts() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-tree-conflict-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::write(dir.join("tree/one.txt"), b"one").unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let tree = dir.join("tree");
        let staging = Staging::new(&tree);
        staging.ensure_store().unwrap();
        staging.write_conflicts(&["one.txt".to_owned()]).unwrap();
        let copy = tree.join(conflict_copy("one.txt"));
        fs::write(&copy, b"theirs").unwrap();

        stage_paths(&workspace, &tree, "", &["one.txt".to_owned()], false).unwrap();

        assert_eq!(staging.conflicts().unwrap().len(), 0);
        assert!(
            !copy.exists(),
            "the copy a merge left is taken away with the conflict"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// Removing a staged path from the work stages its deletion.
    #[test]
    fn a_staged_path_the_work_no_longer_holds_is_staged_as_gone() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-tree-deleted-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::write(dir.join("tree/one.txt"), b"one").unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let tree = dir.join("tree");

        stage_paths(&workspace, &tree, "", &["one.txt".to_owned()], false).unwrap();
        fs::remove_file(tree.join("one.txt")).unwrap();
        let staged = stage_paths(&workspace, &tree, "", &["one.txt".to_owned()], false).unwrap();

        assert!(
            staged
                .iter()
                .any(|change| change.path == "one.txt" && change.kind == AddChangeKind::Deleted),
            "the removal is reported as a deletion"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// Writing a structure back over a work writes what it names and takes away what it dropped.
    #[test]
    fn putting_back_writes_the_target_and_takes_out_what_it_dropped() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-tree-put-back-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = StorageSet::sole(RorolalaStorage::create(dir.join("store")));
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let alpha = runtime
            .block_on(store.write_object(b"alpha", Codec::Raw))
            .unwrap();
        let beta = runtime
            .block_on(store.write_object(b"beta", Codec::Raw))
            .unwrap();
        let root = dir.join("work");

        let target = TreeStructure::build(
            None,
            [
                TreeEntry::directory("art"),
                TreeEntry::file("art/a.txt", FileMode::Regular, alpha),
                TreeEntry::file("b.txt", FileMode::Regular, beta),
            ],
        )
        .unwrap();
        let old = TreeStructure::build(
            None,
            [
                TreeEntry::directory("art"),
                TreeEntry::directory("gone"),
                TreeEntry::file("art/a.txt", FileMode::Regular, beta),
                TreeEntry::file("art/old.txt", FileMode::Regular, alpha),
            ],
        )
        .unwrap();

        run_put_back(&runtime, &store, &root, &target, &old);

        assert_eq!(fs::read(root.join("art/a.txt")).unwrap(), b"alpha");
        assert_eq!(fs::read(root.join("b.txt")).unwrap(), b"beta");
        assert!(
            !root.join("art/old.txt").exists(),
            "an entry dropped is taken away"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A put-back asked to stop writes nothing, and one whose content is not in the store says so.
    #[test]
    fn a_cancelled_or_unstored_put_back_is_refused() {
        let dir = std::env::temp_dir().join(format!(
            "rola-cov-tree-put-back-fail-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = StorageSet::sole(RorolalaStorage::create(dir.join("store")));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let root = dir.join("work");
        let target = TreeStructure::build(
            None,
            [TreeEntry::file(
                "a.txt",
                FileMode::Regular,
                rorolala_storage::Key::new([9; 32]),
            )],
        )
        .unwrap();

        let cancelled = Cancellation::new();
        cancelled.cancel();
        let error = put_back(
            &runtime,
            &store,
            &root,
            &target,
            &TreeStructure::empty(),
            false,
            &cancelled,
        )
        .unwrap_err();
        assert!(matches!(error, WriteError::Cancelled));

        let error = put_back(
            &runtime,
            &store,
            &root,
            &target,
            &TreeStructure::empty(),
            false,
            &Cancellation::new(),
        )
        .unwrap_err();
        assert!(matches!(error, WriteError::Storage { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    /// Writes `target` over `root`, with `old` as what was there before.
    fn run_put_back(
        runtime: &tokio::runtime::Runtime,
        store: &StorageSet,
        root: &std::path::Path,
        target: &TreeStructure,
        old: &TreeStructure,
    ) {
        put_back(
            runtime,
            store,
            root,
            target,
            old,
            true,
            &Cancellation::new(),
        )
        .unwrap();
        put_back(
            runtime,
            store,
            root,
            target,
            target,
            true,
            &Cancellation::new(),
        )
        .unwrap();
    }
}

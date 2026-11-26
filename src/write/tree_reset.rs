//! Putting a tree's staging area back to what the recorded version holds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rorolala_layout::Layout;
use rorolala_storage::{Lockable as _, StorageSet};
use rorolala_tree::{EntryKind, Staging, Tree, TreeEntry, TreeStructure, conflict_copy};
use rorolala_utils_lazyffi::{StrSpan, lazyffi};
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;

use crate::write::{WriteError, tree::relative_to};

/// What a reset did.
#[derive(Debug, Clone)]
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

    reset_tree(&tree, &layout, &set, &index, cwd, paths)
}

/// The reset once the tree and its parts are known.
fn reset_tree(
    tree: &Tree,
    layout: &Layout,
    store: &StorageSet,
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
/// A `RolaWriteTreeReset *` the caller releases with `free_rola_write_tree_reset`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `paths` is a
/// `RolaStrSpan` of the paths to reset, empty for the whole staging area, and `target` is the
/// tree's path, empty for the directory itself.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`reset`] names.
#[lazyffi(export = rola_write_tree_reset)]
pub fn reset_at(
    directory: &Path,
    target: &str,
    paths: StrSpan,
) -> Result<WriteTreeReset, WriteError> {
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
    let reset = reset(&workspace, &tree_root, "", &paths)?;

    Ok(WriteTreeReset {
        whole: reset.whole,
        paths: reset.paths,
    })
}

/// What a reset did, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaWriteTreeReset *` the caller releases with `free_rola_write_tree_reset`. `_whole` says
/// whether the whole staging area was cleared, and `_path_count` and `_path_at` answer the paths
/// as the run wrote them, each a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaWriteTreeReset)]
#[derive(Debug, Clone, Default)]
pub struct WriteTreeReset {
    /// Whether the whole staging area was cleared.
    whole: bool,
    /// The paths as the run wrote them, or `.` for the whole area.
    paths: Vec<String>,
}

#[lazyffi(export = rola_)]
impl WriteTreeReset {
    /// Whether the whole staging area was cleared.
    #[must_use]
    pub const fn whole(&self) -> bool {
        self.whole
    }

    /// How many paths the run reset.
    #[must_use]
    pub const fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// One path the run reset, or an empty string past the end.
    #[must_use]
    pub fn path_at(&self, index: usize) -> String {
        self.paths.get(index).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::Key;
    use rorolala_tree::{FileMode, TreeEntry, TreeStructure, conflict_copy};

    use super::{
        WriteTreeReset, entries_of, is_directory, put_back, remove_conflict, settle_conflicts,
        targets, under,
    };

    fn key(seed: u8) -> Key {
        Key::new([seed; 32])
    }

    fn file(path: &str) -> TreeEntry {
        TreeEntry::file(path, FileMode::Regular, key(1))
    }

    fn executable(path: &str) -> TreeEntry {
        TreeEntry::file(path, FileMode::Executable, key(2))
    }

    fn directory(path: &str) -> TreeEntry {
        TreeEntry::directory(path)
    }

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-tree-reset-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A scope names itself and everything below it, and nothing that merely starts alike.
    #[test]
    fn a_scope_holds_itself_and_everything_under_it() {
        assert!(under("", "a"));
        assert!(under("a", "a"));
        assert!(under("a", "a/b"));
        assert!(!under("a", "ab"));
        assert!(!under("a/b", "a"));
    }

    /// The entries of a structure are keyed by the path they name.
    #[test]
    fn the_entries_of_a_structure_are_keyed_by_path() {
        let structure = TreeStructure::build(None, [file("a.psd"), directory("renders")]).unwrap();
        let entries = entries_of(&structure);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries["a.psd"], file("a.psd"));
        assert_eq!(entries["renders"], directory("renders"));
    }

    /// A path is a directory when either structure says it is one.
    #[test]
    fn a_directory_is_one_in_either_structure() {
        let staged = BTreeMap::from([
            ("renders".to_owned(), directory("renders")),
            ("a.psd".to_owned(), file("a.psd")),
        ]);
        let recorded = BTreeMap::from([("old".to_owned(), directory("old"))]);

        assert!(is_directory(&staged, &recorded, "renders"));
        assert!(is_directory(&staged, &recorded, "old"));
        assert!(!is_directory(&staged, &recorded, "a.psd"));
        assert!(!is_directory(&staged, &recorded, "missing"));
    }

    /// A named path stands for itself, a directory over it, or the whole tree.
    #[test]
    fn a_named_path_stands_for_itself_a_directory_or_the_whole_tree() {
        let root = std::env::temp_dir();
        let cwd = root.clone();
        let staged = BTreeMap::from([
            ("a.psd".to_owned(), file("a.psd")),
            ("renders".to_owned(), directory("renders")),
            ("renders/b.psd".to_owned(), file("renders/b.psd")),
        ]);
        let recorded = BTreeMap::from([("gone.psd".to_owned(), file("gone.psd"))]);
        let conflicts = vec!["conflict.psd".to_owned()];

        let (selected, named) = targets(
            &root,
            &cwd,
            &["a.psd".to_owned()],
            &staged,
            &recorded,
            &conflicts,
        )
        .unwrap();
        assert_eq!(selected, BTreeSet::from(["a.psd".to_owned()]));
        assert_eq!(named, ["a.psd".to_owned()]);

        let (selected, _) = targets(
            &root,
            &cwd,
            &["renders".to_owned()],
            &staged,
            &recorded,
            &conflicts,
        )
        .unwrap();
        assert_eq!(
            selected,
            BTreeSet::from(["renders".to_owned(), "renders/b.psd".to_owned()])
        );

        let (selected, named) = targets(
            &root,
            &cwd,
            &[".".to_owned()],
            &staged,
            &recorded,
            &conflicts,
        )
        .unwrap();
        assert_eq!(selected.len(), 5);
        assert_eq!(named, [".".to_owned()]);

        let absolute = root.join("a.psd");
        let (_, named) = targets(
            &root,
            &cwd,
            &[absolute.to_string_lossy().into_owned()],
            &staged,
            &recorded,
            &conflicts,
        )
        .unwrap();
        assert_eq!(named, ["a.psd".to_owned()]);

        assert!(
            targets(
                &root,
                &cwd,
                &["nothing.psd".to_owned()],
                &staged,
                &recorded,
                &conflicts
            )
            .is_err()
        );
    }

    /// Putting a path back takes the recorded entry, and one no version names is taken out.
    #[test]
    fn putting_back_restores_recorded_entries_and_removes_the_others() {
        let mut entries = BTreeMap::from([
            ("a.psd".to_owned(), file("a.psd")),
            ("new.psd".to_owned(), file("new.psd")),
        ]);
        let recorded = BTreeMap::from([("a.psd".to_owned(), executable("a.psd"))]);
        let paths = BTreeSet::from(["a.psd".to_owned(), "new.psd".to_owned()]);

        put_back(&mut entries, &recorded, &paths);

        assert_eq!(entries["a.psd"], executable("a.psd"));
        assert!(!entries.contains_key("new.psd"));
    }

    /// A conflict copy a reset covers is removed beside its path, and the rest stay.
    #[test]
    fn settling_conflicts_drops_the_ones_a_reset_covers_and_keeps_the_rest() {
        let root = scratch("settle");
        let covered = "renders/b.psd";
        let kept = "other.psd";
        std::fs::create_dir_all(root.join("renders")).unwrap();
        let covered_copy = root.join(conflict_copy(covered));
        let kept_copy = root.join(conflict_copy(kept));
        std::fs::write(&covered_copy, b"theirs").unwrap();
        std::fs::write(&kept_copy, b"theirs").unwrap();

        let remaining = settle_conflicts(
            vec![covered.to_owned(), kept.to_owned()],
            &BTreeSet::from(["renders".to_owned()]),
            &root,
        );

        assert_eq!(remaining, [kept.to_owned()]);
        assert!(!covered_copy.exists());
        assert!(kept_copy.exists());
    }

    /// A conflict copy is removed when it is there, and its absence is not a failure.
    #[test]
    fn a_conflict_copy_is_removed_beside_its_path() {
        let root = scratch("remove");
        let copy = root.join(conflict_copy("a.psd"));
        std::fs::write(&copy, b"theirs").unwrap();

        remove_conflict(&root, "a.psd");
        assert!(!copy.exists());

        remove_conflict(&root, "a.psd");
    }

    /// A reset answer hands over whether it was whole, how many paths, and each in order.
    #[test]
    fn a_reset_answer_hands_over_the_paths_it_was_given() {
        let answer = WriteTreeReset {
            whole: false,
            paths: vec!["a.psd".to_owned(), "renders".to_owned()],
        };

        assert!(!answer.whole());
        assert_eq!(answer.path_count(), 2);
        assert_eq!(answer.path_at(0), "a.psd");
        assert_eq!(answer.path_at(1), "renders");
        assert_eq!(answer.path_at(2), "");
    }
}

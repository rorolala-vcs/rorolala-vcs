//! Staging the removal of work from a tree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rorolala_layout::Layout;
use rorolala_storage::{Lockable as _, StorageSet, key_of_file};
use rorolala_tree::{EntryKind, Tree, TreeEntry, TreeStructure, conflict_copy};
use rorolala_utils_lazyffi::{StrSpan, lazyffi};
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;

use crate::write::WriteError;

/// What a removal did.
#[derive(Debug, Clone)]
pub struct Removal {
    /// The paths that left the staged structure, in path order.
    pub paths: Vec<String>,
    /// Whether the work was left where it was.
    pub cached: bool,
}

/// How a removal is told to act.
#[derive(Debug, Clone, Copy)]
pub struct RemoveOptions {
    /// Leave the work in place.
    pub cached: bool,
    /// Take a directory with what is under it.
    pub recursive: bool,
    /// Drop work that was never staged.
    pub force: bool,
}

/// Stages the removal of `paths` from the tree.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when a path is not one the tree holds, and
/// [`WriteError::Precondition`] with `what` the path when the work there was never staged and
/// `force` is not set.
pub fn remove_paths(
    tree: &Tree,
    layout: &Layout,
    store: &StorageSet,
    index: &VCSIndex,
    cwd: &Path,
    paths: &[String],
    options: RemoveOptions,
) -> Result<Removal, WriteError> {
    let staging = tree.staging();
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let _guard = runtime
        .block_on(staging.lock())
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

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

    let mut entries: BTreeMap<String, TreeEntry> = base
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.clone()))
        .collect();
    let mut conflicts = staging.conflicts().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;

    let root = tree.root().to_path_buf();
    let targets = collect_targets(&entries, &root, cwd, paths, options.recursive)?;
    apply_targets(&mut entries, &mut conflicts, &root, &targets, options)?;

    let mut checkout = crate::write::checkout::Checkout::open(layout, tree.root());
    for path in &targets {
        checkout.forget(path)?;
    }
    checkout.finish()?;

    prune_empty_directories(&mut entries);

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

    Ok(Removal {
        paths: targets,
        cached: options.cached,
    })
}

/// The paths a removal is about, as the tree writes them.
fn collect_targets(
    entries: &BTreeMap<String, TreeEntry>,
    root: &Path,
    cwd: &Path,
    paths: &[String],
    recursive: bool,
) -> Result<Vec<String>, WriteError> {
    let mut targets: Vec<String> = Vec::new();

    for given in paths {
        let resolved = if Path::new(given).is_absolute() {
            PathBuf::from(given)
        } else {
            cwd.join(given)
        };
        let Some(relative) = relative_to(root, &resolved) else {
            return Err(WriteError::InvalidPath {
                path: given.clone(),
                reason: "the path is outside the tree".to_owned(),
            });
        };

        match entries.get(&relative) {
            Some(entry) if entry.kind() == EntryKind::Directory => {
                if !recursive || holds_tree_or_data(root, &relative) {
                    return Err(WriteError::InvalidPath {
                        path: given.clone(),
                        reason: "the directory cannot be taken".to_owned(),
                    });
                }

                targets.extend(
                    entries
                        .keys()
                        .filter(|path| under(&relative, path))
                        .cloned(),
                );
            }
            Some(_) => targets.push(relative),
            None => {
                return Err(WriteError::InvalidPath {
                    path: given.clone(),
                    reason: "the tree holds no such path".to_owned(),
                });
            }
        }
    }

    targets.sort_unstable();
    targets.dedup();

    Ok(targets)
}

/// Takes the target paths out of the staged structure and, unless cached, the work.
fn apply_targets(
    entries: &mut BTreeMap<String, TreeEntry>,
    conflicts: &mut Vec<String>,
    root: &Path,
    targets: &[String],
    options: RemoveOptions,
) -> Result<(), WriteError> {
    for path in targets {
        let Some(entry) = entries.get(path).cloned() else {
            continue;
        };

        if !options.cached
            && entry.kind() == EntryKind::File
            && !options.force
            && let Some(storage) = entry.storage()
        {
            let disk = root.join(path);
            match key_of_file(&disk) {
                Ok(key) if key.digest() == storage.digest() => {}
                Ok(_) => {
                    return Err(WriteError::Precondition {
                        what: path.clone(),
                        reason: "dirty".to_owned(),
                    });
                }
                Err(_) => {}
            }
        }

        entries.remove(path);
        if let Some(at) = conflicts.iter().position(|held| held == path) {
            let _ = conflicts.remove(at);
            let _ = std::fs::remove_file(root.join(conflict_copy(path)));
        }

        if !options.cached {
            take_work(root, path, entry.kind());
        }
    }

    Ok(())
}

/// Stages the removal of paths from the tree a Workspace holds.
///
/// # Errors
///
/// Returns [`WriteError::NoStore`] or [`WriteError::NoIndex`] when either is missing, and the
/// failures [`remove_paths`] names.
pub fn remove_paths_for_workspace(
    workspace: &Workspace,
    directory: &Path,
    target: &str,
    paths: &[String],
    options: RemoveOptions,
) -> Result<Removal, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let set = workspace.storage_set()?;
    let index = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let at = if target.is_empty() {
        directory.to_path_buf()
    } else {
        let target = Path::new(target);
        if target.is_absolute() {
            target.to_path_buf()
        } else {
            directory.join(target)
        }
    };
    let tree = Tree::locate(workspace.get_root(), &at).map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;

    remove_paths(&tree, &layout, &set, &index, directory, paths, options)
}

/// Stages the removal of paths from the tree a directory is in.
///
/// # FFI
///
/// A `RolaWriteTreeRemove *` the caller releases with `free_rola_write_tree_remove`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `paths` is a
/// `RolaStrSpan` of the paths to remove, and `target` is the tree's path, empty for the directory
/// itself.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`remove_paths_for_workspace`] names.
#[lazyffi(export = rola_write_tree_remove_paths)]
pub fn remove_paths_at(
    directory: &Path,
    target: &str,
    paths: StrSpan,
    cached: bool,
    recursive: bool,
    force: bool,
) -> Result<WriteTreeRemove, WriteError> {
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
    let removal = remove_paths_for_workspace(
        &workspace,
        &tree_root,
        "",
        &paths,
        RemoveOptions {
            cached,
            recursive,
            force,
        },
    )?;

    Ok(WriteTreeRemove::of(&removal))
}

/// What a tree removal staged, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaWriteTreeRemove *` the caller releases with `free_rola_write_tree_remove`.
/// `_path_count` and `_path_at` answer the paths that left the staged structure, each a `char *`
/// the caller releases with `free_string`, and `_cached` says whether the work was left where it
/// was.
#[lazyffi(export = RolaWriteTreeRemove)]
#[derive(Debug, Clone, Default)]
pub struct WriteTreeRemove {
    /// The paths that left the staged structure, in path order.
    paths: Vec<String>,
    /// Whether the work was left where it was.
    cached: bool,
}

impl WriteTreeRemove {
    /// The answer a removal is handed over as.
    fn of(removal: &Removal) -> Self {
        Self {
            paths: removal.paths.clone(),
            cached: removal.cached,
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteTreeRemove {
    /// How many paths left the staged structure.
    #[must_use]
    pub const fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// One path that left the staged structure, or an empty string past the end.
    #[must_use]
    pub fn path_at(&self, index: usize) -> String {
        self.paths.get(index).cloned().unwrap_or_default()
    }

    /// Whether the work was left where it was.
    #[must_use]
    pub const fn cached(&self) -> bool {
        self.cached
    }
}

/// `path` inside `root`, or nothing when it is not.
fn relative_to(root: &Path, path: &Path) -> Option<String> {
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
    path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether the directory at `path` holds a tree of its own or the Workspace's data.
fn holds_tree_or_data(root: &Path, path: &str) -> bool {
    let disk = root.join(path);

    disk.join(".rola-tree").is_dir()
        || disk.join(".rola").is_dir()
        || std::fs::read_dir(&disk).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                entry.path().join(".rola-tree").is_dir() || entry.path().join(".rola").is_dir()
            })
        })
}

/// Deletes `path` under `root`, when it is there.
fn take_work(root: &Path, path: &str, kind: EntryKind) {
    let disk = root.join(path);

    match kind {
        EntryKind::Directory => {
            let _ = std::fs::remove_dir(&disk);
        }
        EntryKind::File | EntryKind::Symlink => {
            let _ = std::fs::remove_file(&disk);
            prune_empty(root, disk.parent());
        }
    }
}

/// Removes the directories a file's removal left empty, up to but not including the tree's root.
pub(crate) fn prune_empty(root: &Path, mut at: Option<&Path>) {
    while let Some(directory) = at {
        if directory == root
            || !directory.starts_with(root)
            || std::fs::remove_dir(directory).is_err()
        {
            break;
        }

        at = directory.parent();
    }
}

/// Takes out the directory entries a removal left with nothing under them.
pub(crate) fn prune_empty_directories(entries: &mut BTreeMap<String, TreeEntry>) {
    let directories: Vec<String> = entries
        .iter()
        .filter(|(_, entry)| entry.kind() == EntryKind::Directory)
        .map(|(path, _)| path.clone())
        .collect();

    for directory in directories {
        if !entries
            .keys()
            .any(|path| path != &directory && under(&directory, path))
        {
            entries.remove(&directory);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use rorolala_layout::Layout;
    use rorolala_storage::{Key, RorolalaStorage, StorageSet};
    use rorolala_tree::{FileMode, Tree, TreeEntry, TreeStructure};
    use rorolala_utils_lazyffi::StrSpan;
    use rorolala_utils_location::Locate as _;
    use rorolala_vcs::VCSIndex;
    use rorolala_workspace::Workspace;

    use super::{RemoveOptions, collect_targets, remove_paths, remove_paths_at};
    use crate::write::WriteError;

    #[test]
    fn a_staged_file_is_taken_from_the_structure_and_the_work() {
        let dir = std::env::temp_dir().join(format!("rola-write-rm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::write(dir.join("tree/one.txt"), b"one").unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layout = Layout::open(dir.join(".rola/layout")).unwrap();
        let store = RorolalaStorage::at(dir.join(".rola/storage")).unwrap();
        let index = VCSIndex::create(dir.join(".rola/index"));
        let tree = Tree::locate(workspace.get_root(), &dir.join("tree")).unwrap();
        let key = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.write_object(b"one", rorolala_storage::Codec::Raw))
            .unwrap();
        tree.staging()
            .write_structure(
                &TreeStructure::build(
                    None,
                    [TreeEntry::file(
                        "one.txt",
                        rorolala_tree::FileMode::Regular,
                        key,
                    )],
                )
                .unwrap(),
            )
            .unwrap();

        let removal = remove_paths(
            &tree,
            &layout,
            &StorageSet::sole(store),
            &index,
            &dir.join("tree"),
            &["one.txt".to_owned()],
            RemoveOptions {
                cached: false,
                recursive: false,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(removal.paths, vec!["one.txt"]);
        assert!(!dir.join("tree/one.txt").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_outside_or_unknown_to_the_tree_is_refused() {
        let root = std::env::temp_dir().join("rola-tree-remove-targets");
        let cwd = root.clone();
        let mut entries = BTreeMap::new();
        entries.insert(
            "a.txt".to_owned(),
            TreeEntry::file("a.txt", FileMode::Regular, Key::new([1; 32])),
        );
        entries.insert("renders".to_owned(), TreeEntry::directory("renders"));
        entries.insert(
            "renders/b.psd".to_owned(),
            TreeEntry::file("renders/b.psd", FileMode::Regular, Key::new([2; 32])),
        );

        assert_eq!(
            collect_targets(&entries, &root, &cwd, &["a.txt".to_owned()], false).unwrap(),
            vec!["a.txt"]
        );
        assert_eq!(
            collect_targets(&entries, &root, &cwd, &["renders".to_owned()], true).unwrap(),
            vec!["renders", "renders/b.psd"]
        );
        assert!(matches!(
            collect_targets(&entries, &root, &cwd, &["nothing".to_owned()], false),
            Err(WriteError::InvalidPath { .. })
        ));
        assert!(matches!(
            collect_targets(&entries, &root, &cwd, &["renders".to_owned()], false),
            Err(WriteError::InvalidPath { .. })
        ));

        let outside = std::env::temp_dir().join("rola-tree-remove-elsewhere");
        assert!(matches!(
            collect_targets(&entries, &root, &outside, &["a.txt".to_owned()], false),
            Err(WriteError::InvalidPath { .. })
        ));
    }

    /// A staged tree whose work is `disk`, with one file `one.txt` recorded.
    fn staged_tree(label: &str, disk: &[u8]) -> (std::path::PathBuf, Tree) {
        let dir = std::env::temp_dir().join(format!(
            "rola-cov-tree-remove-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::write(dir.join("tree/one.txt"), disk).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let store = workspace.get_current_rola_storage().unwrap();
        let key = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.write_object(b"one", rorolala_storage::Codec::Raw))
            .unwrap();
        let tree = Tree::locate(workspace.get_root(), &dir.join("tree")).unwrap();
        tree.staging()
            .write_structure(
                &TreeStructure::build(None, [TreeEntry::file("one.txt", FileMode::Regular, key)])
                    .unwrap(),
            )
            .unwrap();

        (dir, tree)
    }

    /// A cached removal takes the path out of the structure and leaves the work where it was.
    #[test]
    fn a_cached_removal_through_a_directory_leaves_the_work() {
        let (dir, _tree) = staged_tree("cached", b"one");

        let answer = remove_paths_at(
            &dir.join("tree"),
            "",
            StrSpan::from(vec!["one.txt".to_owned()]),
            true,
            false,
            false,
        )
        .unwrap();

        assert_eq!(answer.path_count(), 1);
        assert_eq!(answer.path_at(0), "one.txt");
        assert_eq!(answer.path_at(1), "");
        assert!(answer.cached());
        assert!(dir.join("tree/one.txt").is_file());

        let _ = fs::remove_dir_all(&dir);
    }

    /// A removal refuses work that changed on disk, and takes it when forced.
    #[test]
    fn a_removal_refuses_dirty_work_and_force_takes_it() {
        let (dir, tree) = staged_tree("dirty", b"changed");
        let workspace = Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        let store = workspace.storage_set().unwrap();
        let index = VCSIndex::locate(&dir).unwrap();
        let options = RemoveOptions {
            cached: false,
            recursive: false,
            force: false,
        };

        assert!(matches!(
            remove_paths(
                &tree,
                &layout,
                &store,
                &index,
                &dir.join("tree"),
                &["one.txt".to_owned()],
                options,
            ),
            Err(WriteError::Precondition { .. })
        ));

        let removal = remove_paths(
            &tree,
            &layout,
            &store,
            &index,
            &dir.join("tree"),
            &["one.txt".to_owned()],
            RemoveOptions {
                force: true,
                ..options
            },
        )
        .unwrap();
        assert_eq!(removal.paths, vec!["one.txt"]);
        assert!(!dir.join("tree/one.txt").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}

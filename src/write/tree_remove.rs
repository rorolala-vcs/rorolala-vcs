//! Staging the removal of work from a tree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rorolala_layout::Layout;
use rorolala_storage::{Lockable as _, RorolalaStorage, key_of_file};
use rorolala_tree::{EntryKind, Tree, TreeEntry, TreeStructure, conflict_copy};
use rorolala_utils_lazyffi::lazyffi;
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
    store: &RorolalaStorage,
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
    let store = workspace
        .get_current_rola_storage()
        .ok_or(WriteError::NoStore)?;
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

    remove_paths(&tree, &layout, &store, &index, directory, paths, options)
}

/// Stages the removal of paths from the tree a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"paths":[...],"cached":false}`,
/// or a `RolaWriteError *` the caller releases with `free_rola_write_error`. `paths` is a JSON array
/// of strings, and `target` is the tree's path, empty for the directory itself.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`remove_paths_for_workspace`] names.
#[lazyffi(export = rola_write_tree_remove_paths)]
pub fn remove_paths_at(
    directory: &Path,
    target: &str,
    paths: &str,
    cached: bool,
    recursive: bool,
    force: bool,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let paths: Vec<String> = serde_json::from_str(paths).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })?;
    let removal = remove_paths_for_workspace(
        &workspace,
        directory,
        target,
        &paths,
        RemoveOptions {
            cached,
            recursive,
            force,
        },
    )?;

    serde_json::to_string(&serde_json::json!({
        "paths": removal.paths,
        "cached": removal.cached,
    }))
    .map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
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
fn prune_empty(root: &Path, mut at: Option<&Path>) {
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
fn prune_empty_directories(entries: &mut BTreeMap<String, TreeEntry>) {
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
    use std::fs;

    use rorolala_layout::Layout;
    use rorolala_storage::RorolalaStorage;
    use rorolala_tree::{Tree, TreeEntry, TreeStructure};
    use rorolala_utils_location::Locate as _;
    use rorolala_vcs::VCSIndex;
    use rorolala_workspace::Workspace;

    use super::{RemoveOptions, remove_paths};

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
            &store,
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
}

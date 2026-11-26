//! Making a directory a tree, and taking the files it held into its first staged structure.

use std::path::{Path, PathBuf};

use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_tree::{FileMode, Staging, TreeEntry, TreeStructure, read_storage};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use uuid::Uuid;

use crate::write::WriteError;

/// What making a tree at a directory would do.
#[derive(Debug, Clone)]
pub struct TreePlan {
    /// The tree's root, which is the directory itself.
    pub root: PathBuf,
    /// The Layout path of the tree, or nothing when it is the Workspace root.
    pub prefix: Option<LayoutPath>,
    /// Whether the tree is the Workspace root.
    pub is_root: bool,
    /// The entries that stop being files, with the version each records.
    pub collapse: Vec<(Uuid, LayoutPath, [u8; 32])>,
}

/// What making a tree did.
#[derive(Debug, Clone)]
pub struct TreeInit {
    /// The tree's path, or `.` for the Workspace root.
    pub path: String,
    /// How many entries stopped being files.
    pub collapsed: usize,
    /// Whether the tree is the Workspace root.
    pub is_root: bool,
}

/// What making a tree did, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaWriteTreeInit *` the caller releases with `free_rola_write_tree_init`. `_path` returns a
/// `char *` the caller releases with `free_string`; `_collapsed` is how many entries stopped being
/// files, and `_is_root` says whether the tree is the Workspace root.
#[lazyffi(export = RolaWriteTreeInit)]
#[derive(Debug, Clone, Default)]
pub struct WriteTreeInit {
    /// The tree's path, or `.` for the Workspace root.
    path: String,
    /// How many entries stopped being files.
    collapsed: u64,
    /// Whether the tree is the Workspace root.
    is_root: bool,
}

impl WriteTreeInit {
    /// The answer a made tree is handed over as.
    fn of(init: &TreeInit) -> Self {
        Self {
            path: init.path.clone(),
            collapsed: u64::try_from(init.collapsed).unwrap_or(u64::MAX),
            is_root: init.is_root,
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteTreeInit {
    /// The tree's path, or `.` for the Workspace root.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// How many entries stopped being files.
    #[must_use]
    pub const fn collapsed(&self) -> u64 {
        self.collapsed
    }

    /// Whether the tree is the Workspace root.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.is_root
    }
}

/// What making a tree of `directory` would do, or nothing when it cannot be one.
#[must_use]
pub fn plan(workspace: &Workspace, layout: &Layout, directory: &Path) -> Option<TreePlan> {
    let workspace_root = normalize(workspace.get_root());
    let directory = normalize(directory);

    if !directory.is_dir() || !directory.starts_with(&workspace_root) {
        return None;
    }
    if directory.join(".rola-tree").is_dir() {
        return None;
    }

    let is_root = directory == workspace_root;
    let prefix = if is_root {
        None
    } else {
        LayoutPath::from_relative(directory.strip_prefix(&workspace_root).ok()?).ok()
    };

    let mut collapse = Vec::new();
    for (id, data) in layout.entries() {
        if data.kind() == EntryKind::Tree {
            continue;
        }

        let Some(path) = layout.path_of(id) else {
            continue;
        };
        if let Some(prefix) = &prefix
            && !(path == *prefix
                || path
                    .as_str()
                    .strip_prefix(prefix.as_str())
                    .is_some_and(|rest| rest.starts_with('/')))
        {
            continue;
        }

        collapse.push((id, path, data.version()));
    }

    Some(TreePlan {
        root: directory,
        prefix,
        is_root,
        collapse,
    })
}

/// What making a tree at a target would do, or an error when it cannot be one.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the directory cannot be a tree, and
/// [`WriteError::Layout`] when the Layout cannot be read.
pub fn plan_at(workspace: &Workspace, cwd: &Path, target: &str) -> Result<TreePlan, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
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

    plan(workspace, &layout, &at).ok_or_else(|| WriteError::InvalidPath {
        path: at.display().to_string(),
        reason: "the directory cannot be a tree".to_owned(),
    })
}

/// Makes the tree a plan describes, taking the files it held into its staged structure.
///
/// `force` says the caller has already agreed to the collapse, or that no entries are collapsing.
///
/// # Errors
///
/// Returns [`WriteError::Precondition`] when entries would collapse and `force` is false, and
/// [`WriteError::Tree`] or [`WriteError::Layout`] when the staging area or the Layout cannot be
/// written.
pub fn init(
    workspace: &Workspace,
    cwd: &Path,
    target: &str,
    force: bool,
) -> Result<TreeInit, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
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
    let plan = plan(workspace, &layout, &at).ok_or_else(|| WriteError::InvalidPath {
        path: at.display().to_string(),
        reason: "the directory cannot be a tree".to_owned(),
    })?;

    if (plan.is_root || !plan.collapse.is_empty()) && !force {
        return Err(WriteError::Precondition {
            what: "tree".to_owned(),
            reason: "confirmation needed".to_owned(),
        });
    }

    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let mut entries: Vec<TreeEntry> = Vec::new();
    let mut removed: Vec<Uuid> = Vec::new();

    for (id, path, version) in &plan.collapse {
        if *version == [0; 32] {
            removed.push(*id);
            continue;
        }

        let storage = runtime
            .block_on(read_storage(&index, version))
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;
        let relative = plan.prefix.as_ref().map_or_else(
            || path.as_str().to_owned(),
            |prefix| {
                path.as_str()
                    .strip_prefix(prefix.as_str())
                    .and_then(|rest| rest.strip_prefix('/'))
                    .unwrap_or_default()
                    .to_owned()
            },
        );
        entries.push(TreeEntry::file(relative, FileMode::Regular, storage));
        removed.push(*id);
    }

    let staging = Staging::new(&plan.root);
    staging.ensure_store().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;

    if !entries.is_empty() {
        let structure = TreeStructure::build(None, entries).map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
        staging
            .write_structure(&structure)
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;
    }

    if !removed.is_empty() {
        let mut batch = layout.batch();
        for id in &removed {
            batch
                .remove_entry(*id)
                .map_err(|error| WriteError::Layout {
                    reason: error.to_string(),
                })?;
        }
        batch.commit().map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })?;
    }

    Ok(TreeInit {
        path: plan
            .prefix
            .as_ref()
            .map_or_else(|| ".".to_owned(), |prefix| prefix.as_str().to_owned()),
        collapsed: removed.len(),
        is_root: plan.is_root,
    })
}

/// Makes the tree a directory is in.
///
/// # FFI
///
/// A `RolaWriteTreeInit *` the caller releases with `free_rola_write_tree_init`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `target` is the tree's
/// path, empty for the directory itself, and `force` says the caller has agreed to the collapse.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`init`] names.
#[lazyffi(export = rola_write_tree_init)]
pub fn init_at(directory: &Path, target: &str, force: bool) -> Result<WriteTreeInit, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let init = init(&workspace, directory, target, force)?;

    Ok(WriteTreeInit::of(&init))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::{WriteTreeInit, init_at, plan, plan_at};
    use crate::write::WriteError;
    use crate::write::index::write_file_version;
    use rorolala_layout::{LayoutPath, MutableData};
    use rorolala_storage::{Codec, RorolalaStorage};
    use rorolala_vcs::{ROOT_VERSION, VCSIndex};
    use uuid::Uuid;

    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-tree-init-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_directory_inside_the_workspace_can_be_a_tree() {
        let dir = scratch("plan");
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        let tree = dir.join("renders");
        fs::create_dir_all(&tree).unwrap();

        let placed = plan(&workspace, &layout, &tree).unwrap();
        assert!(!placed.is_root);
        assert_eq!(placed.prefix.as_ref().unwrap().as_str(), "renders");
        assert_eq!(placed.collapse, Vec::new());

        let root = plan(&workspace, &layout, &dir).unwrap();
        assert!(root.is_root);
        assert!(root.prefix.is_none());

        fs::create_dir_all(tree.join(".rola-tree")).unwrap();
        assert!(plan(&workspace, &layout, &tree).is_none());

        let file = dir.join("a.txt");
        fs::write(&file, b"one").unwrap();
        assert!(plan(&workspace, &layout, &file).is_none());

        let outside = scratch("outside");
        assert!(plan(&workspace, &layout, &outside).is_none());

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn a_relative_target_is_read_from_the_run_directory() {
        let dir = scratch("plan-at");
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::create_dir_all(dir.join("renders")).unwrap();

        let planned = plan_at(&workspace, &dir, "renders").unwrap();
        assert_eq!(planned.prefix.as_ref().unwrap().as_str(), "renders");

        assert!(plan_at(&workspace, &dir, "missing").is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_answer_hands_over_what_it_found() {
        let answer = WriteTreeInit {
            path: "renders".to_owned(),
            collapsed: 2,
            is_root: false,
        };

        assert_eq!(answer.path(), "renders");
        assert_eq!(answer.collapsed(), 2);
        assert!(!answer.is_root());
    }

    #[test]
    fn a_directory_outside_every_workspace_makes_no_tree() {
        let dir = scratch("no-workspace");

        assert!(matches!(
            init_at(&dir, "", false),
            Err(WriteError::NoWorkspace)
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_forced_init_makes_a_tree_and_takes_the_files_in() {
        let dir = scratch("init");
        Workspace::create(&dir).unwrap();
        fs::create_dir_all(dir.join("renders")).unwrap();
        fs::write(dir.join("renders/a.psd"), b"one").unwrap();

        let made = init_at(&dir, "renders", true).unwrap();

        assert_eq!(made.path(), "renders");
        assert!(!made.is_root());
        assert!(dir.join("renders/.rola-tree").is_dir());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unforced_init_of_a_root_or_a_collapse_needs_confirmation() {
        let dir = scratch("unforced");
        Workspace::create(&dir).unwrap();

        assert!(matches!(
            init_at(&dir, "", false),
            Err(WriteError::Precondition { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_forced_init_takes_a_recorded_file_into_the_tree() {
        let dir = scratch("collapse");
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        let index = VCSIndex::locate(&dir).unwrap();
        let store = RorolalaStorage::at(dir.join(".rola/storage")).unwrap();
        let content = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.write_object(b"one", Codec::Raw))
            .unwrap();
        let written = write_file_version(
            &index,
            &content,
            &[0; 32],
            ROOT_VERSION,
            None,
            "alice",
            "first",
        )
        .unwrap();
        layout
            .create_entry(
                Uuid::from_u128(1),
                MutableData::new(None, *written.version.digest(), ""),
            )
            .unwrap();
        layout
            .create_path(
                &LayoutPath::new("renders/a.psd").unwrap(),
                Uuid::from_u128(1),
            )
            .unwrap();
        fs::create_dir_all(dir.join("renders")).unwrap();
        fs::write(dir.join("renders/a.psd"), b"one").unwrap();
        fs::write(dir.join("renders/b.psd"), b"two").unwrap();

        let made = init_at(&dir, "renders", true).unwrap();

        assert_eq!(made.collapsed(), 1);
        assert_eq!(made.path(), "renders");
        assert!(dir.join("renders/.rola-tree").is_dir());

        let _ = fs::remove_dir_all(&dir);
    }
}

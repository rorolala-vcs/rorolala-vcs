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

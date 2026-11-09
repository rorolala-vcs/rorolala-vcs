//! A tree as the rest of Rola meets it: a directory with a version of its own.

use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use rorolala_errors::Failure as _;
use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_storage::{Key, RorolalaStorage};
use rorolala_utils_location::normalize;
use rorolala_vcs::VCSIndex;
use uuid::Uuid;

use crate::TreeError;
use crate::baseline::Baseline;
use crate::consts::{CONFLICT_MARK, DATA, ENTRY_ID, MARK};
use crate::files;
use crate::rules::TreeRules;
use crate::scan::{Scanned, scan};
use crate::staging::Staging;
use crate::status::{TreeStatus, compare};
use crate::structure::TreeStructure;

/// One recorded version of a tree: its version hash, and the structure that version names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The hash of the version the Layout names.
    pub version: [u8; 32],

    /// The structure that version names.
    pub structure: TreeStructure,
}

/// A directory versioned as one file.
#[derive(Debug, Clone)]
pub struct Tree {
    /// The root of the Workspace the tree sits in.
    workspace_root: PathBuf,

    /// The directory that is the tree.
    root: PathBuf,
}

impl Tree {
    /// The tree that holds `target`, inside the Workspace at `workspace_root`.
    ///
    /// `target` may be the tree's root, a directory or file inside it, or a path that is gone; the
    /// nearest directory above it that carries a tree mark is the tree. A path with no tree above
    /// it, or one outside the Workspace, is refused rather than answered with nothing.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NotATree`] when nothing above `target` is a tree, and
    /// [`TreeError::OutsideWorkspace`] when the tree found is not inside `workspace_root`.
    pub fn locate(workspace_root: &Path, target: &Path) -> Result<Self, TreeError> {
        let workspace_root = normalize(workspace_root);
        let target = normalize(target);

        let mut at = if target.is_dir() {
            target.clone()
        } else {
            target
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| TreeError::NotATree(target.clone()))?
        };

        loop {
            if at.join(MARK).is_dir() {
                if !at.starts_with(&workspace_root) {
                    return Err(TreeError::OutsideWorkspace(at));
                }

                return Self::open(&workspace_root, &at);
            }

            if at == workspace_root {
                return Err(TreeError::NotATree(target));
            }

            match at.parent() {
                Some(parent) => at = parent.to_path_buf(),
                None => return Err(TreeError::NotATree(target)),
            }

            if !at.starts_with(&workspace_root) {
                return Err(TreeError::OutsideWorkspace(target));
            }
        }
    }

    /// The tree rooted at `root`, which must carry a tree mark.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NotATree`] when `root` is not a tree, and
    /// [`TreeError::OutsideWorkspace`] when it is not inside the Workspace.
    pub fn open(workspace_root: &Path, root: &Path) -> Result<Self, TreeError> {
        let workspace_root = normalize(workspace_root);
        let root = normalize(root);

        if !root.join(MARK).is_dir() {
            return Err(TreeError::NotATree(root));
        }
        if !root.starts_with(&workspace_root) {
            return Err(TreeError::OutsideWorkspace(root));
        }

        Ok(Self {
            workspace_root,
            root,
        })
    }

    /// The directory that is the tree.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    /// The root of the Workspace the tree sits in.
    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        self.workspace_root.as_path()
    }

    /// The path the tree is recorded at in the Layout.
    ///
    /// A tree at the Workspace root is at the reserved root path; every other tree is at the path
    /// from the Workspace root.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::OutsideWorkspace`] when the tree is not under its Workspace.
    pub fn layout_path(&self) -> Result<LayoutPath, TreeError> {
        let relative = self
            .root
            .strip_prefix(&self.workspace_root)
            .map_err(|_| TreeError::OutsideWorkspace(self.root.clone()))?;

        if relative.as_os_str().is_empty() {
            return Ok(LayoutPath::root());
        }

        LayoutPath::from_relative(relative).map_err(|error| TreeError::Layout(error.to_string()))
    }

    /// The `Uuid` the tree is recorded by, when the Layout records one.
    ///
    /// # Errors
    ///
    /// Returns what [`layout_path`](Self::layout_path) does.
    pub fn recorded_id(&self, layout: &Layout) -> Result<Option<Uuid>, TreeError> {
        let path = self.layout_path()?;

        Ok(layout.id_of(&path))
    }

    /// The recorded version and structure of the tree, when the Layout records one.
    ///
    /// The baseline is read first: when it is the copy of the version the Layout names, the store is
    /// not opened at all. What is not here is read from the version the Layout names, through the
    /// variant it holds.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Layout`] when the Layout names a file rather than a tree,
    /// [`TreeError::Index`] when the version cannot be read, [`TreeError::Store`] when its content
    /// cannot be read, and [`TreeError::Malformed`] when that content is not a structure.
    pub async fn recorded(
        &self,
        layout: &Layout,
        index: &VCSIndex,
        store: &RorolalaStorage,
    ) -> Result<Option<Recorded>, TreeError> {
        let path = self.layout_path()?;
        let Some(id) = layout.id_of(&path) else {
            return Ok(None);
        };

        self.recorded_of(layout, id, index, store).await
    }

    /// The recorded version and structure of the entry `id`.
    ///
    /// # Errors
    ///
    /// Returns what [`recorded`](Self::recorded) does.
    pub async fn recorded_of(
        &self,
        layout: &Layout,
        id: Uuid,
        index: &VCSIndex,
        store: &RorolalaStorage,
    ) -> Result<Option<Recorded>, TreeError> {
        let Some(data) = layout.entry(id) else {
            return Ok(None);
        };

        if data.kind() != EntryKind::Tree {
            return Err(TreeError::Layout(format!(
                "{} is recorded as a file rather than as a tree",
                self.root.display()
            )));
        }

        let version = data.version();
        if version == [0; 32] {
            return Ok(None);
        }

        let baseline = self.baseline();
        if baseline.fresh_for(&version)
            && let Some(structure) = baseline.tree()?
        {
            return Ok(Some(Recorded { version, structure }));
        }

        let structure = read_structure(index, store, &version).await?;

        Ok(Some(Recorded { version, structure }))
    }

    /// The structure the tree records: the baseline copy of it when that is what the Layout names,
    /// the recorded structure otherwise, and nothing when the tree is not recorded yet.
    ///
    /// # Errors
    ///
    /// Returns what [`recorded`](Self::recorded) does, and [`TreeError::Io`] when the baseline
    /// cannot be read.
    pub async fn recorded_structure(
        &self,
        layout: &Layout,
        index: &VCSIndex,
        store: &RorolalaStorage,
    ) -> Result<TreeStructure, TreeError> {
        let recorded = self.recorded(layout, index, store).await?;

        if let Some(recorded) = &recorded {
            return Ok(recorded.structure.clone());
        }

        Ok(self.baseline().tree()?.unwrap_or_else(TreeStructure::empty))
    }

    /// The staging area of the tree.
    #[must_use]
    pub fn staging(&self) -> Staging {
        Staging::new(&self.root)
    }

    /// Whether the tree holds work a commit would record.
    ///
    /// What makes a version is a staged structure, a staged description or a conflict settled by the
    /// commit's refusal. It is what a sync plan asks before it sends a tree: a tree with nothing
    /// pending has nothing a commit would make.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Staging`] when the staging area cannot be read.
    pub fn pending(&self) -> Result<bool, TreeError> {
        let staging = self.staging();

        Ok(staging.structure()?.is_some()
            || staging.description()?.is_some()
            || !staging.conflicts()?.is_empty())
    }

    /// The baseline of the tree.
    #[must_use]
    pub fn baseline(&self) -> Baseline {
        Baseline::new(&self.root)
    }

    /// The Layout entry id the tree keeps in its mark, when one is written there and reads.
    ///
    /// The id is the one the Layout names the tree by, written down locally when the tree is
    /// recorded. A file that does not read as a `Uuid` is answered as none rather than as a
    /// failure: it is a local note, and a reading that cannot use it still has the tree.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the file is there and cannot be read.
    pub fn local_id(&self) -> Result<Option<Uuid>, TreeError> {
        let Some(text) = files::read_text(&self.local_id_path())? else {
            return Ok(None);
        };

        Ok(Uuid::parse_str(text.trim()).ok())
    }

    /// Writes `id` down in the tree's mark as the Layout entry it is recorded at.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the file cannot be written.
    pub fn write_local_id(&self, id: Uuid) -> Result<(), TreeError> {
        files::write(&self.local_id_path(), id.to_string().as_bytes())
    }

    /// The rules the tree's work is read by.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the tree's own rules file cannot be read.
    pub fn rules(&self, workspace: IgnoreRuleSet) -> Result<TreeRules, TreeError> {
        TreeRules::load(&self.root, workspace)
    }

    /// Reads the work the tree holds, as the structure it would be recorded as.
    ///
    /// # Errors
    ///
    /// Returns what [`scan`] does.
    pub fn scan(&self, rules: &TreeRules, include_ignored: bool) -> Result<Scanned, TreeError> {
        let previous = self.baseline().entries();

        scan(&self.root, rules, &previous, include_ignored)
    }

    /// How the tree stands: what is staged, what changed and was not staged, and what is in conflict.
    ///
    /// # Errors
    ///
    /// Returns what [`recorded`](Self::recorded) and [`scan`](Self::scan) do, and
    /// [`TreeError::Staging`] when the staging area cannot be read.
    pub async fn status(
        &self,
        layout: &Layout,
        index: &VCSIndex,
        store: &RorolalaStorage,
        workspace: IgnoreRuleSet,
        include_ignored: bool,
    ) -> Result<TreeStatus, TreeError> {
        let recorded = self.recorded_structure(layout, index, store).await?;

        let staging = self.staging();
        let staged = staging.structure()?.unwrap_or_else(|| recorded.clone());
        let conflicts = staging.conflicts()?;

        let rules = if include_ignored {
            TreeRules::empty(workspace)
        } else {
            self.rules(workspace)?
        };
        let scanned = self.scan(&rules, include_ignored)?;

        // The digest cache is the reading's memory rather than a record of the work, so a cache
        // that cannot be written leaves the reading it came from standing.
        let _ = self.baseline().write_entries(&scanned.cache);

        Ok(compare(&recorded, &staged, &scanned.structure, &conflicts))
    }

    /// Whether `path` is a conflict copy rather than work.
    #[must_use]
    pub fn is_conflict_copy(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(CONFLICT_MARK))
    }

    /// The file the tree's Layout entry id is kept in.
    fn local_id_path(&self) -> PathBuf {
        self.root.join(MARK).join(ENTRY_ID)
    }

    /// Whether `path` is the Workspace's own data rather than work.
    #[must_use]
    pub fn is_data_dir(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == DATA || name == MARK)
    }
}

/// The structure the version `version` names, read through its variant.
///
/// # Errors
///
/// Returns [`TreeError::Index`] when the version or its variant cannot be read, and
/// [`TreeError::Store`] when the content the variant names cannot be, and [`TreeError::Malformed`]
/// when that content is not a structure.
pub async fn read_structure(
    index: &VCSIndex,
    store: &RorolalaStorage,
    version: &[u8; 32],
) -> Result<TreeStructure, TreeError> {
    let object = index
        .read(Key::new(*version))
        .await
        .map_err(|error| TreeError::Index(error.reason()))?;
    let version = object
        .expect_version()
        .map_err(|error| TreeError::Index(error.to_string()))?;

    let object = index
        .read(Key::new(*version.variant()))
        .await
        .map_err(|error| TreeError::Index(error.reason()))?;
    let variant = object
        .expect_variant()
        .map_err(|error| TreeError::Index(error.to_string()))?;

    let bytes = store
        .read_object(&Key::new(*variant.storage_hash()))
        .await
        .map_err(|error| TreeError::Store(error.to_string()))?;

    TreeStructure::decode(&bytes)
}

/// The content key the version `version` stands for, read through its variant.
///
/// It is what a tree built from already recorded files needs: the paths a Workspace recorded as
/// files keep the content they were recorded with when they become one tree's entries, and that is
/// the content key the version's variant names.
///
/// # Errors
///
/// Returns [`TreeError::Index`] when the version or its variant cannot be read.
pub async fn read_storage(index: &VCSIndex, version: &[u8; 32]) -> Result<Key, TreeError> {
    let object = index
        .read(Key::new(*version))
        .await
        .map_err(|error| TreeError::Index(error.reason()))?;
    let version = object
        .expect_version()
        .map_err(|error| TreeError::Index(error.to_string()))?;

    let object = index
        .read(Key::new(*version.variant()))
        .await
        .map_err(|error| TreeError::Index(error.reason()))?;
    let variant = object
        .expect_variant()
        .map_err(|error| TreeError::Index(error.to_string()))?;

    Ok(Key::new(*variant.storage_hash()))
}

/// The description object a version's structure names, if it names one.
///
/// # Errors
///
/// Returns [`TreeError::Store`] when the description content cannot be read, and
/// [`TreeError::Malformed`] when it is not UTF-8.
pub async fn read_description(
    store: &RorolalaStorage,
    structure: &TreeStructure,
) -> Result<Option<String>, TreeError> {
    let Some(key) = structure.description() else {
        return Ok(None);
    };

    let bytes = store
        .read_object(key)
        .await
        .map_err(|error| TreeError::Store(error.to_string()))?;

    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| TreeError::Malformed)
}

/// The version hash a hex name stands for, for a target a person wrote.
///
/// # Errors
///
/// Returns [`TreeError::Layout`] when the text is not a hash.
pub fn version_of(text: &str) -> Result<[u8; 32], TreeError> {
    let key = Key::from_str(text).map_err(|error| TreeError::Layout(error.to_string()))?;

    Ok(*key.digest())
}

/// Whether there is a tree at `directory`.
#[must_use]
pub fn marked(directory: &Path) -> bool {
    directory.join(MARK).is_dir()
}

/// The files a tree keeps for itself that a caller may need to leave out of a loop.
#[must_use]
pub fn is_control_file(path: &Path) -> bool {
    Tree::is_data_dir(path) || Tree::is_conflict_copy(path)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rola-tree-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");

        dir
    }

    /// The id a tree keeps is the Layout entry's, and a file that is not one is no id.
    #[test]
    fn a_tree_keeps_the_id_of_the_entry_it_is_recorded_at() {
        let root = scratch("id");
        fs::create_dir_all(root.join(crate::consts::MARK)).expect("a mark");
        let tree = Tree::open(&root, &root).expect("a tree");

        assert_eq!(tree.local_id().expect("a reading"), None);
        tree.write_local_id(Uuid::from_u128(7)).expect("an id");
        assert_eq!(
            tree.local_id().expect("a reading"),
            Some(Uuid::from_u128(7))
        );

        fs::write(
            root.join(crate::consts::MARK).join(crate::consts::ENTRY_ID),
            "not an id",
        )
        .expect("a file that is not an id");
        assert_eq!(tree.local_id().expect("a reading"), None);

        let _ = fs::remove_dir_all(&root);
    }

    /// A tree with nothing staged has nothing a commit would record.
    #[test]
    fn a_new_tree_has_nothing_pending() {
        let root = scratch("pending");
        fs::create_dir_all(root.join(crate::consts::MARK)).expect("a mark");

        let tree = Tree::open(&root, &root).expect("a tree");
        assert!(!tree.pending().expect("a reading"));

        let structure = TreeStructure::build(None, Vec::new()).expect("an empty structure");
        tree.staging()
            .write_structure(&structure)
            .expect("a staged structure");
        assert!(tree.pending().expect("a reading"));

        tree.staging().clear().expect("a cleared staging area");
        assert!(!tree.pending().expect("a reading"));

        let _ = fs::remove_dir_all(&root);
    }
}

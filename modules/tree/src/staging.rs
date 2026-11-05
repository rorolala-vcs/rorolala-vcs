//! What a tree has staged: the content and the structure a commit would make of it.

use std::fs;
use std::path::{Path, PathBuf};

use rorolala_storage::{LockError, Lockable, LockingGuard, RorolalaStorage, store_file};

use crate::TreeError;
use crate::consts::{CONFLICTS, DESCRIPTION, LOCK, MARK, STAGED_TREE, STAGING};
use crate::files;
use crate::scan::mode_of;
use crate::structure::{TreeEntry, TreeStructure};

/// The staging area of the tree at one root.
///
/// The content is a store of its own under [`MARK`](crate::consts::MARK), so what is staged and
/// then dropped leaves nothing behind in the Workspace's store. The structure is one file beside
/// it: the whole of what a commit would record, rewritten as paths are added and removed.
#[derive(Debug, Clone)]
pub struct Staging {
    /// The root of the tree this staging belongs to.
    root: PathBuf,
}

impl Staging {
    /// The staging area of the tree at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// The store staged content is kept in, when one has been made.
    #[must_use]
    pub fn store(&self) -> Option<RorolalaStorage> {
        RorolalaStorage::at(self.directory())
    }

    /// The store staged content is kept in, made where it is not there yet.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the directory cannot be made, and [`TreeError::Staging`] when
    /// the store cannot be made in it.
    pub fn ensure_store(&self) -> Result<RorolalaStorage, TreeError> {
        fs::create_dir_all(self.directory())?;
        let store = RorolalaStorage::create(self.directory());

        // A store that could not be made answers nothing when it is looked for, which is what
        // tells a made one from a directory that only looks like one.
        if RorolalaStorage::at(self.directory()).is_none() {
            return Err(TreeError::Staging(format!(
                "the staging store at {} could not be made",
                self.directory().display()
            )));
        }

        Ok(store)
    }

    /// The structure staged here, when anything is.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Malformed`] when the staged structure does not read, and
    /// [`TreeError::Io`] when it cannot be read.
    pub fn structure(&self) -> Result<Option<TreeStructure>, TreeError> {
        match files::read(&self.structure_path())? {
            Some(bytes) => Ok(Some(TreeStructure::decode(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Writes `structure` down as what is staged.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the structure cannot be written.
    pub fn write_structure(&self, structure: &TreeStructure) -> Result<(), TreeError> {
        self.ensure_store()?;
        files::write(&self.structure_path(), &structure.encode())
    }

    /// The description staged for the next version, when one is.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the file cannot be read.
    pub fn description(&self) -> Result<Option<String>, TreeError> {
        let text = files::read_text(&self.description_path())?;

        Ok(text.filter(|text| !text.trim().is_empty()))
    }

    /// Stages `text` as the description of the next version, or clears it when it says nothing.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the file cannot be written or removed.
    pub fn write_description(&self, text: Option<&str>) -> Result<(), TreeError> {
        if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
            files::write(&self.description_path(), text.as_bytes())
        } else {
            files::remove(&self.description_path())?;

            Ok(())
        }
    }

    /// The paths of the conflicts left by a merge, in the order they were written.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the file cannot be read.
    pub fn conflicts(&self) -> Result<Vec<String>, TreeError> {
        let Some(text) = files::read_text(&self.conflicts_path())? else {
            return Ok(Vec::new());
        };

        Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Writes the paths a merge left in conflict, or clears the list when it is empty.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the file cannot be written or removed.
    pub fn write_conflicts(&self, paths: &[String]) -> Result<(), TreeError> {
        if paths.is_empty() {
            files::remove(&self.conflicts_path())?;

            return Ok(());
        }

        let mut text = String::new();
        for path in paths {
            text.push_str(path);
            text.push('\n');
        }

        files::write(&self.conflicts_path(), text.as_bytes())
    }

    /// What the next commit would carry: the staged structure, or nothing when nothing is staged.
    ///
    /// # Errors
    ///
    /// Returns what [`structure`](Self::structure) does.
    pub fn merged_with(&self, recorded: TreeStructure) -> Result<TreeStructure, TreeError> {
        Ok(self.structure()?.unwrap_or(recorded))
    }

    /// Hands the file at `disk` into the staging store as the work at `path` would be.
    ///
    /// It is what `tree add` does for one file: the content goes into the staging area's own store,
    /// and what comes back is the entry a structure would hold for it. A file that cannot be read is
    /// an error rather than a skipped path, since a run that named it asked for it to be staged.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the file cannot be read, and [`TreeError::Store`] when its
    /// content cannot be written into the staging store.
    pub async fn stage_file(&self, disk: &Path, path: &str) -> Result<TreeEntry, TreeError> {
        let store = self.ensure_store()?;
        let key = store_file(&store, disk)
            .await
            .map_err(|error| TreeError::Store(error.to_string()))?;
        let meta = fs::metadata(disk)?;

        Ok(TreeEntry::file(path, mode_of(&meta), key))
    }

    /// The entry a symbolic link at `disk` makes, as the work at `path`.
    ///
    /// A link has no content, so nothing is written into the store: what is recorded is its target.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the link cannot be read, and [`TreeError::Path`] when its
    /// target does not read as UTF-8.
    pub fn stage_symlink(&self, disk: &Path, path: &str) -> Result<TreeEntry, TreeError> {
        let target = fs::read_link(disk)?;
        let target = target
            .to_str()
            .ok_or_else(|| TreeError::Path(path.to_owned()))?;

        Ok(TreeEntry::symlink(path, target))
    }

    /// Empties the staging area: its content, its structure, its description and its conflicts.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when any of them cannot be taken away.
    pub fn clear(&self) -> Result<(), TreeError> {
        match fs::remove_dir_all(self.directory()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        files::remove(&self.description_path())?;
        files::remove(&self.conflicts_path())?;

        Ok(())
    }

    /// The directory staged content is kept in.
    #[must_use]
    pub fn directory(&self) -> PathBuf {
        self.root.join(MARK).join(STAGING)
    }

    /// The file the staged structure is kept in.
    #[must_use]
    pub fn structure_path(&self) -> PathBuf {
        self.directory().join(STAGED_TREE)
    }

    /// The file the description is kept in.
    #[must_use]
    pub fn description_path(&self) -> PathBuf {
        self.root.join(MARK).join(DESCRIPTION)
    }

    /// The file the conflict list is kept in.
    #[must_use]
    pub fn conflicts_path(&self) -> PathBuf {
        self.root.join(MARK).join(CONFLICTS)
    }

    /// The lock file a run changing the staging area holds.
    #[must_use]
    pub fn lock_path(&self) -> PathBuf {
        self.root.join(MARK).join(LOCK)
    }
}

impl Lockable for Staging {
    fn lock_path(&self) -> PathBuf {
        Self::lock_path(self)
    }

    async fn lock(&self) -> Result<LockingGuard<Self>, LockError> {
        LockingGuard::acquire(self.clone(), Self::lock_path(self)).await
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::{Key, LockError, Lockable as _};

    use super::Staging;
    use crate::structure::{FileMode, TreeEntry, TreeStructure};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-tree-staging-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".rola-tree")).unwrap();

        dir
    }

    /// A structure with one file in it.
    fn structure() -> TreeStructure {
        TreeStructure::build(
            None,
            [TreeEntry::file(
                "a.psd",
                FileMode::Regular,
                Key::new([2; 32]),
            )],
        )
        .unwrap()
    }

    #[test]
    fn a_staged_structure_is_read_back_whole() {
        let root = scratch("structure");
        let staging = Staging::new(&root);

        assert_eq!(staging.structure().unwrap(), None);

        staging.write_structure(&structure()).unwrap();

        assert_eq!(staging.structure().unwrap(), Some(structure()));
        assert!(staging.store().is_some());
    }

    #[test]
    fn a_description_and_a_conflict_list_are_read_back_and_cleared() {
        let root = scratch("description");
        let staging = Staging::new(&root);

        assert_eq!(staging.description().unwrap(), None);
        staging.write_description(Some("a change")).unwrap();
        assert_eq!(staging.description().unwrap(), Some("a change".to_owned()));

        staging
            .write_conflicts(&["textures/b.psd".to_owned()])
            .unwrap();
        assert_eq!(
            staging.conflicts().unwrap(),
            vec!["textures/b.psd".to_owned()]
        );

        staging.clear().unwrap();

        assert_eq!(staging.description().unwrap(), None);
        assert_eq!(staging.conflicts().unwrap(), Vec::<String>::new());
        assert_eq!(staging.structure().unwrap(), None);
    }

    #[tokio::test]
    async fn the_staging_area_is_locked_while_a_run_holds_it() {
        let root = scratch("lock");
        let staging = Staging::new(&root);

        assert!(!staging.is_locking());
        let guard = staging.lock().await.unwrap();
        assert!(staging.is_locking());
        assert!(matches!(staging.lock().await, Err(LockError::Locked)));

        drop(guard);
        assert!(!staging.is_locking());
    }
}

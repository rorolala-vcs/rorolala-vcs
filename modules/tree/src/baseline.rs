//! What the last recorded version of a tree looked like, kept beside it.

use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use rorolala_storage::Key;
use rorolala_workspace_analyze::Cache;

use crate::TreeError;
use crate::consts::{BASELINE, BASELINE_ENTRIES, BASELINE_TREE, BASELINE_VERSION, MARK};
use crate::files;
use crate::scan::cache_path;
use crate::structure::TreeStructure;

/// The baseline a tree keeps: the structure of the version it was last at, and the digest cache the
/// next reading starts from.
///
/// The structure is a copy rather than a key: with it here, a reading can say what the recorded
/// version holds even when the store no longer can, and a status is not a network trip.
#[derive(Debug, Clone)]
pub struct Baseline {
    /// The root of the tree this baseline belongs to.
    root: PathBuf,
}

impl Baseline {
    /// The baseline of the tree at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// The recorded structure, when a copy of it is here.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Malformed`] when the copy does not read as a structure, and
    /// [`TreeError::Io`] when it cannot be read.
    pub fn tree(&self) -> Result<Option<TreeStructure>, TreeError> {
        match files::read(&self.tree_path())? {
            Some(bytes) => Ok(Some(TreeStructure::decode(&bytes)?)),
            None => Ok(None),
        }
    }

    /// The version the copy belongs to, when the copy says.
    #[must_use]
    pub fn version(&self) -> Option<[u8; 32]> {
        let text = files::read_text(&self.version_path()).ok()??;
        let key = Key::from_str(text.trim()).ok()?;

        Some(*key.digest())
    }

    /// Whether the copy here is the one for `version`.
    #[must_use]
    pub fn fresh_for(&self, version: &[u8; 32]) -> bool {
        self.version().is_some_and(|held| &held == version)
    }

    /// Writes `structure` down as the copy of `version`.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the copy cannot be written.
    pub fn write(&self, structure: &TreeStructure, version: &[u8; 32]) -> Result<(), TreeError> {
        files::write(&self.tree_path(), &structure.encode())?;
        files::write(&self.version_path(), Key::new(*version).hex().as_bytes())?;

        Ok(())
    }

    /// The digest cache the last reading left.
    #[must_use]
    pub fn entries(&self) -> Cache {
        Cache::read(&cache_path(&self.root))
    }

    /// Writes the digest cache a reading made.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Io`] when the cache cannot be written. The reading it came from is
    /// still the reading; a cache that could not be written is one the next reading reads again.
    pub fn write_entries(&self, cache: &Cache) -> Result<(), TreeError> {
        cache
            .write(&cache_path(&self.root))
            .map_err(|error| TreeError::Io(error.to_string()))
    }

    /// Forgets the digest cache, so the next reading reads every file again.
    pub fn clear_entries(&self) {
        let _ = std::fs::remove_file(self.entries_path());
    }

    /// The file the recorded structure copy is kept in.
    #[must_use]
    pub fn tree_path(&self) -> PathBuf {
        self.root().join(BASELINE_TREE)
    }

    /// The file the copy's version is kept in.
    #[must_use]
    pub fn version_path(&self) -> PathBuf {
        self.root().join(BASELINE_VERSION)
    }

    /// The file the digest cache is kept in.
    #[must_use]
    pub fn entries_path(&self) -> PathBuf {
        self.root().join(BASELINE_ENTRIES)
    }

    /// The directory the baseline is kept in.
    #[must_use]
    fn root(&self) -> PathBuf {
        self.root.join(MARK).join(BASELINE)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::LayoutPath;
    use rorolala_storage::Key;
    use rorolala_workspace_analyze::{Cache, Entry};

    use super::Baseline;
    use crate::structure::{FileMode, TreeEntry, TreeStructure};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-tree-baseline-{}-{label}-{}",
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
                Key::new([1; 32]),
            )],
        )
        .unwrap()
    }

    #[test]
    fn a_baseline_written_answers_with_its_structure_and_version() {
        let root = scratch("write");
        let baseline = Baseline::new(&root);
        let version = [7; 32];

        assert!(!baseline.fresh_for(&version));
        assert_eq!(baseline.tree().unwrap(), None);

        baseline.write(&structure(), &version).unwrap();

        assert_eq!(baseline.tree().unwrap(), Some(structure()));
        assert_eq!(baseline.version(), Some(version));
        assert!(baseline.fresh_for(&version));
        assert!(!baseline.fresh_for(&[8; 32]));
    }

    #[test]
    fn a_baseline_keeps_the_digest_cache_of_the_reading() {
        let root = scratch("cache");
        let baseline = Baseline::new(&root);
        let path = LayoutPath::new("a.psd").unwrap();
        let mut cache = Cache::empty();
        cache.insert(path.clone(), Entry::new(1, 0, 2, [3; 32], None));

        assert!(baseline.entries().get(&path).is_none());

        baseline.write_entries(&cache).unwrap();

        assert!(baseline.entries().get(&path).is_some());
    }
}

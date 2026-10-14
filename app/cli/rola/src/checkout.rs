//! Putting a version's content back into the tree.
//!
//! A version is what the Layout names and the store is what it means, so a run that brings a
//! version down — `rola sync`'s pull and `rola checkin` — writes the content at the path and then
//! writes down that the two agree. A path left marked as disagreed would be reported as changed by
//! the very next reading, which is the one thing a pull must not leave behind.
//!
//! What a run reads and writes down is one [cache](librorolala::tree_analyze::Cache), so a run that
//! brings many paths down opens it once, remembers each path in memory, and writes it once — rather
//! than reading and writing the whole cache for every path, which is what a run of many files would
//! otherwise pay.

use std::path::{Path, PathBuf};

use librorolala::layout::{Layout, LayoutPath};
use librorolala::tree_analyze::{Cache, cache_path, entry_of};

/// A run's readings of the tree, written down once when the run is done.
pub struct Checkout {
    /// The Workspace root the paths are relative to.
    root: PathBuf,

    /// The cache file the readings are kept in.
    cache_file: PathBuf,

    /// What the readings are, in memory until [`Checkout::finish`].
    cache: Cache,

    /// Whether anything has been remembered, so a run that brought nothing down writes nothing.
    dirty: bool,
}

impl Checkout {
    /// Opens the readings kept beside `layout` under `root`.
    #[must_use]
    pub fn open(layout: &Layout, root: &Path) -> Self {
        let cache_file = cache_path(root, layout);
        let cache = Cache::read(&cache_file);

        Self {
            root: root.to_path_buf(),
            cache_file,
            cache,
            dirty: false,
        }
    }

    /// Writes down that the path `path` under the root holds what the Layout now names.
    ///
    /// A path that is not a file — one a run did not write, or wrote nowhere — is nothing to
    /// remember.
    ///
    /// # Errors
    ///
    /// Returns a message when the path or the file could not be read.
    pub fn remember(&mut self, path: &str) -> Result<(), String> {
        let path = LayoutPath::new(path).map_err(|error| error.to_string())?;
        let disk = self.root.join(path.to_path_buf());

        if !disk.is_file() {
            return Ok(());
        }

        let entry = entry_of(&disk).map_err(|error| error.to_string())?;
        self.cache.insert(path, entry);
        self.dirty = true;

        Ok(())
    }

    /// Writes the readings down, when there are any to write.
    ///
    /// # Errors
    ///
    /// Returns a message when the cache could not be written.
    pub fn finish(self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }

        self.cache
            .write(&self.cache_file)
            .map_err(|error| error.to_string())
    }
}

/// Writes down that the path `path` under `root` holds what the Layout now names.
///
/// # Errors
///
/// Returns a message when the path, the file, or the cache could not be read or written.
pub fn remember(layout: &Layout, root: &Path, path: &str) -> Result<(), String> {
    let mut checkout = Checkout::open(layout, root);
    checkout.remember(path)?;
    checkout.finish()
}

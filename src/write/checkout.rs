//! Writing down that a path holds what the Layout now names.

use std::path::{Path, PathBuf};

use rorolala_layout::{Layout, LayoutPath};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;
use rorolala_workspace_analyze::{Cache, cache_path, entry_of};

use crate::write::WriteError;

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
    /// A path that is not a file (one a run did not write, or wrote nowhere) is nothing to
    /// remember.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::InvalidPath`] when the path is malformed, and [`WriteError::Io`]
    /// when the file or the cache cannot be read or written.
    pub fn remember(&mut self, path: &str) -> Result<(), WriteError> {
        let path = LayoutPath::new(path).map_err(|error| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
        let disk = self.root.join(path.to_path_buf());

        if !disk.is_file() {
            return Ok(());
        }

        let entry = entry_of(&disk).map_err(|error| WriteError::Io {
            path: disk.display().to_string(),
            reason: error.to_string(),
        })?;
        self.cache.insert(path, entry);
        self.dirty = true;

        Ok(())
    }

    /// Writes the readings down, when there are any to write.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Io`] when the cache cannot be written.
    pub fn finish(self) -> Result<(), WriteError> {
        if !self.dirty {
            return Ok(());
        }

        self.cache
            .write(&self.cache_file)
            .map_err(|error| WriteError::Io {
                path: self.cache_file.display().to_string(),
                reason: error.to_string(),
            })
    }
}

/// Writes down that the path `path` under `root` holds what the Layout now names.
///
/// # Errors
///
/// Returns the failures [`Checkout::remember`] and [`Checkout::finish`] name.
pub fn remember(layout: &Layout, root: &Path, path: &str) -> Result<(), WriteError> {
    let mut checkout = Checkout::open(layout, root);
    checkout.remember(path)?;
    checkout.finish()
}

/// Writes down that a path under a Workspace holds what its Layout now names.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, [`WriteError::Layout`]
/// when its Layout cannot be read, and the failures [`remember`] names.
#[lazyffi(export = rola_write_checkout_remember)]
pub fn remember_at(directory: &Path, path: &str) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    remember(&layout, workspace.get_root(), path)?;

    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::remember;

    #[test]
    fn a_written_path_is_remembered() {
        let dir = std::env::temp_dir().join(format!("rola-write-checkout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        remember(&workspace.layout().unwrap(), &dir, "a.txt").unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}

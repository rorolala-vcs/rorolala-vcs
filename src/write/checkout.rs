//! Writing down that a path holds what the Layout now names.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use rorolala_layout::{Layout, LayoutPath};
use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use rorolala_workspace_analyze::{Cache, cache_path, entry_of};
use serde::Serialize;

use crate::write::{WriteError, layout::set_version, storage::extract_file, tree::relative_to};

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

    /// Forgets the reading of a path the run took out.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::InvalidPath`] when the path is malformed.
    pub fn forget(&mut self, path: &str) -> Result<(), WriteError> {
        let path = LayoutPath::new(path).map_err(|error| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
        if self.cache.remove(&path).is_some() {
            self.dirty = true;
        }

        Ok(())
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

/// Forgets the reading of a path a run took out.
///
/// # Errors
///
/// Returns the failures [`Checkout::forget`] and [`Checkout::finish`] name.
pub fn forget(layout: &Layout, root: &Path, path: &str) -> Result<(), WriteError> {
    let mut checkout = Checkout::open(layout, root);
    checkout.forget(path)?;
    checkout.finish()
}

/// What restoring one path wrote.
#[derive(Debug, Clone, Serialize)]
pub struct Restored {
    /// The Layout entry the path names.
    pub id: uuid::Uuid,
    /// The version the Layout now records.
    pub version: Key,
}

/// Puts the content of `version` at `path`, and writes down that the two agree.
///
/// `path` is relative to `cwd` or absolute, and must be inside the Workspace and name an entry.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Workspace or names no entry,
/// [`WriteError::Precondition`] when the version does not read, [`WriteError::Index`] when the index
/// cannot be read, and [`WriteError::Storage`] when the content cannot be extracted.
pub fn restore(
    workspace: &Workspace,
    cwd: &Path,
    path: &str,
    version: &str,
) -> Result<Restored, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = workspace
        .get_current_rola_storage()
        .ok_or(WriteError::NoStore)?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(workspace.get_root());
    let resolved = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = normalize(&resolved);
    let relative = relative_to(&root, &resolved).ok_or_else(|| WriteError::InvalidPath {
        path: path.to_owned(),
        reason: "the path is outside the Workspace".to_owned(),
    })?;
    let layout_path = LayoutPath::new(&relative).map_err(|error| WriteError::InvalidPath {
        path: path.to_owned(),
        reason: error.to_string(),
    })?;
    let id = layout
        .id_of(&layout_path)
        .ok_or_else(|| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the Layout names no such path".to_owned(),
        })?;
    let target = Key::from_str(version).map_err(|error| WriteError::Precondition {
        what: format!("version `{version}`"),
        reason: error.to_string(),
    })?;
    let named = index
        .read_version(&target.to_string())
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;
    let variant = index
        .read_variant(&Key::new(*named.variant()).to_string())
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;
    let content = Key::new(*variant.storage_hash());
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    runtime.block_on(extract_file(&store, &content, &resolved))?;
    set_version(&layout, id, *target.digest())?;
    remember(&layout, &root, &relative)?;

    Ok(Restored {
        id,
        version: target,
    })
}

/// Puts a version's content back at a path under a Workspace.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`restore`] names.
#[lazyffi(export = rola_write_checkout_restore)]
pub fn restore_at(directory: &Path, path: &str, version: &str) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let restored = restore(&workspace, directory, path, version)?;

    serde_json::to_string(&restored).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Forgets the reading of a path under a Workspace.
///
/// # FFI
///
/// An empty string on success, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, [`WriteError::Layout`]
/// when its Layout cannot be read, and the failures [`forget`] names.
#[lazyffi(export = rola_write_checkout_forget)]
pub fn forget_at(directory: &Path, path: &str) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    forget(&layout, workspace.get_root(), path)?;

    Ok(String::new())
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

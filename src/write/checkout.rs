//! Writing down that a path holds what the Layout now names.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use rorolala_layout::{Layout, LayoutPath};
use rorolala_storage::{Key, StorageSet};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vault::Vault;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use rorolala_workspace_analyze::{Cache, cache_path, entry_of};

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
#[derive(Debug, Clone)]
pub struct Restored {
    /// The Layout entry the path names.
    pub id: uuid::Uuid,
    /// The version the Layout now records.
    pub version: Key,
}

/// What putting content back at a path wrote: the entry, and the version it now records.
///
/// It is the answer of every call that restores a path or aligns one, in a Workspace and in a Vault
/// alike, because each of them settles one entry on one version.
///
/// # FFI
///
/// A `RolaWriteRestore *` the caller releases with `free_rola_write_restore`. Its `_id` and
/// `_version` each return a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaWriteRestore)]
#[derive(Debug, Clone)]
pub struct WriteRestore {
    /// The Layout entry the path names, as its `Uuid`.
    id: String,
    /// The version the Layout now records, as a key.
    version: String,
}

impl WriteRestore {
    /// The answer a settled entry is handed over as.
    pub(crate) fn of(id: uuid::Uuid, version: &Key) -> Self {
        Self {
            id: id.to_string(),
            version: version.to_string(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteRestore {
    /// The Layout entry the path names, as its `Uuid`.
    #[must_use]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// The version the Layout now records, as a key.
    #[must_use]
    pub fn version(&self) -> String {
        self.version.clone()
    }
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
    let store = workspace.storage_set()?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(workspace.get_root());
    let (restored, relative) = restore_into(&layout, &store, &index, &root, cwd, path, version)?;
    remember(&layout, &root, &relative)?;

    Ok(restored)
}

/// Puts the content of `version` at `path` in a Vault.
///
/// `path` is relative to `cwd` or absolute, and must be inside the Vault and name an entry.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Vault or names no entry,
/// [`WriteError::Precondition`] when the version does not read, [`WriteError::Index`] when the index
/// cannot be read, and [`WriteError::Storage`] when the content cannot be extracted.
pub fn restore_vault(
    vault: &Vault,
    cwd: &Path,
    path: &str,
    version: &str,
) -> Result<Restored, WriteError> {
    let layout = vault.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = vault.storage_set()?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(vault.get_root());
    let (restored, _) = restore_into(&layout, &store, &index, &root, cwd, path, version)?;

    Ok(restored)
}

/// Puts a version's content at a path, answering what the Layout now names and where it is.
fn restore_into(
    layout: &Layout,
    store: &StorageSet,
    index: &VCSIndex,
    root: &Path,
    cwd: &Path,
    path: &str,
    version: &str,
) -> Result<(Restored, String), WriteError> {
    let resolved = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = normalize(&resolved);
    let relative = relative_to(root, &resolved).ok_or_else(|| WriteError::InvalidPath {
        path: path.to_owned(),
        reason: "the path is outside the scope".to_owned(),
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
    runtime.block_on(extract_file(store, &content, &resolved))?;
    set_version(layout, id, *target.digest())?;

    Ok((
        Restored {
            id,
            version: target,
        },
        relative,
    ))
}

/// Puts a version's content back at a path under a Workspace.
///
/// # FFI
///
/// A `RolaWriteRestore *` the caller releases with `free_rola_write_restore`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`restore`] names.
#[lazyffi(export = rola_write_checkout_restore)]
pub fn restore_at(directory: &Path, path: &str, version: &str) -> Result<WriteRestore, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let restored = restore(&workspace, directory, path, version)?;

    Ok(WriteRestore::of(restored.id, &restored.version))
}

/// Puts a version's content back at a path under a Vault.
///
/// # FFI
///
/// A `RolaWriteRestore *` the caller releases with `free_rola_write_restore`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoVault`] when no Vault holds `directory`, and the failures
/// [`restore_vault`] names.
#[lazyffi(export = rola_write_checkout_restore_vault)]
pub fn restore_vault_at(
    directory: &Path,
    path: &str,
    version: &str,
) -> Result<WriteRestore, WriteError> {
    let vault = Vault::locate(directory).ok_or(WriteError::NoVault)?;
    let restored = restore_vault(&vault, directory, path, version)?;

    Ok(WriteRestore::of(restored.id, &restored.version))
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

    use super::{
        Checkout, WriteRestore, forget_at, remember, remember_at, restore, restore_at,
        restore_vault_at,
    };
    use crate::write::WriteError;

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

    #[test]
    fn a_restore_answer_names_the_entry_and_the_version() {
        let id = uuid::Uuid::from_u128(7);
        let version = rorolala_storage::Key::new([9; 32]);
        let answer = WriteRestore::of(id, &version);

        assert_eq!(answer.id(), id.to_string());
        assert_eq!(answer.version(), version.to_string());
    }

    #[test]
    fn remembering_and_forgetting_change_only_what_the_readings_hold() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-checkout-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let clean = Checkout::open(&layout, &dir);
        clean.finish().unwrap();

        let mut checkout = Checkout::open(&layout, &dir);
        checkout.remember("a.txt").unwrap();
        checkout.remember("missing.txt").unwrap();
        checkout.finish().unwrap();

        let mut written = Checkout::open(&layout, &dir);
        written.forget("a.txt").unwrap();
        written.finish().unwrap();

        let mut refused = Checkout::open(&layout, &dir);
        assert!(matches!(
            refused.remember(""),
            Err(WriteError::InvalidPath { .. })
        ));
        assert!(matches!(
            refused.forget(""),
            Err(WriteError::InvalidPath { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_workspace_exports_remember_and_forget_a_path() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-checkout-exports-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        assert_eq!(remember_at(&dir, "a.txt").unwrap(), "");
        assert_eq!(forget_at(&dir, "a.txt").unwrap(), "");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_restore_of_a_path_the_layout_does_not_name_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-checkout-restore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let version = rorolala_storage::Key::new([1; 32]).to_string();

        assert!(matches!(
            restore(&workspace, &dir, "missing.psd", &version),
            Err(WriteError::InvalidPath { .. })
        ));
        assert!(matches!(
            restore_at(&dir, "missing.psd", &version),
            Err(WriteError::InvalidPath { .. })
        ));

        let vault_root = dir.join("vault");
        rorolala_vault::Vault::create(&vault_root).unwrap();
        assert!(restore_vault_at(&vault_root, "missing.psd", &version).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}

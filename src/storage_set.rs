//! Finding the stores a directory works on.
//!
//! A run that is in a store works on that store, since it is where the caller deliberately stands.
//! A run that is not may be inside a Vault or a Workspace, and each of those names the stores it
//! binds; the deeper one wins, because it is the one the caller is in, and a Vault and a Workspace
//! at the same root are one place, which is read as a Vault the way the store search reads it.
//!
//! What this answers is the order a run reaches for its stores in. It does not read a
//! configuration for itself: that is the Vault's or the Workspace's, and a configuration that is
//! there and does not read is their failure rather than an empty answer here.

use std::path::Path;

use rorolala_storage::{RorolalaStorage, StorageConfigError, StorageSet};
use rorolala_utils_configure::Configure as _;
use rorolala_utils_constants::STORAGE_CONFIG_PATH;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;

use crate::write::WriteError;

/// The stores `directory` works on, when it works on any.
///
/// A store at or above `directory` is the whole set. Otherwise the deeper of the Vault and the
/// Workspace holding `directory` answers with the stores it binds, and a Vault wins a tie.
///
/// # Errors
///
/// Returns [`StorageConfigError`] when the Vault or the Workspace is found but its configuration
/// does not read. It is not answered as "no store", which would let a run write where the
/// configuration never said.
pub fn for_directory(directory: &Path) -> Result<Option<StorageSet>, StorageConfigError> {
    if let Some(store) = RorolalaStorage::locate(directory) {
        return Ok(Some(StorageSet::sole(store)));
    }

    let vault = Vault::locate(directory);
    let workspace = Workspace::locate(directory);

    match (vault, workspace) {
        (Some(vault), Some(workspace)) => {
            let vault_root = vault.get_root();
            let workspace_root = workspace.get_root();

            if depth(vault_root) >= depth(workspace_root) {
                Ok(Some(vault.storage_set()?))
            } else {
                Ok(Some(workspace.storage_set()?))
            }
        }
        (Some(vault), None) => Ok(Some(vault.storage_set()?)),
        (None, Some(workspace)) => Ok(Some(workspace.storage_set()?)),
        (None, None) => Ok(None),
    }
}

/// How many components a root has, which is how deep in the tree it sits.
fn depth(root: &Path) -> usize {
    root.components().count()
}

/// What a binding wrote, as the C ABI answers it.
#[lazyffi(export = RolaWriteStorageBind)]
#[derive(Debug, Clone, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is a separate fact about the binding that a caller reads on its own"
)]
pub struct WriteStorageBind {
    /// Whether a Workspace was bound.
    bound_workspace: bool,
    /// Whether a Vault was bound.
    bound_vault: bool,
    /// Whether the store recorded the binding back.
    reverse_recorded: bool,
    /// Whether the store can only be read.
    read_only: bool,
    /// What a caller should be told, one line each.
    warnings: Vec<String>,
}

#[lazyffi(export = rola_)]
impl WriteStorageBind {
    /// Whether a Workspace was bound.
    #[must_use]
    pub const fn bound_workspace(&self) -> bool {
        self.bound_workspace
    }

    /// Whether a Vault was bound.
    #[must_use]
    pub const fn bound_vault(&self) -> bool {
        self.bound_vault
    }

    /// Whether the store recorded the binding back.
    #[must_use]
    pub const fn reverse_recorded(&self) -> bool {
        self.reverse_recorded
    }

    /// Whether the store can only be read.
    #[must_use]
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// How many lines a caller should be told.
    #[must_use]
    pub const fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// One line a caller should be told, or an empty string past the end.
    #[must_use]
    pub fn warning_at(&self, index: usize) -> String {
        self.warnings.get(index).cloned().unwrap_or_default()
    }
}

/// Makes the directory at `root` a store.
///
/// # FFI
///
/// Nothing on success, or a `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::Precondition`] when the directory is a store already, a Workspace or a
/// Vault root, or not a directory, and [`WriteError::Storage`] when the store cannot be made.
#[lazyffi(export = rola_storage_create)]
pub fn create_store_at(root: &Path) -> Result<(), WriteError> {
    let root = normalize(root);

    if root.join(STORAGE_CONFIG_PATH).is_file() {
        return Err(WriteError::Precondition {
            what: root.display().to_string(),
            reason: "the directory is a store already".to_owned(),
        });
    }

    if root.join(".rola").is_dir() || root.join("vault.toml").is_file() {
        return Err(WriteError::Precondition {
            what: root.display().to_string(),
            reason: "the directory is a Workspace or a Vault".to_owned(),
        });
    }

    if root.exists() && !root.is_dir() {
        return Err(WriteError::InvalidPath {
            path: root.display().to_string(),
            reason: "it is not a directory".to_owned(),
        });
    }

    RorolalaStorage::try_create(&root)
        .map(|_| ())
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })
}

/// Binds a store to the Workspace rooted at `workspace_root`.
///
/// # FFI
///
/// A `RolaWriteStorageBind *` the caller releases with `free_rola_write_storage_bind`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds the root,
/// [`WriteError::Precondition`] when the path is not a store or is inside the Workspace's own data
/// directory, and [`WriteError::Storage`] when a configuration cannot be read or written.
#[lazyffi(export = rola_workspace_storage_bind)]
pub fn workspace_bind_storage(
    workspace_root: &Path,
    store_root: &Path,
) -> Result<WriteStorageBind, WriteError> {
    let workspace = Workspace::locate(workspace_root).ok_or(WriteError::NoWorkspace)?;
    let root = normalized_store(store_root)?;
    let own = normalize(&workspace.get_root().join(".rola"));

    if root.starts_with(&own) && root != normalize(&own.join("storage")) {
        return Err(WriteError::Precondition {
            what: root.display().to_string(),
            reason: "it is inside the Workspace's own data directory".to_owned(),
        });
    }

    let mut config = read_workspace_config(&workspace)?;
    let already = workspace
        .storage_set()
        .is_ok_and(|set| set.bindings().iter().any(|binding| binding.root() == root));

    let store = RorolalaStorage::at(&root).ok_or_else(|| not_a_store(&root))?;
    let (reverse, read_only, warnings) = record(&store, workspace.get_root(), true)?;

    if !already {
        config
            .default_config_mut()
            .add_storage(root.to_string_lossy().into_owned());
        config
            .write_to(&workspace.config_path())
            .map_err(|error| config_error(&error))?;
    }

    Ok(WriteStorageBind {
        bound_workspace: true,
        bound_vault: false,
        reverse_recorded: reverse,
        read_only,
        warnings,
    })
}

/// Binds a store to the Vault rooted at `vault_root`.
///
/// # FFI
///
/// The same `RolaWriteStorageBind *` as [`workspace_bind_storage`], with `_bound_vault` set
/// instead.
///
/// # Errors
///
/// Returns [`WriteError::NoVault`] when no Vault holds the root, and the same failures as
/// [`workspace_bind_storage`] otherwise.
#[lazyffi(export = rola_vault_storage_bind)]
pub fn vault_bind_storage(
    vault_root: &Path,
    store_root: &Path,
) -> Result<WriteStorageBind, WriteError> {
    let vault = Vault::locate(vault_root).ok_or(WriteError::NoVault)?;
    let root = normalized_store(store_root)?;
    let own = normalize(vault.get_root());

    if root.starts_with(&own) && root != normalize(&own.join("storage")) {
        return Err(WriteError::Precondition {
            what: root.display().to_string(),
            reason: "it is inside the Vault's root".to_owned(),
        });
    }

    let mut config = rorolala_vault::Config::read_from(&vault.config_path())
        .map_err(|error| config_error(&error))?;
    let already = vault
        .storage_set()
        .is_ok_and(|set| set.bindings().iter().any(|binding| binding.root() == root));

    let store = RorolalaStorage::at(&root).ok_or_else(|| not_a_store(&root))?;
    let (reverse, read_only, warnings) = record(&store, vault.get_root(), false)?;

    if !already {
        config
            .storage_config_mut()
            .add_storage(root.to_string_lossy().into_owned());
        config
            .write_to(&vault.config_path())
            .map_err(|error| config_error(&error))?;
    }

    Ok(WriteStorageBind {
        bound_workspace: false,
        bound_vault: true,
        reverse_recorded: reverse,
        read_only,
        warnings,
    })
}

/// The places the store rooted at `store_root` records as writing to it.
///
/// # FFI
///
/// A `RolaStorageConnected *` the caller releases with `free_rola_storage_connected`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::Precondition`] when the directory is not a store, and
/// [`WriteError::Storage`] when its configuration cannot be read.
#[lazyffi(export = rola_storage_connected)]
pub fn connected(store_root: &Path) -> Result<StorageConnected, WriteError> {
    let root = normalized_store(store_root)?;
    let store = RorolalaStorage::at(&root).ok_or_else(|| not_a_store(&root))?;
    let connected = store.connected().map_err(|error| WriteError::Storage {
        reason: error.to_string(),
    })?;

    Ok(StorageConnected {
        workspaces: connected.workspaces,
        vaults: connected.vaults,
    })
}

/// The places a store records as writing to it.
#[lazyffi(export = RolaStorageConnected)]
#[derive(Debug, Clone, Default)]
pub struct StorageConnected {
    /// The Workspaces that write to it, as the configuration writes them.
    workspaces: Vec<String>,
    /// The Vaults that write to it, as the configuration writes them.
    vaults: Vec<String>,
}

#[lazyffi(export = rola_)]
impl StorageConnected {
    /// How many Workspaces write to the store.
    #[must_use]
    pub const fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// The Workspace at `index`, or an empty string past the end.
    #[must_use]
    pub fn workspace_at(&self, index: usize) -> String {
        self.workspaces.get(index).cloned().unwrap_or_default()
    }

    /// How many Vaults write to the store.
    #[must_use]
    pub const fn vault_count(&self) -> usize {
        self.vaults.len()
    }

    /// The Vault at `index`, or an empty string past the end.
    #[must_use]
    pub fn vault_at(&self, index: usize) -> String {
        self.vaults.get(index).cloned().unwrap_or_default()
    }
}

/// `path` resolved and checked to be a store, without opening it.
fn normalized_store(path: &Path) -> Result<std::path::PathBuf, WriteError> {
    let root = normalize(path);

    if !root.is_dir() {
        return Err(WriteError::InvalidPath {
            path: root.display().to_string(),
            reason: "it is not a directory".to_owned(),
        });
    }

    Ok(root)
}

/// The failure a path that is not a store is refused with.
fn not_a_store(root: &Path) -> WriteError {
    WriteError::Precondition {
        what: root.display().to_string(),
        reason: "it carries no `rolast.toml`".to_owned(),
    }
}

/// A configuration failure as a write failure.
fn config_error(error: &rorolala_utils_configure::Error) -> WriteError {
    WriteError::Storage {
        reason: error.to_string(),
    }
}

/// The Workspace's configuration, read.
fn read_workspace_config(workspace: &Workspace) -> Result<rorolala_workspace::Config, WriteError> {
    rorolala_workspace::Config::read_from(&workspace.config_path())
        .map_err(|error| config_error(&error))
}

/// Records the binding in the store, answering whether it was recorded and what to warn about.
fn record(
    store: &RorolalaStorage,
    root: &Path,
    workspace_side: bool,
) -> Result<(bool, bool, Vec<String>), WriteError> {
    if !store.is_writable() {
        return Ok((
            false,
            true,
            vec!["the store is read-only, so it is used for reading only".to_owned()],
        ));
    }

    let recorded = if workspace_side {
        store.connect_workspace(root)
    } else {
        store.connect_vault(root)
    };

    recorded
        .map(|_| (true, false, Vec::new()))
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::RorolalaStorage;
    use rorolala_utils_configure::Configure as _;
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::{Config as VaultConfig, Vault};
    use rorolala_workspace::{Config, Workspace};

    use super::{create_store_at, for_directory, workspace_bind_storage};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-storage-set-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_dir_that_is_in_a_store_works_on_that_store_alone() {
        let dir = scratch("store");
        let root = dir.join("store");
        let _ = RorolalaStorage::create(&root);

        let set = for_directory(&root.join("obj")).unwrap().unwrap();

        assert_eq!(set.stores().count(), 1);
        assert_eq!(set.write_target().get_root(), root.as_path());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_answers_with_the_stores_it_binds() {
        let dir = scratch("workspace");
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let shared = dir.join("shared");
        let _ = RorolalaStorage::create(&shared);

        let mut config = Config::read_from(&workspace.config_path()).unwrap();
        config
            .default_config_mut()
            .add_storage(shared.to_string_lossy());
        config.write_to(&workspace.config_path()).unwrap();

        let set = for_directory(&dir).unwrap().unwrap();

        assert_eq!(set.write_target().get_root(), shared.as_path());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_deeper_of_a_vault_and_a_workspace_answers() {
        let parent = scratch("deeper");
        let workspace_root = parent.join("workspace");
        Workspace::create(&workspace_root).unwrap();
        let vault_root = workspace_root.join("vault");
        Vault::create(&vault_root).unwrap();
        let vault = Vault::locate(&vault_root).unwrap();
        let shared = parent.join("shared");
        let _ = RorolalaStorage::create(&shared);

        let mut config = VaultConfig::read_from(&vault.config_path()).unwrap();
        config
            .storage_config_mut()
            .add_storage(shared.to_string_lossy());
        config.write_to(&vault.config_path()).unwrap();

        let set = for_directory(&vault_root).unwrap().unwrap();

        // The Vault is the deeper root, so its stores are the ones answered.
        assert_eq!(set.write_target().get_root(), shared.as_path());

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn a_dir_that_is_in_nothing_works_on_nothing() {
        let dir = scratch("nowhere");

        assert!(for_directory(&dir).unwrap().is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_configuration_that_does_not_read_is_a_failure_rather_than_no_store() {
        let dir = scratch("broken");
        Workspace::create(&dir).unwrap();
        fs::write(dir.join(".rola").join("workspace.toml"), "not toml").unwrap();

        assert!(for_directory(&dir).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_store_is_made_and_bound_to_the_workspace() {
        let dir = scratch("bind");
        Workspace::create(&dir).unwrap();
        let store = dir.join("shared");

        create_store_at(&store).unwrap();
        assert!(store.join("rolast.toml").is_file());
        assert!(create_store_at(&store).is_err());

        let answer = workspace_bind_storage(&dir, &store).unwrap();
        assert!(answer.bound_workspace());
        assert!(answer.reverse_recorded());

        let workspace = Workspace::locate(&dir).unwrap();
        let set = workspace.storage_set().unwrap();
        assert!(
            set.bindings()
                .iter()
                .any(|binding| binding.root() == store.as_path())
        );

        let record = RorolalaStorage::at(&store).unwrap().connected().unwrap();
        assert!(
            record
                .workspaces
                .iter()
                .any(|path| Path::new(path) == dir.as_path())
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

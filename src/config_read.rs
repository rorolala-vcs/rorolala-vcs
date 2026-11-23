//! The configuration a place holds, as the C ABI reads it.
//!
//! A Workspace's configuration is where the Vaults it knows are bound, which name it reaches for,
//! and what a collecting run is told; a Vault's is what it says about itself, who may administer it,
//! and what its daemon may serve. Both are written as files by [`rorolala_utils_configure`], and the
//! reading here is the same one the commands make before they act.
//!
//! What crosses is a handle per configuration: the file is read once, and each fact is read from the
//! handle by its own accessor. A list nested in a configuration is a list handle of its own, so the
//! ABI never carries a document a caller would have to parse.

use std::path::Path;

use rorolala_daemon::{Namespaces, registry_path};
use rorolala_storage::{BoundStorage, StorageSet};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;

/// The stores a Workspace works on, read from the Workspace holding `directory`.
///
/// A Workspace whose configuration cannot be read still answers with the list, empty, since the
/// caller can make nothing of a store it could not read either way. A directory no Workspace holds
/// answers with null.
///
/// # FFI
///
/// A `RolaStorageBindings *` the caller releases with `free_rola_storage_bindings`, or null when a
/// directory no Workspace holds.
#[must_use]
#[lazyffi(export = rola_workspace_storages_new)]
pub fn workspace_storages_new(directory: &Path) -> Option<StorageBindings> {
    let workspace = Workspace::locate(directory)?;
    let set = workspace.storage_set().ok();

    Some(storage_bindings(set.as_ref()))
}

/// The stores a Vault works on, read from the Vault holding `directory`.
///
/// # FFI
///
/// A `RolaStorageBindings *` the caller releases with `free_rola_storage_bindings`, or null when a
/// directory no Vault holds.
#[must_use]
#[lazyffi(export = rola_vault_storages_new)]
pub fn vault_storages_new(directory: &Path) -> Option<StorageBindings> {
    let vault = Vault::locate(directory)?;
    let set = vault.storage_set().ok();

    Some(storage_bindings(set.as_ref()))
}

/// The stores a place works on.
#[lazyffi(export = RolaStorageBindings)]
#[derive(Debug, Clone, Default)]
pub struct StorageBindings {
    /// The stores, in the order the place reaches for them.
    bindings: Vec<StorageBinding>,
}

#[lazyffi(export = rola_)]
impl StorageBindings {
    /// How many stores the place works on.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.bindings.len()
    }

    /// The store at `index`, or null past the end.
    #[must_use]
    pub fn at(&self, index: usize) -> Option<StorageBinding> {
        self.bindings.get(index).cloned()
    }
}

/// One store a place works on.
#[lazyffi(export = RolaStorageBinding)]
#[derive(Debug, Clone)]
pub struct StorageBinding {
    /// The path as the configuration wrote it.
    path: String,
    /// The resolved, absolute path.
    root: String,
    /// Whether this is the place's own store.
    own: bool,
    /// Whether the directory is a store this build can open.
    present: bool,
    /// Whether the store would accept a write.
    writable: bool,
    /// Why it cannot be used, when it cannot.
    reason: String,
}

#[lazyffi(export = rola_)]
impl StorageBinding {
    /// The path as the configuration wrote it.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The resolved, absolute path.
    #[must_use]
    pub fn root(&self) -> String {
        self.root.clone()
    }

    /// Whether this is the place's own store.
    #[must_use]
    pub const fn own(&self) -> bool {
        self.own
    }

    /// Whether the directory is a store this build can open.
    #[must_use]
    pub const fn present(&self) -> bool {
        self.present
    }

    /// Whether the store would accept a write.
    #[must_use]
    pub const fn writable(&self) -> bool {
        self.writable
    }

    /// Why it cannot be used, or an empty string when it can.
    #[must_use]
    pub fn reason(&self) -> String {
        self.reason.clone()
    }
}

/// The stores a place works on, as the list hands them out.
fn storage_bindings(set: Option<&StorageSet>) -> StorageBindings {
    let Some(set) = set else {
        return StorageBindings::default();
    };

    let mut bindings: Vec<StorageBinding> = set
        .bindings()
        .iter()
        .map(|binding| storage_binding(set, binding))
        .collect();

    // A place that does not write `.` among its bindings still reads its own store last.
    if !set.own_bound() {
        bindings.push(StorageBinding {
            path: ".".to_owned(),
            root: set.own_root().display().to_string(),
            own: true,
            present: set.own_present(),
            writable: set.own_writable(),
            reason: String::new(),
        });
    }

    StorageBindings { bindings }
}

/// One store, as the list hands it out.
fn storage_binding(set: &StorageSet, binding: &BoundStorage) -> StorageBinding {
    let own = binding.is_own();

    StorageBinding {
        path: binding.written().to_owned(),
        root: binding.root().display().to_string(),
        own,
        present: if own {
            set.own_present()
        } else {
            binding.is_present()
        },
        writable: if own {
            set.own_writable()
        } else {
            binding.is_writable()
        },
        reason: binding.reason().map_or_else(String::new, str::to_owned),
    }
}

/// The Vaults the machine's daemon serves, by namespace.
///
/// The table is the daemon's own registry, under the user's data home. A machine that has
/// registered nothing has an empty table, which is an empty list rather than a failure; a table
/// that is there and cannot be read is `null`, since the reader cannot tell what is served.
///
/// # FFI
///
/// A `RolaDaemonVaults *` the caller releases with `free_rola_daemon_vaults`, or null when the
/// table cannot be read. Its Vaults are read by index, each an owned `RolaDaemonVault`.
#[must_use]
#[lazyffi(export = rola_daemon_vaults_new)]
pub fn daemon_vaults_new() -> Option<DaemonVaults> {
    let path = registry_path()?;
    let namespaces = Namespaces::read_from(&path).ok()?;
    let vaults = namespaces
        .iter()
        .map(|(name, path)| DaemonVault {
            name: name.to_owned(),
            path: path.display().to_string(),
        })
        .collect();

    Some(DaemonVaults { vaults })
}

/// The Vaults the daemon serves.
#[lazyffi(export = RolaDaemonVaults)]
#[derive(Debug, Clone)]
pub struct DaemonVaults {
    /// The Vaults, in namespace order.
    vaults: Vec<DaemonVault>,
}

#[lazyffi(export = rola_)]
impl DaemonVaults {
    /// How many Vaults the daemon serves.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.vaults.len()
    }

    /// The Vault at `index`, or null past the end.
    #[must_use]
    pub fn at(&self, index: usize) -> Option<DaemonVault> {
        self.vaults.get(index).cloned()
    }
}

/// One Vault the daemon serves.
#[lazyffi(export = RolaDaemonVault)]
#[derive(Debug, Clone)]
pub struct DaemonVault {
    /// The namespace it answers under.
    name: String,
    /// The directory the Vault sits at.
    path: String,
}

#[lazyffi(export = rola_)]
impl DaemonVault {
    /// The namespace the Vault answers under.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// The directory the Vault sits at.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_configure::Configure as _;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::{Config as WorkspaceConfig, Workspace};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let name = format!(
            "rorolala-config-read-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );

        for base in [Some(std::env::temp_dir()), program_dir()] {
            let Some(base) = base else {
                continue;
            };
            let dir = base.join(&name);
            let _ = fs::remove_dir_all(&dir);

            if fs::create_dir_all(&dir).is_ok() {
                return dir;
            }
        }

        panic!("no writable directory to make {name} in");
    }

    /// The directory this test's program sits in, or nothing when it cannot be found.
    fn program_dir() -> Option<PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
    }

    #[test]
    fn no_workspace_answers_with_nothing() {
        let dir = scratch("absent");

        assert!(rorolala_workspace::read(&dir).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_vaults_a_workspace_knows_are_listed_by_name() {
        let dir = scratch("vaults");
        Workspace::create(&dir).unwrap();

        let workspace = Workspace::locate(&dir).unwrap();
        let path = workspace.config_path();
        let mut config = WorkspaceConfig::read_from(&path).unwrap();
        config.vaults_mut().bind("studio", "tcp://127.0.0.1:9270");
        config
            .vaults_mut()
            .bind("alpha", "ssh://alice@example.test/vault");
        config.default_config_mut().set_vault("studio");
        config.write_to(&path).unwrap();

        let config = rorolala_workspace::read(&dir).unwrap();

        assert_eq!(config.vault_count(), 2);
        assert_eq!(config.vault_at(0).unwrap().name(), "alpha");
        assert_eq!(config.vault_at(1).unwrap().name(), "studio");
        assert!(config.vault_at(1).unwrap().is_default());
        assert_eq!(
            config.vault_at(1).unwrap().address(),
            "tcp://127.0.0.1:9270"
        );
        assert_eq!(config.default_vault(), "studio");
        assert!(!config.gc_present());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_that_is_no_vault_answers_with_nothing() {
        let dir = scratch("no-vault");

        assert!(rorolala_vault::read(&dir).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_says_what_it_is_and_who_administers_it() {
        let dir = scratch("vault");
        rorolala_vault::Vault::create(&dir).unwrap();

        let config = rorolala_vault::read(&dir).unwrap();

        assert_ne!(config.name(), "");
        assert_ne!(config.description(), "");
        assert_eq!(config.admin_count(), 0);
        assert_eq!(config.admin_at(0), "");
        assert!(config.max_concurrency() > 0);

        let _ = fs::remove_dir_all(&dir);
    }
}

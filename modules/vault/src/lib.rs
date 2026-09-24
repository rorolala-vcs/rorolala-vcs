#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};

use rorolala_storage::{LockError, Lockable, LockingGuard, RorolalaStorage};
use rorolala_utils_constants::LOCK_FILE;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate, LocateHelper};

mod config;
mod error;
mod ffi;
mod init;
mod keys;
mod root_vault;

pub use config::*;
pub use error::*;
pub use ffi::*;
pub use keys::*;
pub use root_vault::*;

/// Path to the Vault configuration file, and the directories the Vault keeps under it
///
/// All three are the layout [`rorolala_utils_constants`] states, re-exported so the Vault's
/// own spelling of where it keeps things is still one name.
pub use rorolala_utils_constants::{
    VAULT_CONFIG_PATH as CONFIG_PATH, VAULT_KEYS_DIR as KEYS_DIR, VAULT_STORAGE_DIR as STORAGE_DIR,
    VAULT_VAULTS_DIR as VAULTS_DIR,
};

/// Rorolala remote resource vault
///
/// It can only be loaded and operated on the machine where the Vault resides,
/// and cannot be operated directly by the Workspace.
#[lazyffi(export = RolaVault)]
#[derive(Default, Clone)]
pub struct Vault {
    /// The directory where the current Vault is located
    current_dir: PathBuf,
}

/// Locates a [`Vault`] by searching upwards from the given directory
///
/// Starting at `vault_dir`, this walks up the directory tree looking for
/// the Vault configuration file ([`CONFIG_PATH`]). Returns `Some(Vault)`
/// pointing at the directory that contains the configuration file, or
/// `None` if no Vault could be found.
#[must_use]
#[lazyffi(export = locate_rola_vault)]
pub fn locate_vault(vault_dir: &Path) -> Option<Vault> {
    Vault::locate(vault_dir)
}

impl Locate for Vault {
    fn locate(cwd: &Path) -> Option<Self> {
        let path = cwd.locate(|cwd| cwd.join(CONFIG_PATH).exists())?;
        Some(Self { current_dir: path })
    }

    fn get_root(&self) -> &Path {
        self.current_dir.as_path()
    }
}

impl Lockable for Vault {
    /// The lock sits at the Vault's root, beside the configuration that makes the directory one.
    ///
    /// A Vault is found by sniffing upwards to the nearest configuration, so a Vault inside another
    /// locks its own root and not the one holding it: the lock belongs to the Vault the run is in,
    /// and is not passed down from above.
    fn lock_path(&self) -> PathBuf {
        self.current_dir.join(LOCK_FILE)
    }

    async fn lock(&self) -> Result<LockingGuard<Self>, LockError> {
        LockingGuard::acquire(self.clone(), self.lock_path()).await
    }
}

impl Vault {
    /// The Vault rooted at `current_dir`, which is taken to hold a configuration.
    ///
    /// This builds a Vault out of a directory that is already known to be one — where a
    /// search has just found its configuration, say — rather than looking for one, so it
    /// says nothing about whether `current_dir` is a Vault and takes the caller's word for it.
    pub(crate) const fn at(current_dir: PathBuf) -> Self {
        Self { current_dir }
    }

    /// The file this Vault keeps its configuration in.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.current_dir.join(CONFIG_PATH)
    }

    /// The store this Vault keeps, if it has one.
    ///
    /// A Vault keeps its store under [`STORAGE_DIR`], and having one is what a directory
    /// carrying [`STORAGE_CONFIG_PATH`](rorolala_utils_constants::STORAGE_CONFIG_PATH) there
    /// says. Nothing is made here: a Vault without one is a Vault without one.
    #[must_use]
    pub fn get_current_rola_storage(&self) -> Option<RorolalaStorage> {
        RorolalaStorage::at(self.storage_root())
    }

    /// The store this Vault keeps, made where it is not there yet.
    ///
    /// Making a store is best effort, so this always answers: a store that could not be made
    /// is still handed back, and what failed shows up on the first read or write that needs it.
    #[must_use]
    pub fn get_or_create_rola_storage(&self) -> RorolalaStorage {
        self.get_current_rola_storage()
            .unwrap_or_else(|| RorolalaStorage::create(self.storage_root()))
    }

    /// Where the Vault's store is rooted, with the layout's spelling walked back out of it.
    ///
    /// The directory is written `./storage/`, and joining that onto the Vault's root leaves
    /// the `./` in the middle of the path a reader is shown, so it is a path of its own here.
    fn storage_root(&self) -> PathBuf {
        self.current_dir.join(STORAGE_DIR).components().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate;

    use crate::{CONFIG_PATH, Vault};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-vault-locate-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_vault_keeps_its_configuration_in_the_directory_it_was_located_at() {
        let dir = scratch("config-path");
        Vault::create(&dir).unwrap();

        let vault = Vault::locate(&dir).unwrap();

        assert_eq!(vault.config_path(), dir.join(CONFIG_PATH));
        assert!(vault.config_path().is_file(), "{:?}", vault.config_path());

        let _ = fs::remove_dir_all(&dir);
    }
}

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate, LocateHelper};

mod config;
mod error;
mod init;
mod keys;
mod root_vault;

pub use config::*;
pub use error::*;
pub use keys::*;
pub use root_vault::*;

/// Path to the Vault configuration file, and the directories the Vault keeps under it
///
/// All three are the layout [`rorolala_utils_constants`] states, re-exported so the Vault's
/// own spelling of where it keeps things is still one name.
pub use rorolala_utils_constants::{
    VAULT_CONFIG_PATH as CONFIG_PATH, VAULT_KEYS_DIR as KEYS_DIR, VAULT_VAULTS_DIR as VAULTS_DIR,
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

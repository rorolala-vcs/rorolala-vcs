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

pub use config::*;
pub use error::*;

/// Path to the Vault configuration file, and the directory its keys are kept in
///
/// Both are the layout [`rorolala_utils_constants`] states, re-exported so the Vault's own
/// spelling of where it keeps things is still one name.
pub use rorolala_utils_constants::{VAULT_CONFIG_PATH as CONFIG_PATH, VAULT_KEYS_DIR as KEYS_DIR};

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

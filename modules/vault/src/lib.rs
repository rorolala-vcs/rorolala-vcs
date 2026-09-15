#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate, LocateHelper};

/// Rorolala remote resource vault
///
/// It can only be loaded and operated on the machine where the Vault resides,
/// and cannot be operated directly by the Workspace.
#[lazyffi(export = RolaVault)]
pub struct Vault {
    /// The directory where the current Vault is located
    current_dir: PathBuf,
}

impl Locate for Vault {
    fn locate(cwd: &std::path::Path) -> Option<Self> {
        let path = cwd.locate(|cwd| cwd.join("vault.toml").exists())?;
        Some(Self { current_dir: path })
    }

    fn get_root(&self) -> &Path {
        self.current_dir.as_path()
    }
}

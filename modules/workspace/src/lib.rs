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

pub use error::*;

/// Workspace data directory
#[lazyffi(export = WORKSPACE_DATA_DIR)]
pub const DATA_DIR: &str = "./.rola/";

/// Path to the workspace configuration file
#[lazyffi(export = WORKSPACE_CONFIG_PATH)]
pub const CONFIG_PATH: &str = "./.rola/workspace.toml";

/// Rorolala local workspace
///
/// The local workspace is used to edit, organize, and advance the progress
/// and versions of local projects. You can share progress by setting up a
/// remote Vault.
#[lazyffi(export = RolaWorkspace)]
#[derive(Default, Clone)]
pub struct Workspace {
    /// The directory where the current Workspace is located
    current_dir: PathBuf,
}

impl Locate for Workspace {
    fn locate(cwd: &std::path::Path) -> Option<Self> {
        let path = cwd.locate(|cwd| cwd.join(DATA_DIR).is_dir())?;
        Some(Self { current_dir: path })
    }

    fn get_root(&self) -> &Path {
        self.current_dir.as_path()
    }
}

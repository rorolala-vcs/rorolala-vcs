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

/// Path, inside the Workspace's data directory, where its member keys are kept
///
/// A key kept here belongs to the Workspace it was set up in, so it is one of the two
/// directories the local scope of a member search covers — the other being the Vault's.
#[lazyffi(export = WORKSPACE_KEYS_DIR)]
pub const KEYS_DIR: &str = "./.rola/auth/";

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

/// Locates a [`Workspace`] by searching upwards from the given directory
///
/// Starting at `workspace_dir`, this walks up the directory tree looking for
/// the Workspace configuration file ([`CONFIG_PATH`]). Returns `Some(Workspace)`
/// pointing at the directory that contains the configuration file, or
/// `None` if no Workspace could be found.
#[must_use]
#[lazyffi(export = locate_rola_workspace)]
pub fn locate_workspace(workspace_dir: &Path) -> Option<Workspace> {
    Workspace::locate(workspace_dir)
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

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::{Workspace, free_rola_workspace, locate_rola_workspace};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-workspace-locate-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn locating_nothing_hands_back_null() {
        let dir = scratch("absent");
        let raw = CString::new(dir.to_str().unwrap()).unwrap();

        // SAFETY: `raw` is a NUL-terminated C string that outlives the call, which is
        // all the export reads it for.
        let found = unsafe { locate_rola_workspace(raw.as_ptr().cast_mut()) };

        assert!(found.is_null(), "{dir:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn locating_a_workspace_hands_back_a_pointer_its_owner_releases() {
        let dir = scratch("found");
        Workspace::create(&dir).unwrap();
        let raw = CString::new(dir.to_str().unwrap()).unwrap();

        // SAFETY: `raw` is a NUL-terminated C string that outlives the call.
        let found = unsafe { locate_rola_workspace(raw.as_ptr().cast_mut()) };
        assert!(!found.is_null(), "{dir:?}");

        // SAFETY: `found` came from the export above and has not been released yet.
        unsafe { free_rola_workspace(found) };

        let _ = fs::remove_dir_all(&dir);
    }
}

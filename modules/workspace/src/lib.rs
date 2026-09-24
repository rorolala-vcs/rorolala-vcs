#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};

use rorolala_storage::{LockError, Lockable, LockingGuard, RorolalaStorage};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate, LocateHelper};

mod config;
mod error;
mod ffi;
mod init;

pub use config::*;
pub use error::*;
pub use ffi::*;

/// Where the Workspace keeps its data, its configuration, its keys and its store
///
/// These are the layout [`rorolala_utils_constants`] states, re-exported so the
/// Workspace's own spelling of where it keeps things is still one name.
pub use rorolala_utils_constants::{
    WORKSPACE_CONFIG_PATH as CONFIG_PATH, WORKSPACE_DATA_DIR as DATA_DIR,
    WORKSPACE_KEYS_DIR as KEYS_DIR, WORKSPACE_LOCK_PATH as LOCK_PATH,
    WORKSPACE_STORAGE_DIR as STORAGE_DIR,
};

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

impl Lockable for Workspace {
    /// The lock sits inside the data directory, beside the rest of what the Workspace keeps for
    /// itself rather than at the root the work is in.
    fn lock_path(&self) -> PathBuf {
        self.current_dir.join(LOCK_PATH).components().collect()
    }

    async fn lock(&self) -> Result<LockingGuard<Self>, LockError> {
        LockingGuard::acquire(self.clone(), self.lock_path()).await
    }
}

impl Workspace {
    /// The file this Workspace keeps its configuration in.
    ///
    /// The configuration sits inside the data directory, so a Workspace that has one
    /// always has somewhere to read it from and to write it back to.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.current_dir.join(CONFIG_PATH)
    }

    /// The store this Workspace keeps, if it has one.
    ///
    /// A Workspace keeps its store under [`STORAGE_DIR`] inside its data directory, and having
    /// one is what a configuration
    /// ([`STORAGE_CONFIG_PATH`](rorolala_utils_constants::STORAGE_CONFIG_PATH)) there says.
    /// Nothing is made here: a Workspace without one is a Workspace without one.
    #[must_use]
    pub fn get_current_rola_storage(&self) -> Option<RorolalaStorage> {
        RorolalaStorage::at(self.storage_root())
    }

    /// The store this Workspace keeps, made where it is not there yet.
    ///
    /// Making a store is best effort, so this always answers: a store that could not be made
    /// is still handed back, and what failed shows up on the first read or write that needs it.
    #[must_use]
    pub fn get_or_create_rola_storage(&self) -> RorolalaStorage {
        self.get_current_rola_storage()
            .unwrap_or_else(|| RorolalaStorage::create(self.storage_root()))
    }

    /// Where the Workspace's store is rooted, with the layout's spelling walked back out of it.
    ///
    /// The directory is written `./.rola/storage/`, and joining that onto the Workspace's root
    /// leaves the `./` in the middle of the path a reader is shown, so it is a path of its own
    /// here.
    fn storage_root(&self) -> PathBuf {
        self.current_dir.join(STORAGE_DIR).components().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate;

    use crate::{CONFIG_PATH, Workspace, free_rola_workspace, locate_rola_workspace};

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

    #[test]
    fn a_workspace_keeps_its_configuration_inside_the_directory_it_was_located_at() {
        let dir = scratch("config-path");
        Workspace::create(&dir).unwrap();

        let workspace = Workspace::locate(&dir).unwrap();

        assert_eq!(workspace.config_path(), dir.join(CONFIG_PATH));
        assert!(
            workspace.config_path().is_file(),
            "{:?}",
            workspace.config_path()
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

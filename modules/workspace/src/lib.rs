#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rorolala_layout::{Layout, LayoutError};
use rorolala_storage::{LockError, Lockable, LockingGuard, RorolalaStorage};
use rorolala_utils_constants::{LAYOUT_TRACK_FILE, WORKSPACE_READONLY_LAYOUTS_DIR};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate, LocateHelper};

mod cache;
mod config;
mod error;
mod ffi;
mod init;
mod merging;
mod ownership;

pub use cache::*;
pub use config::*;
pub use error::*;
pub use ffi::*;
pub use merging::*;
pub use ownership::*;

/// Where the Workspace keeps its data, its configuration, its keys, its store and its index
///
/// These are the layout [`rorolala_utils_constants`] states, re-exported so the
/// Workspace's own spelling of where it keeps things is still one name.
pub use rorolala_utils_constants::{
    WORKSPACE_CONFIG_PATH as CONFIG_PATH, WORKSPACE_DATA_DIR as DATA_DIR,
    WORKSPACE_IGNORE_RULES_DIR as IGNORE_RULES_DIR, WORKSPACE_INDEX_DIR as INDEX_DIR,
    WORKSPACE_KEYS_DIR as KEYS_DIR, WORKSPACE_LAYOUT_DIR as LAYOUT_DIR,
    WORKSPACE_LOCK_PATH as LOCK_PATH, WORKSPACE_STORAGE_DIR as STORAGE_DIR,
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

    /// The Layout this Workspace works in, made where it is not there yet.
    ///
    /// A Workspace has one Layout and it is kept under [`LAYOUT_DIR`]. Opening it makes it where it
    /// is not there, which is what a command that changes the work wants: a Workspace that has just
    /// been made already has one, and one made by an older version has the Layout it worked in
    /// adopted here before anything is made.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] when the Layout cannot be made or read, or the one an older Workspace
    /// worked in cannot be taken over.
    pub fn layout(&self) -> Result<Layout, LayoutError> {
        self.adopt_legacy_layout()?;

        Layout::open(self.layout_dir())
    }

    /// The Layout this Workspace works in, when there is one to read.
    ///
    /// It makes nothing: a Workspace that holds no Layout is a Workspace with no work recorded, and
    /// a reader is told that rather than handed one that was made on the way. A Workspace made by a
    /// version that kept several Layouts has the one it worked in adopted here, once.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] when a Layout that is there cannot be read, or the one an older
    /// Workspace worked in cannot be taken over.
    pub fn layout_if_there(&self) -> Result<Option<Layout>, LayoutError> {
        match Layout::open_existing(self.layout_dir()) {
            Ok(layout) => Ok(Some(layout)),
            Err(error) if error.is_not_found() => {
                self.adopt_legacy_layout()?;

                match Layout::open_existing(self.layout_dir()) {
                    Ok(layout) => Ok(Some(layout)),
                    Err(error) if error.is_not_found() => Ok(None),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// The Vault the Layout being worked in tracks, when it tracks one.
    ///
    /// A Layout tracks nothing when the file naming its Vault is not there, and a Workspace with no
    /// Layout tracks nothing either: a Layout is a place to work, and the Vault it tracks is where
    /// the work goes.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] when the file naming the Vault cannot be read.
    pub fn layout_track(&self) -> Result<Option<String>, LayoutError> {
        let dir = self.layout_dir();
        if !dir.is_dir() {
            return Ok(None);
        }

        match fs::read_to_string(dir.join(LAYOUT_TRACK_FILE)) {
            Ok(text) => {
                let track = text.trim();

                Ok((!track.is_empty()).then(|| track.to_owned()))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Makes the Layout being worked in track the Vault `track`, or track none when it is empty.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] when the file naming the Vault cannot be written, or the Layout an
    /// older Workspace worked in cannot be taken over.
    pub fn set_layout_track(&self, track: &str) -> Result<(), LayoutError> {
        let track = track.trim();
        if track.is_empty() {
            return self.un_track_layout();
        }

        self.adopt_legacy_layout()?;

        let dir = self.layout_dir();
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(LAYOUT_TRACK_FILE), format!("{track}\n"))?;

        Ok(())
    }

    /// Makes the Layout being worked in track no Vault.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] when what it names its Vault in cannot be taken away.
    pub fn un_track_layout(&self) -> Result<(), LayoutError> {
        match fs::remove_file(self.layout_dir().join(LAYOUT_TRACK_FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Where the Workspace's one Layout is kept, with the layout's spelling walked back out of it.
    #[must_use]
    pub fn layout_dir(&self) -> PathBuf {
        self.current_dir.join(LAYOUT_DIR).components().collect()
    }

    /// Takes over the Layout a version before this one kept among several.
    ///
    /// A Workspace made before a Workspace had one Layout kept its Layouts under
    /// [`LEGACY_LAYOUTS_DIR`], one directory each, and named the one it worked in in
    /// [`LEGACY_LAYOUT_PATH`]. What it worked in is the one it goes on working in, so it is moved to
    /// [`LAYOUT_DIR`] and the rest are left where they are: nothing is taken from a Workspace that
    /// is not part of what it worked in, and a layout it did not work in is not one there is a
    /// command to choose any more.
    ///
    /// The move is the reason this is not simply "the Layout is not there": a Workspace that has
    /// none is one there is nothing to adopt from, and that is answered with "no Layout" as it was.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] when the Layout that was worked in is there but cannot be moved.
    fn adopt_legacy_layout(&self) -> Result<(), LayoutError> {
        let legacy = self.current_dir.join(LEGACY_LAYOUTS_DIR);
        if !legacy.is_dir() {
            return Ok(());
        }

        let name = match fs::read_to_string(self.current_dir.join(LEGACY_LAYOUT_PATH)) {
            Ok(text) => text.trim().to_owned(),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                // No name was written down: a Workspace with one Layout worked in the only one it
                // had, and one with several and none named has nothing that says which it worked in.
                let mut names = legacy_names(&legacy)?;
                if names.len() == 1 {
                    names.remove(0)
                } else {
                    return Ok(());
                }
            }
            Err(error) => return Err(error.into()),
        };

        let from = legacy.join(&name);
        if !from.is_dir() {
            return Ok(());
        }

        fs::rename(&from, self.layout_dir())?;

        Ok(())
    }

    /// Where a Vault's Layout copy is kept: the pointer, and a directory per generation.
    ///
    /// The cache is a directory per Vault and a directory per Layout under it, both named by what
    /// the copy was fetched under. Inside that one is what [`LayoutCopy`] makes of it — see the
    /// module it lives in for why a copy is kept in generations rather than replaced whole.
    #[must_use]
    pub fn readonly_layouts_dir(&self, vault: &str, layout: &str) -> PathBuf {
        self.current_dir
            .join(WORKSPACE_READONLY_LAYOUTS_DIR)
            .join(vault)
            .join(layout)
    }

    /// The generation of a Vault's Layout copy a reader is to open, or nothing when none is here.
    ///
    /// What is answered is a copy, never a directory to be opened and made into one: a caller with
    /// nothing here is told so rather than handed a path it would turn into a Layout holding nothing.
    #[must_use]
    pub fn readonly_layout_copy(&self, vault: &str, layout: &str) -> Option<PathBuf> {
        LayoutCopy::of(self, vault, layout).current()
    }

    /// The Layout copies this Workspace keeps, each by the Vault and the Layout it was fetched under.
    ///
    /// A copy is here because a fetch brought it, so what is listed is what a reader can open now, in
    /// name order: a Vault that was bound but never fetched has nothing to read here. The cache is
    /// derived data, and a directory that cannot be read is answered as nothing rather than as a
    /// failure — the same bargain [`readonly_layout_copy`](Self::readonly_layout_copy) makes for one
    /// copy.
    #[must_use]
    pub fn readonly_layout_copies(&self) -> Vec<(String, String)> {
        let root = self.current_dir.join(WORKSPACE_READONLY_LAYOUTS_DIR);
        let Ok(vaults) = fs::read_dir(root) else {
            return Vec::new();
        };

        let mut copies = Vec::new();
        for vault in vaults.filter_map(Result::ok) {
            let Ok(layouts) = fs::read_dir(vault.path()) else {
                continue;
            };
            let vault = vault.file_name().to_string_lossy().into_owned();

            for layout in layouts.filter_map(Result::ok) {
                if !layout.path().is_dir() {
                    continue;
                }

                copies.push((
                    vault.clone(),
                    layout.file_name().to_string_lossy().into_owned(),
                ));
            }
        }

        copies.sort();

        copies
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

/// Where a version before this one kept the several Layouts a Workspace could work in.
///
/// It is read by [`Workspace::adopt_legacy_layout`] and written by nothing: the directory is what a
/// Workspace made by an older version still has, and it goes away when every such Workspace has been
/// opened once.
const LEGACY_LAYOUTS_DIR: &str = "./.rola/layouts/";

/// The file, inside the data directory, naming the Layout an older Workspace worked in.
///
/// It is read by [`Workspace::adopt_legacy_layout`] and written by nothing, for the reason
/// [`LEGACY_LAYOUTS_DIR`] is.
const LEGACY_LAYOUT_PATH: &str = "./.rola/LAYOUT";

/// The names the Layouts an older Workspace kept are known by, in name order.
///
/// # Errors
///
/// Returns [`LayoutError`] when the directory cannot be read.
fn legacy_names(legacy: &Path) -> Result<Vec<String>, LayoutError> {
    let mut names: Vec<String> = fs::read_dir(legacy)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect();
    names.sort_unstable();

    Ok(names)
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

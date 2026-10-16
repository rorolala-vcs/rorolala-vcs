//! How the work stands, as the C ABI reads it.
//!
//! The reading itself is the status module's — see [`Status`], which is `rorolala_status::Status` —
//! and what is added here is the one thing a module cannot reach: which account the work acts as,
//! which belongs to the user rather than to a Vault or a Workspace, and which therefore sits with
//! the crate that ties the modules together.
//!
//! What a C caller gets is the reading and never a failure: a directory no Workspace holds, a
//! Workspace working in no Layout, and a tree or a Layout that cannot be read are all answered as
//! nothing, because a `NULL` is the whole of what this surface can say about any of them.

use std::path::Path;

use rorolala_layout::Layout;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;

pub use rorolala_status::*;

use crate::auth;

/// Reads how the work under `directory` stands beside the Layout it works from.
///
/// The Workspace is searched for upwards from the directory, so any directory inside the work reads
/// the same tree; the Layout read is the one the Workspace is working in, and the account is the one
/// the work acts as, which is what tells a change of this account's from one that is not its to
/// make. The index of the Vault or Workspace the directory is inside is read too, since a variant
/// file that moved is found again by what the index holds.
///
/// # FFI
///
/// A `RolaStatus` the caller releases with `free_rola_status`, or null when there is nothing to
/// read: no Workspace holds the directory, the Workspace works in no Layout, or the tree cannot be
/// read beside the one it does.
#[must_use]
#[lazyffi(export = locate_rola_status)]
pub fn locate_status(directory: &Path) -> Option<Status> {
    let workspace = Workspace::locate(directory)?;
    let layout = working_layout(&workspace)?;
    let index = VCSIndex::locate(directory);

    Status::read(
        &workspace,
        &layout,
        auth::current_account().as_deref(),
        index.as_ref(),
    )
    .ok()
}

/// The Layout a Workspace is working in, when it is working in one.
fn working_layout(workspace: &Workspace) -> Option<Layout> {
    let layouts = workspace.layouts();
    let current = layouts.current().ok().flatten()?;

    layouts.get(&current).ok().flatten()
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString, c_char};
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::LayoutPath;
    use rorolala_utils_constants::DEFAULT_LAYOUT_NAME;
    use rorolala_utils_lazyffi::free_string;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{RolaStatus, Status, free_rola_status, locate_rola_status};

    /// A directory of its own, emptied first so a rerun starts clean.
    ///
    /// The system temp is used when it can be. A machine whose temp is not writable — a Windows
    /// profile can be — falls back to the directory this test's own program sits in, which cargo has
    /// just written there and so is writable wherever the temp is not.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let name = format!(
            "rorolala-locate-status-{}-{label}-{}",
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

    /// A Workspace working in one Layout that names `gone.txt`.
    fn held(label: &str) -> PathBuf {
        let dir = scratch(label);

        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layouts = workspace.layouts();
        let layout = layouts.create(DEFAULT_LAYOUT_NAME).unwrap();
        layouts.set_current(DEFAULT_LAYOUT_NAME).unwrap();

        layout
            .create_path(&LayoutPath::new("gone.txt").unwrap(), Uuid::from_u128(1))
            .unwrap();

        dir
    }

    /// The status the ABI hands out for a directory, which the caller releases.
    fn read(directory: &Path) -> *mut RolaStatus {
        let raw = CString::new(directory.to_str().unwrap()).unwrap();

        // SAFETY: `raw` is a NUL-terminated C string that outlives the call, which is all the export
        // reads it for.
        unsafe { locate_rola_status(raw.as_ptr().cast_mut()) }
    }

    /// What a string the ABI handed out says, released once it has been read.
    ///
    /// # Safety
    ///
    /// `text` must be an owned C string the ABI handed out and has not been released.
    unsafe fn taken(text: *mut c_char) -> String {
        // SAFETY: the caller guarantees the string, as the export's contract says.
        let read = unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned();

        // SAFETY: the same string, released once and not used again.
        unsafe { free_string(text) };

        read
    }

    #[test]
    fn locating_nothing_hands_back_null() {
        let dir = scratch("absent");

        assert!(read(&dir).is_null(), "{dir:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_working_in_no_layout_hands_back_null() {
        let dir = scratch("no-layout");
        Workspace::create(&dir).unwrap();

        assert!(read(&dir).is_null(), "{dir:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_reading_crosses_as_a_handle_its_owner_releases() {
        let dir = held("reading");
        fs::write(dir.join("new.txt"), b"new").unwrap();

        let status = read(&dir);
        assert!(!status.is_null(), "{dir:?}");

        // SAFETY: `status` came from the export above and is read through the accessors it names,
        // in this one scope, and is released at the end of it.
        unsafe {
            assert_eq!(Status::rola_status_lost_count(status), 1);
            assert_eq!(Status::rola_status_untagged_count(status), 1);

            assert_eq!(taken(Status::rola_status_lost_at(status, 0)), "gone.txt");
            assert_eq!(taken(Status::rola_status_untagged_at(status, 0)), "new.txt");

            // Past the count there is nothing to answer with, which is an empty string and never a
            // path: a path a Layout names is never empty.
            assert_eq!(taken(Status::rola_status_lost_at(status, 1)), "");

            free_rola_status(status);
        }

        let _ = fs::remove_dir_all(&dir);
    }
}

//! Who holds what, as the C ABI answers it.
//!
//! The read itself is the Workspace's — see `rorolala_workspace::Ownership` — and what is added
//! here is the one thing a module cannot reach: which account the work acts as, which belongs to
//! the user rather than to a Vault or a Workspace, and which therefore sits with the crate that
//! ties the modules together.

use std::path::Path;

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::{Ownership, Workspace};

use crate::auth;

/// Prepares a Workspace's ownership answers for the directory `directory` sits in.
///
/// The Workspace is searched for upwards from the directory, so any directory inside the work
/// answers the same; a directory no Workspace holds answers nothing, and so does a Workspace that
/// works in no Layout or tracks no Vault.
///
/// # FFI
///
/// A `RolaOwnership` the caller releases with `free_rola_ownership`, or null when there is nothing
/// to answer with.
#[must_use]
#[lazyffi(export = locate_rola_ownership)]
pub fn locate_ownership(directory: &Path) -> Option<Ownership> {
    let workspace = Workspace::locate(directory)?;

    Ownership::open(&workspace, auth::current_account())
}

/// The account the work acts as, as Rorolala's own file for the user names it.
///
/// # FFI
///
/// The account name, or an empty string when none is named. The caller owns the string and
/// releases it with `free_string`.
#[must_use]
#[lazyffi(export = rola_current_account)]
pub fn current_account() -> String {
    auth::current_account().unwrap_or_default()
}

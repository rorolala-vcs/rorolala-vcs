//! The files Rorolala keeps for the user, under their local data directory.
//!
//! These belong to no Vault and no Workspace: they are the user's own, and they are small
//! enough to be plain files rather than configurations. A machine that does not say where
//! its local data directory is — and no run that names one — has nowhere to keep them, so every
//! path here is an `Option`. Which directory that is is
//! [`data_home`](rorolala_utils_location::data_home)'s to say, so a run that must keep its files
//! to itself keeps these with them.

use std::path::PathBuf;

use rorolala_utils_constants::{
    USER_ACCOUNT_FILE, USER_DATA_DIR, USER_HISTORY_FILE, USER_LASTEC_FILE,
};
use rorolala_utils_location::data_home;

/// The directory Rorolala keeps its own files in, if the machine or a run names one.
///
/// On a machine that follows the XDG layout this is `~/.local/share/rola`.
pub fn data_dir() -> Option<PathBuf> {
    data_home().map(|dir| dir.join(USER_DATA_DIR))
}

/// The file the address history is kept in.
pub fn history_path() -> Option<PathBuf> {
    Some(data_dir()?.join(USER_HISTORY_FILE))
}

/// The file the account the work acts as is kept in.
pub fn account_path() -> Option<PathBuf> {
    Some(data_dir()?.join(USER_ACCOUNT_FILE))
}

/// The file the exit code of the last run is kept in.
pub fn lastec_path() -> Option<PathBuf> {
    Some(data_dir()?.join(USER_LASTEC_FILE))
}

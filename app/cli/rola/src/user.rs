//! The files Rorolala keeps for the user, under their user data directory.
//!
//! These belong to no Vault and no Workspace: they are the user's own, and they are small
//! enough to be plain files rather than configurations. A machine that does not say where
//! its local data directory is, and no run that names one, has nowhere to keep them, so every
//! path here is an `Option`. Which directory that is is
//! [`user_data_dir`]'s to say, so a run that must keep its files
//! to itself keeps these with them.

use std::path::PathBuf;

use rorolala_utils_constants::{USER_HISTORY_FILE, USER_LASTEC_FILE};
use rorolala_utils_location::user_data_dir;

/// The directory Rorolala keeps its own files in, if the machine or a run names one.
///
/// It is [`user_data_dir`], which carries the userdata name this build was made for. On a
/// machine that follows the XDG layout that is `~/.local/share/rola/<name>`.
pub fn data_dir() -> Option<PathBuf> {
    user_data_dir()
}

/// The file the address history is kept in.
pub fn history_path() -> Option<PathBuf> {
    Some(data_dir()?.join(USER_HISTORY_FILE))
}

/// The file the exit code of the last run is kept in.
pub fn lastec_path() -> Option<PathBuf> {
    Some(data_dir()?.join(USER_LASTEC_FILE))
}

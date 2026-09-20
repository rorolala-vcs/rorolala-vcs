//! The files Rorolala keeps for the user, under their local data directory.
//!
//! These belong to no Vault and no Workspace: they are the user's own, and they are small
//! enough to be plain files rather than configurations. A machine that does not say where
//! its local data directory is has nowhere to keep them, so every path here is an `Option`.

use std::path::PathBuf;

/// The directory, under the user's local data directory, where Rorolala keeps its files.
const DATA_DIR: &str = "rola";

/// The file the addresses seen so far are kept in, one per line.
const HISTORY_FILE: &str = "addr.hs";

/// The directory Rorolala keeps its own files in, if the machine names one.
///
/// On a machine that follows the XDG layout this is `~/.local/share/rola`.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(DATA_DIR))
}

/// The file the address history is kept in.
pub fn history_path() -> Option<PathBuf> {
    Some(data_dir()?.join(HISTORY_FILE))
}

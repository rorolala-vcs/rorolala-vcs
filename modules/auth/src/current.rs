//! The account the work acts as, kept in Rorolala's own file for the user.
//!
//! It belongs to no Vault and no Workspace: which account the work acts as is the user's own
//! choice, and it is one line of text rather than a configuration. A machine that does not name a
//! local data directory has nowhere to keep it, so the path is an `Option` and a run without a
//! file acts as nobody.

use std::fs;
use std::path::PathBuf;

use rorolala_utils_constants::{USER_ACCOUNT_FILE, USER_DATA_DIR};

/// The file the account the work acts as is kept in, if the machine names a data directory.
#[must_use]
pub fn account_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(USER_DATA_DIR).join(USER_ACCOUNT_FILE))
}

/// The account the work acts as, or nothing when none is named.
///
/// What cannot be read is not a failure: a file that is missing, or one that cannot be read at
/// all, is a run that acts as nobody — the same answer a file naming nothing gives, since a name
/// of whitespace is no name.
#[must_use]
pub fn current_account() -> Option<String> {
    let name = fs::read_to_string(account_path()?).ok()?;
    let name = name.trim();

    (!name.is_empty()).then(|| name.to_owned())
}

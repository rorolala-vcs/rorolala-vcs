//! Reading a word a run wrote as a path of the Layout being worked in.
//!
//! A command that acts on one file takes the word the way a shell would, relative to where the run
//! was made, and then has to answer three questions before the Layout is read: is the word a path
//! at all, is it inside the Workspace, and is it a path of the work rather than of what the
//! Workspace keeps for itself. The rules are one place so that `retrack` and `tweak`, which both
//! take one file and act on its record, cannot come to different answers.

use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use rorolala_layout::LayoutPath;
use rorolala_storage::Key;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::normalize;
use uuid::Uuid;

/// Why a word names no path of the Layout being worked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRefusal {
    /// The word names an entry, a hash or a remote name rather than a path of this tree.
    NotLocal,
    /// The path lies outside the Workspace.
    OutsideWorkspace,
    /// The path lies inside the Workspace's own data directory.
    InsideData,
}

/// The path `given` names, relative to the Workspace rooted at `root`, made absolute against the
/// directory `cwd` the run was made in.
///
/// Only a local path is taken. A `Uuid`, a hash and a remote name are refused by shape rather than
/// answered as a path that happens not to be there, since a run that wrote one meant another
/// command. A name holds `@` often enough in a drawing, `logo@2x.png`, that a plain one is left to
/// be a path: what is refused is the Vault-side marker and the one name a Vault's Layout is kept
/// under.
///
/// # Errors
///
/// Returns [`PathRefusal`] when the word names another kind of thing, or a path outside the
/// Workspace or inside its data directory.
pub fn local_path(root: &Path, cwd: &Path, given: &str) -> Result<LayoutPath, PathRefusal> {
    if Uuid::from_str(given).is_ok() || Key::from_str(given).is_ok() || is_remote(given) {
        return Err(PathRefusal::NotLocal);
    }

    let root = normalize(root);
    let disk = normalize(&resolve(cwd, given));

    let Ok(relative) = disk.strip_prefix(&root) else {
        return Err(PathRefusal::OutsideWorkspace);
    };
    let Ok(path) = LayoutPath::from_relative(relative) else {
        return Err(PathRefusal::OutsideWorkspace);
    };

    if path.as_str().split('/').any(|part| part == ".rola") {
        return Err(PathRefusal::InsideData);
    }

    Ok(path)
}

/// Whether `given` is a name of the Vault's side rather than a path in this tree.
fn is_remote(given: &str) -> bool {
    // `@` is the marker a Vault's own path starts with; `truth@VAULT` is how a Vault's one Layout
    // is named. A file of one's own may hold an `@` and is left to be read as the path it is.
    given.starts_with('@')
        || matches!(given.split_once('@'), Some((layout, vault))
            if !layout.is_empty() && !vault.is_empty() && layout == VAULT_LAYOUT_NAME)
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

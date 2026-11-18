//! Puts a path on one version, and writes down what the record says about it.

use std::path::Path;

use rorolala_layout::Layout;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;
use serde::Serialize;

use crate::write::{
    WriteError,
    checkout::{Restored, restore, restore_vault},
    layout::set_all,
};

/// What aligning one path wrote.
#[derive(Debug, Clone, Serialize)]
pub struct Aligned {
    /// The Layout entry the path names.
    pub id: uuid::Uuid,
    /// The version the Layout now records.
    pub version: rorolala_storage::Key,
}

/// Puts the content of `version` at `path` and makes the entry say what `owner`, `description` and
/// `preference` say.
///
/// # Errors
///
/// Returns the failures [`restore`] names, and [`WriteError::Layout`] when the entry cannot be
/// written.
pub fn align_entry(
    workspace: &Workspace,
    cwd: &Path,
    path: &str,
    version: &str,
    owner: Option<&str>,
    description: &str,
    preference: Option<&str>,
) -> Result<Aligned, WriteError> {
    let restored = restore(workspace, cwd, path, version)?;
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;

    align_in(&layout, &restored, owner, description, preference)
}

/// Puts the content of `version` at `path` in a Vault and makes the entry say what `owner`,
/// `description` and `preference` say.
///
/// # Errors
///
/// Returns the failures [`restore_vault`] names, and [`WriteError::Layout`] when the entry cannot
/// be written.
pub fn align_vault_entry(
    vault: &Vault,
    cwd: &Path,
    path: &str,
    version: &str,
    owner: Option<&str>,
    description: &str,
    preference: Option<&str>,
) -> Result<Aligned, WriteError> {
    let restored = restore_vault(vault, cwd, path, version)?;
    let layout = vault.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;

    align_in(&layout, &restored, owner, description, preference)
}

/// Writes the entry an aligned path now names, in one call.
fn align_in(
    layout: &Layout,
    restored: &Restored,
    owner: Option<&str>,
    description: &str,
    preference: Option<&str>,
) -> Result<Aligned, WriteError> {
    set_all(
        layout,
        restored.id,
        *restored.version.digest(),
        owner,
        description,
        preference,
    )?;

    Ok(Aligned {
        id: restored.id,
        version: restored.version,
    })
}

/// Puts a path under a Workspace on one version and writes what the record says about it.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `owner` and `preference` are empty when the entry has none.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`align_entry`] names.
#[lazyffi(export = rola_write_align)]
pub fn align_at(
    directory: &Path,
    path: &str,
    version: &str,
    owner: &str,
    description: &str,
    preference: &str,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let aligned = align_entry(
        &workspace,
        directory,
        path,
        version,
        (!owner.is_empty()).then_some(owner),
        description,
        (!preference.is_empty()).then_some(preference),
    )?;

    serde_json::to_string(&aligned).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Puts a path under a Vault on one version and writes what the record says about it.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `owner` and `preference` are empty when the entry has none.
///
/// # Errors
///
/// Returns [`WriteError::NoVault`] when no Vault holds `directory`, and the failures
/// [`align_vault_entry`] names.
#[lazyffi(export = rola_write_align_vault)]
pub fn align_vault_at(
    directory: &Path,
    path: &str,
    version: &str,
    owner: &str,
    description: &str,
    preference: &str,
) -> Result<String, WriteError> {
    let vault = Vault::locate(directory).ok_or(WriteError::NoVault)?;
    let aligned = align_vault_entry(
        &vault,
        directory,
        path,
        version,
        (!owner.is_empty()).then_some(owner),
        description,
        (!preference.is_empty()).then_some(preference),
    )?;

    serde_json::to_string(&aligned).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;

    use super::{align_entry, align_vault_entry};
    use crate::write::track::{TrackOptions, track_file, track_vault_file};

    #[test]
    fn a_path_is_aligned_to_a_version_and_a_description() {
        let dir = std::env::temp_dir().join(format!("rola-write-align-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();

        let aligned = align_entry(
            &workspace,
            &dir,
            "a.txt",
            &first.version.to_string(),
            Some("alice"),
            "aligned",
            Some("share"),
        )
        .unwrap();

        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");
        let layout = workspace.layout().unwrap();
        let entry = layout.entry(aligned.id).unwrap();
        assert_eq!(entry.version(), *first.version.digest());
        assert_eq!(entry.description(), "aligned");
        assert_eq!(entry.owner(), Some("alice"));
        assert_eq!(entry.preference(), Some("share"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_path_is_aligned_to_a_version() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-align-vault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Vault::create(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();

        let aligned = align_vault_entry(
            &vault,
            &dir,
            "a.txt",
            &first.version.to_string(),
            Some("alice"),
            "aligned",
            Some("share"),
        )
        .unwrap();

        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");
        let layout = vault.layout().unwrap();
        let entry = layout.entry(aligned.id).unwrap();
        assert_eq!(entry.version(), *first.version.digest());
        assert_eq!(entry.description(), "aligned");
        assert_eq!(entry.owner(), Some("alice"));
        assert_eq!(entry.preference(), Some("share"));

        let _ = fs::remove_dir_all(&dir);
    }
}

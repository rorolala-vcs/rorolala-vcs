//! Writing a tree's description, which is the note the next version carries.

use std::path::Path;

use rorolala_tree::{Staging, Tree};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;

use crate::write::WriteError;

/// Writes the description of the next version of the tree at `target`.
///
/// An empty description clears it. The text is the caller's; this only puts it where the next
/// version reads it.
///
/// # Errors
///
/// Returns [`WriteError::Tree`] when no tree holds `target`, when its staging store cannot be made,
/// or when the description cannot be written.
pub fn write_description(
    workspace: &Workspace,
    target: &Path,
    text: &str,
) -> Result<usize, WriteError> {
    let tree = Tree::locate(workspace.get_root(), target).map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    let staging = Staging::new(tree.root());
    staging.ensure_store().map_err(|error| WriteError::Tree {
        reason: error.to_string(),
    })?;
    staging
        .write_description(Some(text))
        .map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

    Ok(text.len())
}

/// Writes the description of the tree a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"bytes":N}`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `target` is the tree's
/// path, relative to `directory`, and empty for the directory itself.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and
/// [`WriteError::Tree`] when the tree or its staging store cannot be written.
#[lazyffi(export = rola_write_tree_description)]
pub fn write_description_at(
    directory: &Path,
    target: &str,
    text: &str,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let target = if target.is_empty() {
        directory.to_path_buf()
    } else {
        let target = Path::new(target);
        if target.is_absolute() {
            target.to_path_buf()
        } else {
            directory.join(target)
        }
    };
    let bytes = write_description(&workspace, &target, text)?;

    serde_json::to_string(&serde_json::json!({ "bytes": bytes })).map_err(|error| {
        WriteError::Json {
            reason: error.to_string(),
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::write_description;

    #[test]
    fn a_description_is_written_into_the_staging_tree() {
        let dir = std::env::temp_dir().join(format!("rola-write-tree-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::create_dir_all(dir.join("tree")).unwrap();
        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();

        let bytes = write_description(&workspace, &dir.join("tree"), "first").unwrap();

        assert_eq!(bytes, 5);
        let _ = fs::remove_dir_all(&dir);
    }
}

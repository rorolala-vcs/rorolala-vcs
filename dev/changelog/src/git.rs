//! Whether the tree a rewrite would touch has changes in it.
//!
//! A changelog is rewritten by replacing the whole file, and a version is set by rewriting a
//! manifest line. Both are safe only on a tree whose changes are the ones being made, so the check
//! is a refusal rather than an attempt to guess which changes were the user's.
//!
//! The editor's own draft, backup and rewrite files are named `__…` and are ignored by the
//! repository; a tree that does not ignore them still has no changes worth refusing over, so the
//! check leaves those paths out.

use std::ffi::OsString;
use std::path::Path;

use crate::{Error, tool::Tool};

/// The working tree's changes, one porcelain line per path.
///
/// Porcelain rather than a plain `diff HEAD`: it names untracked files as well, which are exactly
/// the ones a first changelog appears as.
///
/// # Errors
///
/// Reports [`Error::Tool`] when `git` cannot be started, and [`Error::Status`] when it ran and
/// failed.
pub fn porcelain(root: &Path) -> Result<String, Error> {
    let program = Tool::git();
    let output = program.capture([
        OsString::from("-C"),
        root.as_os_str().to_owned(),
        OsString::from("status"),
        OsString::from("--porcelain"),
    ])?;

    if !output.status.success() {
        return Err(Error::Status {
            program: program.name(),
            code: output.status.code().unwrap_or(1),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Whether porcelain output describes a tree with changes in it.
///
/// A line is one changed path, so a non-blank line that is not one of the editor's own scratch
/// files is a change. The end-of-options marker and blank lines are not paths.
#[must_use]
pub fn is_dirty(porcelain: &str) -> bool {
    porcelain
        .lines()
        .any(|line| !line.trim().is_empty() && !is_own_scratch(line))
}

/// Whether a porcelain line names a file the editor writes for itself.
fn is_own_scratch(line: &str) -> bool {
    // Porcelain writes `XY path`; the path starts at the third byte. A path with spaces is quoted
    // by git, so the quotes are trimmed before the name is compared.
    let Some(path) = line.get(3..) else {
        return false;
    };
    let path = path.trim().trim_matches('"');

    matches!(
        path,
        "__CHANGELOG_DRAFT.md" | "__CHANGELOG_BACKUP.md" | "__CHANGELOG_REWRITE.md"
    )
}

#[cfg(test)]
mod tests {
    use super::is_dirty;

    #[test]
    fn a_clean_tree_has_no_line() {
        assert!(!is_dirty(""));
        assert!(!is_dirty("  \n\n"));
        assert!(!is_dirty(" \n\t\n"));
    }

    #[test]
    fn a_changed_path_is_a_dirty_tree() {
        assert!(is_dirty(" M dev/changelog/src/git.rs\n"));
        assert!(is_dirty("?? CHANGELOG.md\n"));
        assert!(is_dirty("M  Cargo.toml\nM  Cargo.lock\n"));
    }

    #[test]
    fn the_editors_own_scratch_files_are_not_the_trees_changes() {
        assert!(!is_dirty("?? __CHANGELOG_DRAFT.md\n"));
        assert!(!is_dirty(
            " M __CHANGELOG_BACKUP.md\n M __CHANGELOG_REWRITE.md\n"
        ));
        // A file that merely starts the same way is still the tree's.
        assert!(is_dirty("?? __CHANGELOG_NOTE_BY_THE_USER.md\n"));
        assert!(is_dirty("?? __CHANGELOG_DRAFT.md.bak\n"));
    }
}

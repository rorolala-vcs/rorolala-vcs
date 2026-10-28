//! The commands, one to a module.
//!
//! A command turns its words into a state, and a chain handler does the work. What the handlers
//! share — the refusal of a dirty tree, reading a file that may not be there, and writing the
//! details with the summary regenerated from them — lives here rather than in each of them.

pub mod archive;
pub mod get_version;
pub mod render_summary;
pub mod set_version;
pub mod write;
pub mod write_reset;

use std::path::Path;

use mingling::picker::UnmatchedArgs;

use crate::document;
use crate::paths::{self, Workspace};
use crate::{Error, FailedChangelog, Next, git};

// See the note at the crate root: a command's generated types have to be in scope there, and this is
// what puts them on the way.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use {
    archive::*, get_version::*, render_summary::*, set_version::*, write::*, write_reset::*,
};

/// Refuses a run whose command was left with words it does not know.
pub fn refuse_unmatched(words: &UnmatchedArgs, command: &'static str) -> Option<Next> {
    if words.is_empty() {
        return None;
    }

    let words = words.iter().map(|entry| entry.raw.clone()).collect();

    Some(
        FailedChangelog {
            error: Error::UnknownArgument { command, words },
        }
        .into(),
    )
}

/// Refuses a tree with changes in it, unless the run asked to go on.
pub fn ensure_clean(workspace: &Workspace, allow_dirty: bool) -> Result<(), Error> {
    if allow_dirty {
        return Ok(());
    }

    let porcelain = git::porcelain(workspace.root())?;
    if git::is_dirty(&porcelain) {
        return Err(Error::DirtyTree {
            detail: porcelain.trim_end().to_owned(),
        });
    }

    Ok(())
}

/// A file as text, where not being there is the empty file.
pub fn read_text(path: &Path) -> Result<String, Error> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(Error::Io {
            doing: "reading a file",
            detail: format!("{}: {error}", path.display()),
        }),
    }
}

/// Replaces a file: a backup of the original first, then the rewrite, then one rename.
///
/// The rename is what makes the change atomic: a reader sees the file before the rewrite or the
/// file after it, never a file being written. A rewrite that would write the same bytes back is
/// not made at all, so a run that changed nothing leaves no backup behind.
pub fn replace_atomically(
    target: &Path,
    backup: &Path,
    rewrite: &Path,
    original: &str,
    rewritten: &str,
) -> Result<(), Error> {
    if original == rewritten {
        return Ok(());
    }

    std::fs::write(backup, original).map_err(|error| Error::Io {
        doing: "writing the backup",
        detail: format!("{}: {error}", backup.display()),
    })?;

    std::fs::write(rewrite, rewritten).map_err(|error| Error::Io {
        doing: "writing the rewrite",
        detail: format!("{}: {error}", rewrite.display()),
    })?;

    std::fs::rename(rewrite, target).map_err(|error| Error::Io {
        doing: "replacing the file",
        detail: format!("{} -> {}: {error}", rewrite.display(), target.display()),
    })
}

/// Writes the details, and regenerates the summary from the bytes written.
///
/// The summary is read back out of the details rather than taken from the in-memory document, so
/// it names the numbers and headings the file actually holds. A summary that cannot be written
/// puts the details back: the draft has not been consumed, and a rerun would otherwise add the
/// entry a second time.
pub fn write_changelog(workspace: &Workspace, original: &str, details: &str) -> Result<(), Error> {
    let summary = document::summary(details, paths::DETAILS);
    let summary_original = read_text(&workspace.changelog())?;

    replace_atomically(
        &workspace.details(),
        &workspace.details_backup(),
        &workspace.details_rewrite(),
        original,
        details,
    )?;

    if let Err(error) = replace_atomically(
        &workspace.changelog(),
        &workspace.backup(),
        &workspace.rewrite(),
        &summary_original,
        &summary,
    ) {
        let restored = std::fs::write(workspace.details(), original);

        if let Err(restore) = restored {
            eprintln!(
                "the summary could not be written ({error}) and the details could not be put \
                 back: {restore}"
            );
        }

        return Err(error);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::replace_atomically;

    /// A directory of its own, named so two test binaries cannot meet in it.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rola-changelog-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The target, backup and rewrite a test writes through.
    fn paths(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        (
            dir.join("CHANGELOG.md"),
            dir.join("__CHANGELOG_BACKUP.md"),
            dir.join("__CHANGELOG_REWRITE.md"),
        )
    }

    #[test]
    fn a_rewrite_that_would_write_the_same_bytes_is_not_made() {
        let dir = scratch("same");
        let (target, backup, rewrite) = paths(&dir);
        std::fs::write(&target, "same\n").unwrap();

        replace_atomically(&target, &backup, &rewrite, "same\n", "same\n").unwrap();

        assert!(!backup.exists());
        assert!(!rewrite.exists());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "same\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_rewrite_leaves_the_original_beside_the_new_file() {
        let dir = scratch("changed");
        let (target, backup, rewrite) = paths(&dir);
        std::fs::write(&target, "old\n").unwrap();

        replace_atomically(&target, &backup, &rewrite, "old\n", "new\n").unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new\n");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "old\n");
        assert!(!rewrite.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}

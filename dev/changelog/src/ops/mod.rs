//! The commands, one to a module.
//!
//! A command turns its words into a state, and a chain handler does the work. What the handlers
//! share — the refusal of a dirty tree, reading a file that may not be there, and replacing the
//! changelog through a rewrite — lives here rather than in each of them.

pub mod archive;
pub mod get_version;
pub mod set_version;
pub mod write;
pub mod write_reset;

use std::path::Path;

use mingling::picker::UnmatchedArgs;

use crate::paths::Workspace;
use crate::{Error, FailedChangelog, Next, git};

// See the note at the crate root: a command's generated types have to be in scope there, and this is
// what puts them on the way.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use {archive::*, get_version::*, set_version::*, write::*, write_reset::*};

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

/// Replaces the changelog: a backup of the original first, then the merge, then one rename.
///
/// The rename is what makes the change atomic: a reader of `CHANGELOG.md` sees the version before
/// the rewrite or the version after it, never a file being written.
pub fn replace_atomically(
    workspace: &Workspace,
    original: &str,
    rewritten: &str,
) -> Result<(), Error> {
    let backup = workspace.backup();
    std::fs::write(&backup, original).map_err(|error| Error::Io {
        doing: "writing the changelog backup",
        detail: format!("{}: {error}", backup.display()),
    })?;

    let rewrite = workspace.rewrite();
    std::fs::write(&rewrite, rewritten).map_err(|error| Error::Io {
        doing: "writing the changelog rewrite",
        detail: format!("{}: {error}", rewrite.display()),
    })?;

    let changelog = workspace.changelog();
    std::fs::rename(&rewrite, &changelog).map_err(|error| Error::Io {
        doing: "replacing the changelog",
        detail: format!("{} -> {}: {error}", rewrite.display(), changelog.display()),
    })
}

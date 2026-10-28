//! `write`: the draft, merged into the version the workspace is at.
//!
//! The merge is a rewrite of the whole details file, so it is assembled in a scratch file and
//! renamed over it; the summary at the repository root is generated from the details in the same
//! run. The draft is consumed on success: it is one pending entry, and a draft left behind would be
//! written again by the next run.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use super::{ensure_clean, read_text, write_changelog};
use crate::document::Document;
use crate::paths::Workspace;
use crate::setup::ResAllowDirty;
use crate::{Error, Next, draft, run_step};

/// Merges the draft into the details.
#[command(node = "write")]
pub fn write() -> StateWrite {
    StateWrite
}

/// The state the command starts in: it names nothing, since the draft is what is written.
#[derive(Grouped)]
pub struct StateWrite;

/// Merges the draft, and consumes it.
#[chain]
pub fn handle_write(_state: StateWrite, allow_dirty: &ResAllowDirty, ec: &mut ResExitCode) -> Next {
    run_step(ec, || write_draft(allow_dirty.allowed()));

    empty_result!()
}

/// Merges the draft into the current version's details, and regenerates the summary.
fn write_draft(allow_dirty: bool) -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    let draft_path = workspace.draft();

    if !draft_path.is_file() {
        eprintln!("no draft at {}; nothing to write", draft_path.display());

        return Ok(());
    }

    let (category, entry) = draft::read(&draft_path)?;
    let summary = entry.summary.clone();
    let version = workspace.version()?;

    ensure_clean(&workspace, allow_dirty)?;

    let original = read_text(&workspace.details())?;
    let mut document = Document::parse(&original);

    document.add_entry(&version, category, entry)?;

    write_changelog(&workspace, &original, &document.render())?;

    std::fs::remove_file(&draft_path).map_err(|error| Error::Io {
        doing: "removing the draft the changelog was written from",
        detail: format!("{}: {error}", draft_path.display()),
    })?;

    eprintln!("wrote `{summary}` into {version} (Unreleased)");

    Ok(())
}

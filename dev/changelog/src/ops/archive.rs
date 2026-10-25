//! `archive`: the newest open version, dated and sealed.
//!
//! What is sealed is the first version still marked `(Unreleased)` — the one a release is being made
//! from — rather than a version looked up from the manifest. The two agree in the usual order, and
//! sealing the section that is open is what makes a version bumped before its archive still
//! releasable.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use super::{ensure_clean, read_text, replace_atomically};
use crate::document::Document;
use crate::paths::Workspace;
use crate::setup::ResAllowDirty;
use crate::{Error, Next, date, run_step};

/// Dates the newest open version and seals it.
#[command(node = "archive")]
pub fn archive() -> StateArchive {
    StateArchive
}

/// The state the command starts in: it names nothing, since the open version is what is sealed.
#[derive(Grouped)]
pub struct StateArchive;

/// Seals the newest open version.
#[chain]
pub fn handle_archive(
    _state: StateArchive,
    allow_dirty: &ResAllowDirty,
    ec: &mut ResExitCode,
) -> Next {
    run_step(ec, || archive_current(allow_dirty.allowed()));

    empty_result!()
}

/// Replaces the newest `(Unreleased)` marker with today's UTC date.
fn archive_current(allow_dirty: bool) -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    ensure_clean(&workspace, allow_dirty)?;

    let date = date::today_utc()?;
    let original = read_text(&workspace.changelog())?;
    let mut document = Document::parse(&original);

    let version = document.archive_newest(&date)?;

    replace_atomically(&workspace, &original, &document.render())?;

    eprintln!("archived {version} ({date})");

    Ok(())
}

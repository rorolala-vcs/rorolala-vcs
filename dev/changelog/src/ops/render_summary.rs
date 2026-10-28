//! `render-summary`: `CHANGELOG.md` regenerated from the details.
//!
//! The summary is a function of the details, so this is the command to run after a hand edit of a
//! title, a scope or the breaking flag — the parts the summary names. It writes the summary only
//! when its bytes would change, and it does not refuse a dirty tree: a hand edit of the details is
//! the input it is rendering rather than a change it would mix its rewrite into.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use super::{read_text, replace_atomically};
use crate::document;
use crate::paths::{self, Workspace};
use crate::{Error, Next, run_step};

/// Regenerates the summary from the details.
#[command(node = "render-summary")]
pub fn render_summary() -> StateRenderSummary {
    StateRenderSummary
}

/// The state the command starts in: it names nothing.
#[derive(Grouped)]
pub struct StateRenderSummary;

/// Regenerates the summary.
#[chain]
pub fn handle_render_summary(_state: StateRenderSummary, ec: &mut ResExitCode) -> Next {
    run_step(ec, render);

    empty_result!()
}

/// Writes `CHANGELOG.md` from `dev/CHANGELOG-DETAILS.md`.
fn render() -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    let details_path = workspace.details();

    if !details_path.is_file() {
        return Err(Error::DetailsMissing { path: details_path });
    }

    let details = std::fs::read_to_string(&details_path).map_err(|error| Error::Io {
        doing: "reading the details",
        detail: format!("{}: {error}", details_path.display()),
    })?;
    let summary = document::summary(&details, paths::DETAILS);
    let original = read_text(&workspace.changelog())?;
    let changed = original != summary;

    replace_atomically(
        &workspace.changelog(),
        &workspace.backup(),
        &workspace.rewrite(),
        &original,
        &summary,
    )?;

    if changed {
        eprintln!(
            "rendered {} from {}",
            workspace.changelog().display(),
            paths::DETAILS
        );
    } else {
        eprintln!("{} is already up to date", workspace.changelog().display());
    }

    Ok(())
}

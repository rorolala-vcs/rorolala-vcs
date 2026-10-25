//! `write-reset`: the draft, emptied back to its template.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::paths::Workspace;
use crate::{Error, Next, run_step};

/// The draft a person fills in before `write`.
///
/// The comments are there to be read and are ignored by YAML; the empty summary is what `write`
/// refuses until it is filled in, which keeps the template from being written by accident.
const DRAFT_TEMPLATE: &str = concat!(
    "---\n",
    "# What changed, in one line.\n",
    "summary: \"\"\n",
    "# bugfix | optimized | feature | refactor\n",
    "category: bugfix\n",
    "# The scopes this touches, primary first: [\"app\", \"rola\", \"checkin\"].\n",
    "scope: []\n",
    "# Whether this is an incompatible change.\n",
    "breaking: false\n",
    "---\n",
    "\n",
);

/// Resets the draft, creating it when it is not there.
#[command(node = "write-reset")]
pub fn write_reset() -> StateWriteReset {
    StateWriteReset
}

/// The state the command starts in: it names nothing.
#[derive(Grouped)]
pub struct StateWriteReset;

/// Resets the draft.
#[chain]
pub fn handle_write_reset(_state: StateWriteReset, ec: &mut ResExitCode) -> Next {
    run_step(ec, reset);

    empty_result!()
}

/// Writes the template over whatever the draft held.
fn reset() -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    let draft = workspace.draft();

    std::fs::write(&draft, DRAFT_TEMPLATE).map_err(|error| Error::Io {
        doing: "writing the draft",
        detail: format!("{}: {error}", draft.display()),
    })?;

    eprintln!("reset {}", draft.display());

    Ok(())
}

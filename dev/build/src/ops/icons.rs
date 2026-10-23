//! `icons`: the Desktop's own pictures, drawn from the icon sets the tree carries.

use crate::toolchain::Tool;
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::run_step;

/// Draws the Desktop's own pictures from the icon sets the descriptions name.
#[command(node = "icons")]
pub fn icons() -> StateIcons {
    StateIcons
}

/// The state the command starts in: it names nothing, since the manifest in the tree names every
/// picture that is drawn.
#[derive(Grouped)]
pub struct StateIcons;

/// Runs the tree's icon tool.
///
/// What is drawn, and where it goes, is what `app/desktop/icons.toml` says: this is only the way to
/// run the tool that reads it, so that a picture is drawn the same way from a terminal, from an
/// export, and on any machine the tree is checked out on.
#[chain]
pub fn handle_icons(_state: StateIcons, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        Tool::cargo().run(["run", "--quiet", "-p", "rorolala-dev-icons"])
    });

    empty_result!()
}

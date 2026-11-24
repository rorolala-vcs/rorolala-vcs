//! `icons`: the Desktop's own pictures, drawn from the icon sets the tree carries.

use std::path::Path;

use crate::toolchain::Tool;
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Error;
use crate::Next;
use crate::fs;
use crate::paths::Workspace;
use crate::run_step;

/// The license the drawn pictures are distributed under, from the root of the tree.
///
/// It is the copy `cargo dev icons` writes beside the host's pictures, and its text is the set's own
/// `LICENSE` at the pinned commit. One copy travels rather than six, because the six are one license.
const LICENSE: &str = "app/desktop/Core/icons/LICENSE";

/// Where the license is laid beside the programs, from the Desktop directory.
pub const HELD: &str = "THIRD_PARTY/Icons/Phosphor/LICENSE";

/// Lays the icon set's license beside the programs, so a hand-off carries the terms it is under.
///
/// The pictures travel inside the assemblies, so this is the only file of the set a reader of an
/// export can find: without it the layout says where its face came from and not where its marks did.
///
/// # Errors
///
/// Reports a copy that failed.
pub fn lay_out_icon_license(workspace: &Workspace, desktop: &Path) -> Result<(), Error> {
    fs::copy_file(&workspace.root().join(LICENSE), &desktop.join(HELD))
}

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

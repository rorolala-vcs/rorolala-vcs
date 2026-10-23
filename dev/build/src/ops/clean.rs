//! `clean`: everything the build ever produced, removed.
//!
//! What it names is what the build writes: cargo's output, whatever else the target directory holds
//! — the generated header, mingling's completion scripts — .NET's output, and the exported `build/`.
//! The tree itself is not touched, and neither is anything outside it: what is removed is a
//! directory the paths module said where it is.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::fs;
use crate::paths::Workspace;
use crate::run_step;
use crate::toolchain::Tool;

/// Removes everything the build produced.
#[command(node = "clean")]
pub fn clean() -> StateClean {
    StateClean
}

/// The state the command starts in: it names nothing, since everything the build produced is what is
/// removed.
#[derive(Grouped)]
pub struct StateClean;

/// Removes what the build wrote, by the names the build gave it.
pub fn clean_everything() -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    Tool::cargo().run(["clean"])?;
    fs::remove(&workspace.target_dir()?)?;
    fs::remove(&workspace.cs_target_dir()?)?;
    fs::remove(&workspace.build_dir()?)?;

    Ok(())
}

/// Removes everything the build produced.
#[chain]
pub fn handle_clean(_state: StateClean, ec: &mut ResExitCode) -> Next {
    run_step(ec, clean_everything);

    empty_result!()
}

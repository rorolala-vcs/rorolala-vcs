//! `build`: everything that ships — the library and the programs.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use super::{library::build_library, programs::build_programs};
use crate::Next;
use crate::run_step;

/// Builds everything that ships.
#[command(node = "build")]
pub fn build() -> StateBuild {
    StateBuild
}

/// The state the command starts in: it names nothing, since everything that ships is what is built.
#[derive(Grouped)]
pub struct StateBuild;

/// Builds everything that ships: the library first, and the programs after it.
///
/// The order is the order of what a program is built against: the programs link the library, so a
/// build that ran them the other way round would be building one against the last library there
/// happened to be.
///
/// # Errors
///
/// Reports whichever of the two builds failed first.
pub fn build_everything() -> Result<(), crate::Error> {
    build_library()?;
    build_programs()
}

/// Builds everything that ships.
#[chain]
pub fn handle_build(_state: StateBuild, ec: &mut ResExitCode) -> Next {
    run_step(ec, build_everything);

    empty_result!()
}

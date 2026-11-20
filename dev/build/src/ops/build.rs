//! `build`: everything that ships, the library and the programs.

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

/// Builds everything that ships, for the userdata name `userdata_name`.
///
/// The name is what every artifact is compiled with, so the library and the programs a run links
/// against it always name the same directory.
///
/// # Errors
///
/// Reports whichever of the two builds failed first.
pub fn build_everything(userdata_name: &str) -> Result<(), crate::Error> {
    build_library(userdata_name)?;
    build_programs(userdata_name)
}

/// Builds everything that ships.
#[chain]
pub fn handle_build(_state: StateBuild, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        build_everything(&crate::userdata_name_from_environment())
    });

    empty_result!()
}

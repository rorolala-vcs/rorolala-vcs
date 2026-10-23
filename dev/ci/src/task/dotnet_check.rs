//! `dotnet-check`: the .NET side of the workspace, built and tested.
//!
//! The solution is built first, and deliberately: `test` builds only what a test project depends on,
//! and nothing depends on the Desktop program, so without the build a Desktop program that does not
//! compile would be found by neither this check nor `test`.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{dotnet, paths::Workspace};

use super::run_check;
use crate::Next;

/// Builds and tests the .NET side of the workspace.
#[command(node = "dotnet-check")]
pub fn dotnet_check() -> StateDotnetCheck {
    StateDotnetCheck
}

/// The state the check starts in: it names nothing, since the whole solution is what is checked.
#[derive(Grouped)]
pub struct StateDotnetCheck;

/// Restores once, then builds and tests the solution with `--no-restore`.
#[chain]
pub fn handle_dotnet_check(_state: StateDotnetCheck, ec: &mut ResExitCode) -> Next {
    run_check(
        "dotnet-check",
        "dotnet build/test RorolalaSharp.sln -c Release --no-restore",
        ec,
        || dotnet::check_solution(&Workspace::discover()?),
    );

    empty_result!()
}

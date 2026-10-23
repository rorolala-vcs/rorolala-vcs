//! `dotnet-lint`: the .NET side of the workspace, held to every rule the analyzers carry.
//!
//! This is the counterpart of `clippy` for the Rust side, and it is a check of its own for the same
//! reason `clippy` is: the extra tier — `AnalysisMode=All`, which is every rule rather than the
//! recommended few — is asked for here rather than in `Directory.Build.props`, so that an ordinary
//! build is held to the warnings the compiler already raises while this gate is held to the whole of
//! what the analyzers say.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{dotnet, paths::Workspace};

use super::run_check;
use crate::Next;

/// Lints the .NET side of the workspace.
#[command(node = "dotnet-lint")]
pub fn dotnet_lint() -> StateDotnetLint {
    StateDotnetLint
}

/// The state the check starts in: it names nothing, since the whole solution is what is linted.
#[derive(Grouped)]
pub struct StateDotnetLint;

/// Rebuilds the solution with every analyzer rule on and any diagnostic failing the run.
#[chain]
pub fn handle_dotnet_lint(_state: StateDotnetLint, ec: &mut ResExitCode) -> Next {
    run_check(
        "dotnet-lint",
        "dotnet msbuild RorolalaSharp.sln -t:Rebuild -warnAsError -p:AnalysisMode=All",
        ec,
        || dotnet::lint_solution(&Workspace::discover()?),
    );

    empty_result!()
}

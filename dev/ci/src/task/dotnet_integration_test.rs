//! `dotnet-integration-test`: the .NET integration suites under `tests/`.
//!
//! Like the cargo suites beside them, they exercise the Desktop program the way a caller meets it,
//! so what is checked is the program rather than the parts it is made of; unlike them they are run
//! by the .NET test host, since the Desktop program is a .NET program and its suites are written in
//! C#.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{dotnet, paths::Workspace};

use super::run_check;
use crate::Next;

/// Runs the .NET integration suites.
#[command(node = "dotnet-integration-test")]
pub fn dotnet_integration_test() -> StateDotnetIntegrationTest {
    StateDotnetIntegrationTest
}

/// The state the check starts in: it names nothing, since every suite under `tests/` is what is run.
#[derive(Grouped)]
pub struct StateDotnetIntegrationTest;

/// Runs each suite on its own, restoring it first and running it with `--no-restore`.
#[chain]
pub fn handle_dotnet_integration_test(
    _state: StateDotnetIntegrationTest,
    ec: &mut ResExitCode,
) -> Next {
    run_check(
        "dotnet-integration-test",
        "dotnet test tests/*/*.IntegrationTests.csproj -c Release --no-restore",
        ec,
        || dotnet::run_integration_suites(&Workspace::discover()?),
    );

    empty_result!()
}

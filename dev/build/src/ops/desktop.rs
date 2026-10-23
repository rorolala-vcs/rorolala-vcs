//! `desktop`: the Desktop program on its own, laid out where an export lays it out.
//!
//! The C ABI is built first, and deliberately: a plugin that answers who holds an entry binds the
//! library, and both the library and the header the bindings are generated from are the build's
//! output. Building the Desktop without them would be building against the last ones there happened
//! to be, or failing at the header.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::dotnet;
use crate::paths::Workspace;
use crate::run_step;
use crate::{Next, ops::library::build_library};

/// Lays the Desktop program out where an export lays it out.
#[command(node = "desktop")]
pub fn desktop() -> StateDesktop {
    StateDesktop
}

/// The state the command starts in: it names nothing, since the Desktop program is what is laid out.
#[derive(Grouped)]
pub struct StateDesktop;

/// Builds the library and the solution, then lays the program, its launcher and its plugins down.
///
/// The whole solution is restored and built once for the two layouts rather than a project at a
/// time: the restore is shared, and one `MSBuild` over the solution is what builds the host and the
/// plugins together.
pub fn lay_out_desktop() -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    build_library()?;
    dotnet::build_solution(&workspace)?;

    let directory = workspace.desktop_dir()?;
    dotnet::publish_desktop(&workspace, &directory)?;
    dotnet::publish_launcher(&workspace, &directory)?;
    dotnet::publish_plugins(&workspace, &directory)?;

    Ok(())
}

/// Lays the Desktop program out.
#[chain]
pub fn handle_desktop(_state: StateDesktop, ec: &mut ResExitCode) -> Next {
    run_step(ec, lay_out_desktop);

    empty_result!()
}

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
use crate::ops::book::build_book;
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
    dotnet::publish_desktop(&workspace, &directory, false)?;
    dotnet::publish_host(&workspace, &directory, false)?;
    dotnet::publish_launcher(&workspace, &directory, false)?;
    dotnet::publish_plugins(&workspace, &directory)?;
    lay_out_presets(&workspace)?;

    // The Desktop program searches the book laid beside the programs, so a layout a person runs
    // by hand carries it too; without it the program opens with nothing to search.
    build_book(&workspace)?;

    Ok(())
}

/// Lays the preset layouts beside the program, where the menu that switches them reads them.
///
/// The presets are the tree's own files rather than files written for a layout, because a second copy is a
/// copy that goes stale. `__raw` is the set the adjusted presets were made from and is not laid down: the
/// program reads one file a preset, and the files under a directory of their own would be presets named after
/// a directory.
///
/// # Errors
///
/// Reports a read or a copy's own failure.
pub fn lay_out_presets(workspace: &Workspace) -> Result<(), crate::Error> {
    let source = workspace.root().join("app/desktop/Presets/Layouts");
    let destination = workspace.desktop_dir()?.join("layout-presets");

    crate::fs::remove(&destination)?;

    for file in crate::fs::read_files(&source)? {
        if !file
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            continue;
        }

        let Some(name) = file.file_name() else {
            continue;
        };

        crate::fs::copy_file(&file, &destination.join(name))?;
    }

    Ok(())
}

/// Lays the Desktop program out.
#[chain]
pub fn handle_desktop(_state: StateDesktop, ec: &mut ResExitCode) -> Next {
    run_step(ec, lay_out_desktop);

    empty_result!()
}

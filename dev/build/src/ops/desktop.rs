//! `desktop`: the Desktop program, built.
//!
//! The C ABI is built first, and deliberately: a plugin that answers who holds an entry binds the
//! library, and both the library and the header the bindings are generated from are the build's
//! output. Building the Desktop without them would be building against the last ones there happened
//! to be, or failing at the header.
//!
//! `export-desktop` is the same build laid out where an export lays it out, with the C# bindings and
//! the two `NuGet` packages a .NET consumer restores.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::dotnet;
use crate::ops::adwaita::fetch_adwaita;
use crate::ops::book::build_book;
use crate::ops::fonts::{fetch_fonts, lay_out_fonts};
use crate::ops::icons::lay_out_icon_license;
use crate::paths::Workspace;
use crate::run_step;
use crate::toolchain::Tool;
use crate::{Next, ops::library::build_library};

/// Builds the Desktop program and its C ABI.
#[command(node = "desktop")]
pub fn desktop(args: Vec<String>) -> StateDesktop {
    StateDesktop {
        userdata_name: crate::userdata_name_from_args(&args),
    }
}

/// The state the command starts in: the userdata name the build is made for.
#[derive(Grouped)]
pub struct StateDesktop {
    /// The name the Desktop program discovers its own files by.
    userdata_name: String,
}

/// Builds the library and the whole .NET solution, so the Desktop program is compiled.
///
/// The font files are fetched first, and deliberately: the shared project embeds them, so a build that
/// started without them would be a program drawn in the machine's face rather than this one.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] when the name is not one directory segment, and whatever the
/// font fetch, the library build or the solution build reports.
pub fn build_desktop(userdata_name: &str) -> Result<(), crate::Error> {
    let userdata_name = crate::checked_userdata_name(userdata_name)?;
    let workspace = Workspace::discover()?;

    fetch_fonts(&workspace)?;
    build_library(userdata_name)?;
    dotnet::build_solution(&workspace, userdata_name)
}

/// Builds the Desktop program and lays it, its launcher, its plugins and the presets down.
///
/// `self_contained` is what the program and the launcher are published as: a developer's layout
/// leaves the .NET runtime to the machine, a package carries it.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] when the name is not one directory segment, and the first
/// failing step after that.
pub fn lay_out_desktop(userdata_name: &str, self_contained: bool) -> Result<(), crate::Error> {
    let userdata_name = crate::checked_userdata_name(userdata_name)?;
    let workspace = Workspace::discover()?;

    // The Adwaita set is packaged beside the Desktop from the tree, so it is fetched before the
    // program is published: a layout without it offers only the system's own icons.
    fetch_adwaita(&workspace)?;

    build_desktop(userdata_name)?;

    let directory = workspace.desktop_dir()?;
    dotnet::publish_desktop(&workspace, &directory, self_contained, userdata_name)?;
    dotnet::publish_host(&workspace, &directory, self_contained, userdata_name)?;
    dotnet::publish_launcher(&workspace, &directory, self_contained, userdata_name)?;
    dotnet::publish_plugins(&workspace, &directory)?;
    lay_out_presets(&workspace)?;
    lay_out_fonts(&workspace, &directory)?;
    lay_out_icon_license(&workspace, &directory)?;

    // The Desktop program searches the book laid beside the programs, so a layout a person runs
    // by hand carries it too; without it the program opens with nothing to search.
    build_book(&workspace)?;

    Ok(())
}

/// Builds the Desktop program and lays out everything the export carries for it, under `build/`.
///
/// The C# bindings are laid beside the native library they bind, under `build/lib`, and the two
/// `NuGet` packages are written there as well, since that is what a .NET consumer restores.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] when the name is not one directory segment, and the first
/// failing step after that.
pub fn export_desktop(userdata_name: &str, self_contained: bool) -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    // The pictures are drawn from the pinned icon set before the solution is built, so what the
    // program carries is what the pin holds rather than whatever was last in the tree.
    Tool::cargo().run(["run", "--quiet", "-p", "rorolala-dev-icons"])?;

    lay_out_desktop(userdata_name, self_contained)?;

    dotnet::publish_rsharp(&workspace)?;
    crate::nuget::pack_rsharp(&workspace)
}

/// Lays the preset layouts beside the program, where the title row's tabs read them.
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

/// Builds the Desktop program.
#[chain]
pub fn handle_desktop(state: StateDesktop, ec: &mut ResExitCode) -> Next {
    run_step(ec, || build_desktop(&state.userdata_name));

    empty_result!()
}

/// Builds the Desktop program and lays it out under `build/`.
#[command(node = "export-desktop")]
pub fn export_desktop_command(args: Vec<String>) -> StateExportDesktop {
    StateExportDesktop {
        userdata_name: crate::userdata_name_from_args(&args),
    }
}

/// The state the command starts in: the userdata name the export is laid out for.
#[derive(Grouped)]
pub struct StateExportDesktop {
    /// The name the exported program discovers its own files by.
    userdata_name: String,
}

/// Builds the Desktop program and lays it out under `build/`.
#[chain]
pub fn handle_export_desktop(state: StateExportDesktop, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        export_desktop(&state.userdata_name, false)?;
        crate::ops::export::mark_export_integrity(&Workspace::discover()?)
    });

    empty_result!()
}

//! `desktop`: the Desktop program and its launcher.
//!
//! The part is three programs: the window, the host that starts it, and the launcher. `desktop`
//! builds those three and nothing else, against whatever Rust output is already there: the plugins
//! are [`crate::ops::desktop_plugin`]'s, the window over the daemon is [`crate::ops::daemon_gui`]'s,
//! and a build that fails because the Rust side is stale or absent is the signal to take the full
//! `export` path instead.
//!
//! `export-desktop` publishes the same three into `build/bin/desktop`. It lays out only what it names
//! and does not make the rest of the layout consistent: a missing plugin or daemon window is not its
//! concern, each of those being exported by its own command. What it is for is a short loop for a
//! change to the Desktop itself.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::dotnet;
use crate::ops::adwaita::fetch_adwaita;
use crate::ops::fonts::{fetch_fonts, lay_out_fonts};
use crate::ops::icons::lay_out_icon_license;
use crate::paths::Workspace;
use crate::run_step;

/// Builds the Desktop program and its launcher.
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

/// Builds the Desktop program, its host and its launcher.
///
/// It builds against whatever is already there: the C ABI and its header, the fonts, the drawn
/// pictures. None of them is fetched or rebuilt here, and nothing else is built either.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] when the name is not one directory segment, and the build's
/// own failure, including the missing C header a build without the Rust side meets.
pub fn build_desktop(userdata_name: &str) -> Result<(), crate::Error> {
    let userdata_name = crate::checked_userdata_name(userdata_name)?;
    let workspace = Workspace::discover()?;

    dotnet::build_desktop_programs(&workspace, userdata_name)
}

/// Publishes the Desktop program, its host and its launcher under `build/bin/desktop`.
///
/// `self_contained` is what the programs are published as: a developer's layout leaves the .NET
/// runtime to the machine, a package carries it.
///
/// The fonts and the Adwaita set are fetched first, since the shared project embeds the fonts and the
/// Adwaita set is packaged beside the program; a set already there is only read.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] when the name is not one directory segment, and the first
/// failing step after that.
pub fn export_desktop(userdata_name: &str, self_contained: bool) -> Result<(), crate::Error> {
    let userdata_name = crate::checked_userdata_name(userdata_name)?;
    let workspace = Workspace::discover()?;

    // Before the solution is evaluated: the shared font project embeds the files, and a layout
    // without the Adwaita set offers only the system's own icons.
    fetch_fonts(&workspace)?;
    fetch_adwaita(&workspace)?;

    let directory = workspace.desktop_dir()?;
    dotnet::publish_desktop(&workspace, &directory, self_contained, userdata_name)?;
    dotnet::publish_host(&workspace, &directory, self_contained, userdata_name)?;
    dotnet::publish_launcher(&workspace, &directory, self_contained, userdata_name)?;
    lay_out_presets(&workspace)?;
    lay_out_fonts(&workspace, &directory)?;
    lay_out_icon_license(&workspace, &directory)?;

    Ok(())
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

/// Publishes the Desktop program and its launcher under `build/bin/desktop`.
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

/// Publishes the Desktop program and its launcher under `build/`.
#[chain]
pub fn handle_export_desktop(state: StateExportDesktop, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        export_desktop(&state.userdata_name, false)?;
        crate::ops::export::mark_export_integrity(&Workspace::discover()?)
    });

    empty_result!()
}

//! `desktop-plugin`: the plugins the Desktop program ships.
//!
//! A plugin is a project of its own, so building one alone is what a change to a plugin needs: the
//! Rust side and the rest of the Desktop are left as they are. `export-desktop-plugin` is the same
//! build, laid down under `build/bin/desktop/plugins` where the program looks for it.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::dotnet;
use crate::paths::Workspace;
use crate::run_step;

/// Builds the plugins that ship with the Desktop program.
#[command(node = "desktop-plugin")]
pub fn desktop_plugin() -> StateDesktopPlugin {
    StateDesktopPlugin
}

/// The state the command starts in: it names nothing, since every plugin is what is built.
#[derive(Grouped)]
pub struct StateDesktopPlugin;

/// Builds every plugin, for `userdata_name`.
///
/// # Errors
///
/// Reports a project the workspace cannot name, the restore, or the build's own failure.
pub fn build_desktop_plugins(userdata_name: &str) -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    dotnet::build_plugins(&workspace, userdata_name)
}

/// Builds the plugins.
#[chain]
pub fn handle_desktop_plugin(_state: StateDesktopPlugin, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        build_desktop_plugins(&crate::userdata_name_from_environment())
    });

    empty_result!()
}

/// Builds the plugins and lays them out under `build/bin/desktop/plugins`.
#[command(node = "export-desktop-plugin")]
pub fn export_desktop_plugin(args: Vec<String>) -> StateExportDesktopPlugin {
    StateExportDesktopPlugin {
        userdata_name: crate::userdata_name_from_args(&args),
    }
}

/// The state the command starts in: the userdata name the export's plugins are built with.
#[derive(Grouped)]
pub struct StateExportDesktopPlugin {
    /// The name the plugins are built with.
    userdata_name: String,
}

/// Builds the plugins and lays them out where the Desktop program looks for them.
///
/// # Errors
///
/// Reports a project the workspace cannot name, the restore, the build, or a copy's own failure.
pub fn export_desktop_plugins(userdata_name: &str) -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    dotnet::build_plugins(&workspace, userdata_name)?;
    dotnet::publish_plugins(&workspace, &workspace.desktop_dir()?)
}

/// Builds the plugins and lays them out under `build/`.
#[chain]
pub fn handle_export_desktop_plugin(state: StateExportDesktopPlugin, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        export_desktop_plugins(&state.userdata_name)?;
        crate::ops::export::mark_export_integrity(&Workspace::discover()?)
    });

    empty_result!()
}

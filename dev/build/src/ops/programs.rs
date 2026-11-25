//! `cli`: the release command line programs.
//!
//! Compiling them is also what refreshes the completion scripts mingling writes beside cargo's
//! output, so a program added to [`crate::paths::PROGRAMS`] is one a shell can be completed for
//! without anything here changing.
//!
//! `export-cli` is the same build, laid out under `build/bin` with the completion scripts a shell
//! sources. The window over the daemon is not among them: it belongs to the Desktop part, see
//! [`crate::ops::daemon_gui`].

use std::ffi::OsStr;

use rorolala_utils_constants::USERDATA_NAME_ENV_VAR;

use crate::paths::{COMPLETION_SHELLS, PROGRAMS, PROGRAMS_WITHOUT_COMPLETIONS, Workspace};
use crate::platform::Platform;
use crate::toolchain::Tool;
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::run_step;

/// Builds the release command line programs.
#[command(node = "cli")]
pub fn cli() -> StateCli {
    StateCli
}

/// The state the command starts in: it names nothing, since every program is what is built.
#[derive(Grouped)]
pub struct StateCli;

/// Builds every program, named to cargo one at a time, for `userdata_name`.
///
/// A program is named with its own `-p` rather than in one word that happens to hold several: cargo
/// takes one package per flag, and a name is a name.
///
/// # Errors
///
/// Reports the build's own failure.
pub fn build_programs(userdata_name: &str) -> Result<(), crate::Error> {
    let mut arguments: Vec<String> = vec![
        "build".to_owned(),
        "--release".to_owned(),
        "--all-features".to_owned(),
    ];

    for program in PROGRAMS.into_iter().chain(PROGRAMS_WITHOUT_COMPLETIONS) {
        arguments.push("-p".to_owned());
        arguments.push(program.to_owned());
    }

    Tool::cargo().run_with_env(
        &[(USERDATA_NAME_ENV_VAR, OsStr::new(userdata_name))],
        arguments,
    )
}

/// Builds every program the tree ships.
#[chain]
pub fn handle_cli(_state: StateCli, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        build_programs(&crate::userdata_name_from_environment())
    });

    empty_result!()
}

/// Builds the programs and lays them, with the completion scripts, under `build/`.
#[command(node = "export-cli")]
pub fn export_cli(args: Vec<String>) -> StateExportCli {
    StateExportCli {
        userdata_name: crate::userdata_name_from_args(&args),
    }
}

/// The state the command starts in: the userdata name the export's programs are built with.
#[derive(Grouped)]
pub struct StateExportCli {
    /// The name the exported programs discover their user data by.
    userdata_name: String,
}

/// Builds the programs and lays them out under `build/bin` and `build/scripts`.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] when the name is not one directory segment, and whatever
/// [`build_programs`] or [`lay_out_programs`] reports after that.
pub fn export_programs(userdata_name: &str) -> Result<(), crate::Error> {
    let userdata_name = crate::checked_userdata_name(userdata_name)?;
    let workspace = Workspace::discover()?;

    // A program about to be written is stopped first: a running program holds its own executable
    // open, and on Windows a file in use can be neither replaced nor removed.
    stop_running_programs()?;

    build_programs(userdata_name)?;
    lay_out_programs(&workspace, Platform::current())
}

/// Lays the programs and the completion scripts into the export.
///
/// Every program is laid down beside the others, so that a word of `rola`'s reaches the program it
/// names, `rola jj` is `rola-jj`.
///
/// # Errors
///
/// Reports a directory the workspace cannot name or a copy.
pub fn lay_out_programs(workspace: &Workspace, platform: Platform) -> Result<(), crate::Error> {
    let programs = workspace.build_dir()?.join("bin");
    crate::fs::create_directory(&programs)?;

    let release = workspace.release_dir()?;
    for program in PROGRAMS.into_iter().chain(PROGRAMS_WITHOUT_COMPLETIONS) {
        let name = format!("{program}{}", platform.exe_suffix());
        crate::fs::copy_file(&release.join(&name), &programs.join(&name))?;
    }

    // The completions are laid down for the programs that have them: one directory per program, so
    // what a shell's setup sources is named once per program rather than once per program and shell.
    let completions = workspace.completion_dir()?;
    let scripts = workspace.scripts_dir()?;
    for program in PROGRAMS {
        for shell in COMPLETION_SHELLS {
            crate::fs::copy_file(
                &completions.join(format!("{program}_comp.{shell}")),
                &scripts
                    .join(program)
                    .join(format!("{program}-completion.{shell}")),
            )?;
        }
    }

    Ok(())
}

/// Builds the programs and lays them out under `build/`.
#[chain]
pub fn handle_export_cli(state: StateExportCli, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        export_programs(&state.userdata_name)?;
        crate::ops::export::mark_export_integrity(&Workspace::discover()?)
    });

    empty_result!()
}

/// Stops the programs a build is about to write, on Windows.
///
/// A running program holds its own executable open, and Windows refuses to replace it or to remove the
/// directory it is in, which is what an export does to the layout a previous export left. The three
/// are the programs of an export that are started by hand and left running; the Desktop program is
/// not among them, because it is opened through the launcher rather than from the layout.
///
/// Nothing running is the usual case, and it is not a failure: PowerShell answers 1 when
/// `Get-Process` matches nothing, so the run is ended with `exit 0` and a program it could not stop
/// is left for the removal below to name.
#[cfg(windows)]
pub(crate) fn stop_running_programs() -> Result<(), crate::Error> {
    const STOP: &str = "Get-Process -Name 'rola','rola-daemon','rola-daemon-gui' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; exit 0";

    Tool::powershell().run(["-NoProfile", "-NonInteractive", "-Command", STOP])
}

/// Leaves what is running alone: no platform but Windows holds a program's own file open against a
/// build, so there is nothing to stop.
///
/// The signature is the one the Windows twin has, so the call site reads the same on both platforms.
#[cfg(not(windows))]
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn stop_running_programs() -> Result<(), crate::Error> {
    Ok(())
}

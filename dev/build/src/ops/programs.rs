//! `cli`: the release command line programs.
//!
//! Compiling them is also what refreshes the completion scripts mingling writes beside cargo's
//! output, so a program added to [`crate::paths::PROGRAMS`] is one a shell can be completed for
//! without anything here changing.
//!
//! `export-cli` is the same build, laid out under `build/bin` with the two things that are not
//! cargo's own: the window over the daemon, and the completion scripts a shell sources.

use std::ffi::OsStr;
#[cfg(windows)]
use std::ffi::OsString;

use rorolala_utils_constants::USERDATA_NAME_ENV_VAR;
#[cfg(any(windows, target_os = "linux"))]
use rorolala_utils_constants::{DAEMON_DIR, DAEMON_REGISTRY_FILE, USER_DATA_DIR, USER_KEYS_DIR};

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

/// Builds the programs and lays them, with the daemon window and the completion scripts, under
/// `build/`.
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
    lay_out_programs(&workspace, Platform::current(), userdata_name)
}

/// Lays the programs, the daemon window and the completion scripts into the export.
///
/// Every program is laid down beside the others, so that a word of `rola`'s reaches the program it
/// names, `rola jj` is `rola-jj`.
///
/// # Errors
///
/// Reports a directory the workspace cannot name, a copy, or the daemon window's own build.
pub fn lay_out_programs(
    workspace: &Workspace,
    platform: Platform,
    userdata_name: &str,
) -> Result<(), crate::Error> {
    let programs = workspace.build_dir()?.join("bin");
    crate::fs::create_directory(&programs)?;

    let release = workspace.release_dir()?;
    for program in PROGRAMS.into_iter().chain(PROGRAMS_WITHOUT_COMPLETIONS) {
        let name = format!("{program}{}", platform.exe_suffix());
        crate::fs::copy_file(&release.join(&name), &programs.join(&name))?;
    }

    // The window over the daemon is laid down after them rather than with them: it is a C program
    // rather than a cargo one, and it looks for `rola-daemon` beside itself.
    build_daemon_window(workspace, userdata_name)?;

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

/// The keys directory a build's user data holds, relative to the base that data sits in.
#[cfg(any(windows, target_os = "linux"))]
fn userdata_keys_path(name: &str) -> String {
    format!("{USER_DATA_DIR}/{name}/{USER_KEYS_DIR}")
}

/// The daemon's registry file a build's user data holds, relative to the base.
#[cfg(any(windows, target_os = "linux"))]
fn userdata_registry_path(name: &str) -> String {
    format!("{USER_DATA_DIR}/{name}/{DAEMON_DIR}/{DAEMON_REGISTRY_FILE}")
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

/// Builds the Win32 window over the daemon into the export, on Windows.
///
/// It is the script beside the C sources rather than a cargo build, and it is told which export to
/// write into: `BUILD_DIR` is the same override this build honours, so an export laid out elsewhere
/// is still the one the window lands in.
#[cfg(windows)]
fn build_daemon_window(workspace: &Workspace, userdata_name: &str) -> Result<(), crate::Error> {
    let script = workspace.daemon_window_script().into_os_string();
    let build = workspace.build_dir()?.into_os_string();
    let keys = userdata_keys_path(userdata_name);
    let registry = userdata_registry_path(userdata_name);

    Tool::powershell().run_with_env(
        &[
            ("BUILD_DIR", build.as_os_str()),
            (USERDATA_NAME_ENV_VAR, OsStr::new(userdata_name)),
            ("ROLA_USER_KEYS_PATH", OsStr::new(&keys)),
            ("ROLA_DAEMON_REGISTRY_PATH", OsStr::new(&registry)),
        ],
        [
            OsString::from("-NoProfile"),
            OsString::from("-NonInteractive"),
            // The tree may have been checked out with a policy that refuses unsigned scripts; this
            // one is the tree's own, next to the sources it builds.
            OsString::from("-ExecutionPolicy"),
            OsString::from("Bypass"),
            OsString::from("-File"),
            script,
        ],
    )
}

/// Builds the GTK 4 window over the daemon into the export, on Linux.
///
/// It runs the script beside the C sources through the shell, so the script does not have to carry
/// an executable bit through every checkout, and it is told the same export in the same way.
///
/// A musl build is told the machine's own `cc` as well, and deliberately: the window links against
/// GTK, which every distribution builds against glibc, and `musl-gcc` cannot link it. What it is is
/// therefore the host's, like the shared library a musl package lays beside the Desktop program (see
/// `linux::add_shared_library_for_desktop`), while the programs beside it stay musl.
#[cfg(target_os = "linux")]
fn build_daemon_window(workspace: &Workspace, userdata_name: &str) -> Result<(), crate::Error> {
    let script = workspace.daemon_window_script();
    let build = workspace.build_dir()?;
    let keys = userdata_keys_path(userdata_name);
    let registry = userdata_registry_path(userdata_name);
    let mut environment: Vec<(&str, &OsStr)> = vec![
        ("BUILD_DIR", build.as_os_str()),
        (USERDATA_NAME_ENV_VAR, OsStr::new(userdata_name)),
        ("ROLA_USER_KEYS_PATH", OsStr::new(&keys)),
        ("ROLA_DAEMON_REGISTRY_PATH", OsStr::new(&registry)),
    ];

    if workspace
        .target_triple()
        .is_some_and(|triple| triple.contains("musl"))
    {
        environment.push(("CC", OsStr::new("cc")));
    }

    Tool::sh().run_with_env(&environment, [script])
}

/// Lays nothing down: a window is a Windows and a Linux program, and the other platforms have none
/// to build.
///
/// The signature is the twins' above, so all three call sites read the same.
#[cfg(not(any(windows, target_os = "linux")))]
#[allow(clippy::unnecessary_wraps)]
fn build_daemon_window(_workspace: &Workspace, _userdata_name: &str) -> Result<(), crate::Error> {
    Ok(())
}

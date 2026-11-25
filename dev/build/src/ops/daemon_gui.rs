//! `daemon-gui`: the window over the daemon, a C program beside the Rust ones.
//!
//! It belongs to the Desktop part, since it is the face the Desktop opens onto the daemon, but it is
//! split out because it is built by a script of its own rather than by cargo, and a change to it need
//! not rebuild anything else.
//!
//! The window has no build output of its own: the script writes it into the export layout, `build/bin`,
//! where it looks for `rola-daemon` beside itself. `daemon-gui` and `export-daemon-gui` therefore both
//! place it there, the second also giving the tree the integrity level that lets it run, and neither
//! builds any Rust.

#[cfg(any(windows, target_os = "linux"))]
use std::ffi::OsStr;
#[cfg(windows)]
use std::ffi::OsString;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
#[cfg(any(windows, target_os = "linux"))]
use rorolala_utils_constants::USERDATA_NAME_ENV_VAR;
#[cfg(any(windows, target_os = "linux"))]
use rorolala_utils_constants::{DAEMON_DIR, DAEMON_REGISTRY_FILE, USER_DATA_DIR, USER_KEYS_DIR};

use crate::Next;
use crate::paths::Workspace;
use crate::run_step;
#[cfg(any(windows, target_os = "linux"))]
use crate::toolchain::Tool;

/// Builds the window over the daemon.
#[command(node = "daemon-gui")]
pub fn daemon_gui() -> StateDaemonGui {
    StateDaemonGui
}

/// The state the command starts in: it names nothing, since the window is the whole of what is made.
#[derive(Grouped)]
pub struct StateDaemonGui;

/// Builds the window into the export layout, for the userdata name the environment names.
///
/// # Errors
///
/// Reports the script's own failure, a missing C toolchain, or a directory the workspace cannot name.
pub fn build_daemon_gui(userdata_name: &str) -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    // A running window holds its own executable open, and Windows refuses to replace one.
    crate::ops::programs::stop_running_programs()?;

    build_daemon_window(&workspace, userdata_name)
}

/// Builds the window.
#[chain]
pub fn handle_daemon_gui(_state: StateDaemonGui, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        build_daemon_gui(&crate::userdata_name_from_environment())
    });

    empty_result!()
}

/// Builds the window and lays it out under `build/bin`.
#[command(node = "export-daemon-gui")]
pub fn export_daemon_gui(args: Vec<String>) -> StateExportDaemonGui {
    StateExportDaemonGui {
        userdata_name: crate::userdata_name_from_args(&args),
    }
}

/// The state the command starts in: the userdata name the window embeds its paths for.
#[derive(Grouped)]
pub struct StateExportDaemonGui {
    /// The name the window's user data is found under.
    userdata_name: String,
}

/// Builds the window and lays it out under `build/bin`.
#[chain]
pub fn handle_export_daemon_gui(state: StateExportDaemonGui, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        build_daemon_gui(&state.userdata_name)?;
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

/// Builds the Win32 window over the daemon into the export, on Windows.
///
/// It is the script beside the C sources rather than a cargo build, and it is told which export to
/// write into: `BUILD_DIR` is the same override this build honours, so an export laid out elsewhere
/// is still the one the window lands in.
#[cfg(windows)]
pub(crate) fn build_daemon_window(
    workspace: &Workspace,
    userdata_name: &str,
) -> Result<(), crate::Error> {
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
pub(crate) fn build_daemon_window(
    workspace: &Workspace,
    userdata_name: &str,
) -> Result<(), crate::Error> {
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
pub(crate) fn build_daemon_window(
    _workspace: &Workspace,
    _userdata_name: &str,
) -> Result<(), crate::Error> {
    Ok(())
}

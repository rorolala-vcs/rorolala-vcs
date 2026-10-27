//! `export`: the build laid out for hand-off under `build/`, under the names a consumer links,
//! includes and sources.
//!
//! That is also what puts the Desktop program where `rola desktop` reaches for it, so an export is
//! what makes that command work. What is laid down is what the platform spells its artifacts:
//!
//! | cargo artifact               | exported as                       |
//! | ---------------------------- | --------------------------------- |
//! | `librorolala.so`/`.dylib`    | `build/lib/rorolala.so`/`.dylib`  |
//! | `rorolala.dll`               | `build/lib/rorolala.dll`          |
//! | `rorolala.dll.lib`           | `build/lib/rorolala.dll.lib`      |
//! | `rola[.exe]` …               | `build/bin/rola[.exe]` …          |
//! | `ffi_bindings/rorolala_ffi.h`| `build/lib/rorolala.h`/`.hpp`     |
//! | `mingling/<program>_comp.*`  | `build/scripts/<program>/*`       |
//!
//! Windows and Linux each have one artifact that is not cargo's at all: the window over the daemon,
//! built from C by the script beside it after the programs are laid down — Win32 on Windows, GTK 4 on
//! Linux. Each looks for `rola-daemon` beside itself, so what it is built beside has to be there
//! first.

#[cfg(windows)]
use std::ffi::OsString;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::dotnet;
use crate::fs;
use crate::paths::{COMPLETION_SHELLS, PROGRAMS, PROGRAMS_WITHOUT_COMPLETIONS, Workspace};
use crate::platform::Platform;
use crate::run_step;
use crate::toolchain::Tool;

/// The plugins' own pictures first, and deliberately: they are drawn from the pinned icon set rather
/// than carried, so what is laid out is what the pin holds rather than whatever was last in the tree.
#[command(node = "export")]
pub fn export() -> StateExport {
    StateExport
}

/// The state the command starts in: it names nothing, since the whole export is what is laid out.
#[derive(Grouped)]
pub struct StateExport;

/// Lays the build out for hand-off.
///
/// `self_contained` is what the Desktop program and the launcher are published as. A plain export
/// leaves the .NET runtime to the machine, which is what a developer runs; a package carries it, so
/// that installing the package is the whole of what a reader does.
pub fn export_artifacts(self_contained: bool) -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;
    let platform = Platform::current();

    // The programs about to be written are stopped first: a running program holds its own executable
    // open, and on Windows a file in use can be neither replaced nor removed with the directory it
    // sits in.
    stop_running_programs()?;

    Tool::cargo().run(["run", "--quiet", "-p", "rorolala-dev-icons"])?;
    crate::build_everything()?;

    let build = workspace.build_dir()?;
    fs::remove(&build)?;

    let library = build.join("lib");
    let programs = build.join("bin");
    fs::create_directory(&library)?;
    fs::create_directory(&programs)?;

    let release = workspace.release_dir()?;

    // A build that has no shared library — a musl one, see `library.rs` — hands over the static one
    // alone rather than a shared library that is not there. The header beside it is the same.
    let shared = release.join(platform.cargo_shared());
    if shared.is_file() {
        fs::copy_file(&shared, &library.join(platform.shared_exported()))?;
    }

    fs::copy_file(
        &release.join(platform.cargo_static()),
        &library.join(platform.static_exported()),
    )?;

    // Only Windows has an import library, and only a DLL is handed over beside one.
    if let (Some(cargo), Some(exported)) = (platform.cargo_import(), platform.import_exported()) {
        fs::copy_file(&release.join(cargo), &library.join(exported))?;
    }

    // The header is C and C++ at once — its declarations sit in an `extern "C"` block — so it is
    // handed over under both names.
    let header = workspace.header()?;
    fs::copy_file(&header, &library.join("rorolala.h"))?;
    fs::copy_file(&header, &library.join("rorolala.hpp"))?;

    // Every program is laid down beside the others, so that a word of `rola`'s reaches the program
    // it names — `rola jj` is `rola-jj`.
    for program in PROGRAMS.into_iter().chain(PROGRAMS_WITHOUT_COMPLETIONS) {
        let name = format!("{program}{}", platform.exe_suffix());
        fs::copy_file(&release.join(&name), &programs.join(&name))?;
    }

    // The window over the daemon is laid down after them rather than with them: it is a C program
    // rather than a cargo one, and it looks for `rola-daemon` beside itself.
    build_daemon_window(&workspace)?;

    // The completions are laid down for the programs that have them: one directory per program, so
    // what a shell's setup sources is named once per program rather than once per program and shell.
    let completions = workspace.completion_dir()?;
    let scripts = workspace.scripts_dir()?;
    for program in PROGRAMS {
        for shell in COMPLETION_SHELLS {
            fs::copy_file(
                &completions.join(format!("{program}_comp.{shell}")),
                &scripts
                    .join(program)
                    .join(format!("{program}-completion.{shell}")),
            )?;
        }
    }

    // The whole solution is restored and built once for everything below rather than a project at a
    // time: the restore is shared, and one build over the solution is what builds the host, the
    // plugins and the bindings together.
    dotnet::build_solution(&workspace)?;

    let desktop = workspace.desktop_dir()?;
    dotnet::publish_desktop(&workspace, &desktop, self_contained)?;
    dotnet::publish_launcher(&workspace, &desktop, self_contained)?;
    dotnet::publish_plugins(&workspace, &desktop)?;

    // The C# bindings beside the native library they bind, under `lib/` where a consumer of the C
    // ABI looks for both.
    dotnet::publish_rsharp(&workspace)?;

    // And the words that go with it: what the hand-off is, and what it is handed over under. They
    // are the tree's own at its top rather than files written for a layout, because a second copy is
    // a copy that goes stale.
    for file in ["README.md", "LICENSE"] {
        fs::copy_file(&workspace.root().join(file), &build.join(file))?;
    }

    Ok(())
}

/// Lays the export out.
#[chain]
pub fn handle_export(_state: StateExport, ec: &mut ResExitCode) -> Next {
    run_step(ec, || export_artifacts(false));

    empty_result!()
}

/// Stops the programs a build is about to write, on Windows.
///
/// A running program holds its own executable open, and Windows refuses to replace it or to remove the
/// directory it is in — which is what an export does to the layout a previous export left. The three
/// are the programs of an export that are started by hand and left running; the Desktop program is
/// not among them, because it is opened through the launcher rather than from the layout.
///
/// Nothing running is the usual case, and it is not a failure: PowerShell answers 1 when
/// `Get-Process` matches nothing, so the run is ended with `exit 0` and a program it could not stop
/// is left for the removal below to name.
#[cfg(windows)]
fn stop_running_programs() -> Result<(), crate::Error> {
    const STOP: &str = "Get-Process -Name 'rola','rola-daemon','rola-daemon-gui' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; exit 0";

    Tool::powershell().run(["-NoProfile", "-NonInteractive", "-Command", STOP])
}

/// Leaves what is running alone: no platform but Windows holds a program's own file open against a
/// build, so there is nothing to stop.
///
/// The signature is the one the Windows twin has, so the call site reads the same on both platforms.
#[cfg(not(windows))]
#[allow(clippy::unnecessary_wraps)]
fn stop_running_programs() -> Result<(), crate::Error> {
    Ok(())
}

/// Builds the Win32 window over the daemon into the export, on Windows.
///
/// It is the script beside the C sources rather than a cargo build, and it is told which export to
/// write into: `BUILD_DIR` is the same override this build honours, so an export laid out elsewhere
/// is still the one the window lands in.
#[cfg(windows)]
fn build_daemon_window(workspace: &Workspace) -> Result<(), crate::Error> {
    let script = workspace.daemon_window_script().into_os_string();
    let build = workspace.build_dir()?.into_os_string();

    Tool::powershell().run_with_env(
        &[("BUILD_DIR", build.as_os_str())],
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
#[cfg(target_os = "linux")]
fn build_daemon_window(workspace: &Workspace) -> Result<(), crate::Error> {
    let script = workspace.daemon_window_script();
    let build = workspace.build_dir()?;
    let environment = [("BUILD_DIR", build.as_os_str())];

    Tool::sh().run_with_env(&environment, [script])
}

/// Lays nothing down: a window is a Windows and a Linux program, and the other platforms have none
/// to build.
///
/// The signature is the twins' above, so all three call sites read the same.
#[cfg(not(any(windows, target_os = "linux")))]
#[allow(clippy::unnecessary_wraps)]
fn build_daemon_window(_workspace: &Workspace) -> Result<(), crate::Error> {
    Ok(())
}

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
//! | `rola[.exe]` and more        | `build/bin/rola[.exe]` and more   |
//! | `ffi_bindings/rorolala_ffi.h`| `build/lib/rorolala.h`/`.hpp`     |
//! | `mingling/<program>_comp.*`  | `build/scripts/<program>/*`       |
//!
//! Windows and Linux each have one artifact that is not cargo's at all: the window over the daemon,
//! built from C by the script beside it after the programs are laid down, Win32 on Windows, GTK 4 on
//! Linux. Each looks for `rola-daemon` beside itself, so what it is built beside has to be there
//! first.
//!
//! The C ABI and the C# bindings are also handed over as the two `NuGet` packages a .NET consumer
//! restores, `build/lib/RolaSharp.{version}.nupkg` and
//! `build/lib/RolaSharp.native.{runtime}.{version}.nupkg`, see [`crate::nuget`].

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
use rorolala_utils_constants::{DAEMON_DIR, DAEMON_REGISTRY_FILE, USER_DATA_DIR, USER_KEYS_DIR};
use rorolala_utils_constants::{DEVELOPMENT_USERDATA_NAME, USERDATA_NAME_ENV_VAR};

use crate::Next;
use crate::dotnet;
use crate::fs;
use crate::ops::book::build_book;
use crate::paths::{COMPLETION_SHELLS, PROGRAMS, PROGRAMS_WITHOUT_COMPLETIONS, Workspace};
use crate::platform::Platform;
use crate::run_step;
use crate::toolchain::Tool;

/// The plugins' own pictures first, and deliberately: they are drawn from the pinned icon set rather
/// than carried, so what is laid out is what the pin holds rather than whatever was last in the tree.
#[command(node = "export")]
pub fn export(args: Vec<String>) -> StateExport {
    StateExport {
        userdata_name: userdata_name_from_args(&args),
    }
}

/// The state the command starts in: the userdata name every artifact of this export looks under.
#[derive(Grouped)]
pub struct StateExport {
    /// The name the export's programs discover their user data by.
    userdata_name: String,
}

/// The userdata name a build is for, from `ROLA_USERDATA_NAME` or the development name.
///
/// `build`, `lib` and `bin` have no flag of their own: they are the steps `export` and the gate run,
/// and the name reaches them the same way it reaches the C and C# children, through the environment.
pub(crate) fn userdata_name_from_environment() -> String {
    std::env::var(USERDATA_NAME_ENV_VAR)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEVELOPMENT_USERDATA_NAME.to_owned())
}

/// The userdata name `args` names, or the development name when they name none.
///
/// Both spellings are taken, `--userdata-name=value` and `--userdata-name value`, because a build
/// command is typed by hand as often as it is run by a script.
pub(crate) fn userdata_name_from_args(args: &[String]) -> String {
    let mut name = DEVELOPMENT_USERDATA_NAME.to_owned();
    let mut words = args.iter();

    while let Some(word) = words.next() {
        if let Some(value) = word.strip_prefix("--userdata-name=") {
            name = value.to_owned();
        } else if word == "--userdata-name" {
            name = words.next().cloned().unwrap_or_default();
        }
    }

    name
}

/// Refuses a name that is not one directory segment.
///
/// The name becomes a directory under the user's data directory, so a separator or `..` would put
/// a build's files somewhere it never meant to write. The rule is the one the C# side and the
/// documentation state: ASCII letters, digits, `-` and `_`, not starting with `-`.
pub(crate) fn checked_userdata_name(name: &str) -> Result<&str, crate::Error> {
    let invalid = |reason| crate::Error::UserDataName {
        value: name.to_owned(),
        reason,
    };
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return Err(invalid("it is empty"));
    };

    if first == '-' {
        return Err(invalid("it starts with `-`"));
    }
    if !(first.is_ascii_alphanumeric() || first == '_') {
        return Err(invalid(
            "only ASCII letters, digits, `-` and `_` are allowed",
        ));
    }
    if !characters
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err(invalid(
            "only ASCII letters, digits, `-` and `_` are allowed",
        ));
    }

    Ok(name)
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

/// Lays the build out for hand-off.
///
/// `userdata_name` is the directory every artifact discovers its own files under, and
/// `self_contained` is what the Desktop program and the launcher are published as. A plain export
/// leaves the .NET runtime to the machine, which is what a developer runs; a package carries it, so
/// that installing the package is the whole of what a reader does.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] before anything is written when the name is not one
/// directory segment, and the first failing step after that.
pub fn export_artifacts(userdata_name: &str, self_contained: bool) -> Result<(), crate::Error> {
    let userdata_name = checked_userdata_name(userdata_name)?;
    let workspace = Workspace::discover()?;
    let platform = Platform::current();

    // The font files are fetched before anything is built, since the shared project embeds them and a
    // project that was evaluated without them would compile a program with no face of its own.
    crate::fetch_fonts(&workspace)?;

    // The programs about to be written are stopped first: a running program holds its own executable
    // open, and on Windows a file in use can be neither replaced nor removed with the directory it
    // sits in.
    stop_running_programs()?;

    Tool::cargo().run(["run", "--quiet", "-p", "rorolala-dev-icons"])?;
    crate::build_everything(userdata_name)?;

    let build = workspace.build_dir()?;
    fs::remove(&build)?;

    let library = workspace.library_dir()?;
    let programs = build.join("bin");
    fs::create_directory(&library)?;
    fs::create_directory(&programs)?;

    // The book is written into the directory the export was just cleared for, so a run that fails
    // before this point cannot leave a stale book in a layout that packages it.
    build_book(&workspace)?;

    let release = workspace.release_dir()?;

    // A build that has no shared library (a musl one, see `library.rs`) hands over the static one
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

    // The header is C and C++ at once, its declarations sit in an `extern "C"` block, so it is
    // handed over under both names.
    let header = workspace.header()?;
    fs::copy_file(&header, &library.join("rorolala.h"))?;
    fs::copy_file(&header, &library.join("rorolala.hpp"))?;

    // Every program is laid down beside the others, so that a word of `rola`'s reaches the program
    // it names, `rola jj` is `rola-jj`.
    for program in PROGRAMS.into_iter().chain(PROGRAMS_WITHOUT_COMPLETIONS) {
        let name = format!("{program}{}", platform.exe_suffix());
        fs::copy_file(&release.join(&name), &programs.join(&name))?;
    }

    // The window over the daemon is laid down after them rather than with them: it is a C program
    // rather than a cargo one, and it looks for `rola-daemon` beside itself.
    build_daemon_window(&workspace, userdata_name)?;

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
    dotnet::build_solution(&workspace, userdata_name)?;

    let desktop = workspace.desktop_dir()?;
    dotnet::publish_desktop(&workspace, &desktop, self_contained, userdata_name)?;
    dotnet::publish_host(&workspace, &desktop, self_contained, userdata_name)?;
    dotnet::publish_launcher(&workspace, &desktop, self_contained, userdata_name)?;
    dotnet::publish_plugins(&workspace, &desktop)?;
    crate::ops::desktop::lay_out_presets(&workspace)?;
    crate::lay_out_fonts(&workspace, &desktop)?;

    // The C# bindings beside the native library they bind, under `lib/` where a consumer of the C
    // ABI looks for both.
    dotnet::publish_rsharp(&workspace)?;

    // And the same two things as packages, since a .NET consumer restores one instead of being told
    // which file beside which program to reference. A platform without a native package is said out
    // loud and does not fail the export, since a build for it still has the managed package.
    crate::nuget::pack_rsharp(&workspace)?;

    // And the words that go with it: what the hand-off is, and what it is handed over under. They
    // are the tree's own at its top rather than files written for a layout, because a second copy is
    // a copy that goes stale.
    for file in ["README.md", "LICENSE"] {
        fs::copy_file(&workspace.root().join(file), &build.join(file))?;
    }

    // Last, once everything is laid down: the label covers the whole tree rather than the files an
    // earlier export left.
    mark_export_integrity(&workspace)?;

    Ok(())
}

/// Lays the export out.
#[chain]
pub fn handle_export(state: StateExport, ec: &mut ResExitCode) -> Next {
    run_step(ec, || export_artifacts(&state.userdata_name, false));

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

/// Gives the export tree the integrity level that lets it run.
///
/// The export tree is handed to whoever runs it, so it is given the integrity level that lets it
/// run rather than being left at whatever the tree was checked out with: a program marked low
/// starts at low in every session it is started from, and a low process may not write to the
/// medium-level places a run works in, a Workspace under `%LOCALAPPDATA%`, a project's cache.
///
/// The label is set on the tree rather than on each program so that a file added to it later is
/// covered by inheritance, and `/C` keeps one unreadable file from leaving the rest unmarked.
#[cfg(windows)]
fn mark_export_integrity(workspace: &Workspace) -> Result<(), crate::Error> {
    let build = workspace.build_dir()?.into_os_string();
    let mut arguments = vec![build];

    for word in ["/setintegritylevel", "(OI)(CI)M", "/T", "/C", "/Q"] {
        arguments.push(OsString::from(word));
    }

    Tool::at("icacls").run(arguments)
}

/// Leaves the export as it is: an integrity level is Windows's, and no other platform asks a
/// program what it may write to before it writes.
///
/// The signature is the one the Windows twin has, so the call site reads the same on both
/// platforms.
#[cfg(not(windows))]
#[allow(clippy::unnecessary_wraps)]
fn mark_export_integrity(_workspace: &Workspace) -> Result<(), crate::Error> {
    Ok(())
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

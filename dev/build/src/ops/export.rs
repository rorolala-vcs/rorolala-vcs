//! `export`: the whole build laid out for hand-off under `build/`, under the names a consumer
//! links, includes and sources.
//!
//! It is the `export-*` steps in order, the C ABI first, the programs after it, then the Desktop
//! program, its plugins and the window over the daemon, plus the two files that say what the hand-off
//! is. A step run on its own, `export-lib`, `export-cli`, `export-book`, `export-desktop`,
//! `export-daemon-gui` or `export-desktop-plugin`, builds and lays out only its own part and makes no
//! promise that the rest of the layout is there: what it is for is a short loop for a change to that
//! one part.
//!
//! What is laid down is what the platform spells its artifacts:
//!
//! | cargo artifact               | exported as                       |
//! | ---------------------------- | --------------------------------- |
//! | `librorolala.so`/`.dylib`    | `build/lib/rorolala.so`/`.dylib`  |
//! | `rorolala.dll`               | `build/lib/rorolala.dll`          |
//! | `rorolala.dll.lib`           | `build/lib/rorolala.dll.lib`      |
//! | `rola[.exe]` and more        | `build/bin/rola[.exe]` and more   |
//! | `ffi_bindings/rorolala_ffi.h`| `build/lib/rorolala.h`             |
//! | `mingling/<program>_comp.*`  | `build/scripts/<program>/*`       |
//!
//! The Desktop program is what `rola desktop` reaches for, so an export is what makes that command
//! work. The C ABI and the C# bindings are also handed over as the two `NuGet` packages a .NET
//! consumer restores, see [`crate::nuget`].

#[cfg(windows)]
use std::ffi::OsString;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rorolala_utils_constants::{DEVELOPMENT_USERDATA_NAME, USERDATA_NAME_ENV_VAR};

use crate::Next;
use crate::fs;
use crate::paths::Workspace;
use crate::run_step;
use crate::toolchain::Tool;

/// Lays the build out for hand-off.
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
/// `build`, `lib`, `cli`, `book` and `desktop` have no flag of their own: they are the steps the
/// `export-*` commands and the gate run, and the name reaches them the same way it reaches the C and
/// C# children, through the environment.
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

/// Lays the whole build out for hand-off.
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

    // The programs about to be written are stopped first: a running program holds its own executable
    // open, and on Windows a file in use can be neither replaced nor removed with the directory it
    // sits in.
    crate::ops::programs::stop_running_programs()?;

    // The Desktop's own pictures are drawn here, at the full export, rather than on every per-part
    // export: they are idempotent, and a change to the icon manifest is what asks for this step. A
    // per-part `export-desktop` uses the pictures the tree already carries.
    Tool::cargo().run(["run", "--quiet", "-p", "rorolala-dev-icons"])?;

    let build = workspace.build_dir()?;
    fs::remove(&build)?;
    fs::create_directory(&build)?;

    // One part at a time, each building and laying out what it owns. This is the one place the whole
    // set is assembled; a per-part `export-*` lays out only its own part.
    crate::ops::library::export_library(userdata_name)?;
    crate::ops::programs::export_programs(userdata_name)?;
    crate::ops::book::build_book(&workspace)?;
    crate::ops::desktop::export_desktop(userdata_name, self_contained)?;
    crate::ops::desktop_plugin::export_desktop_plugins(userdata_name)?;
    crate::ops::daemon_gui::build_daemon_gui(userdata_name)?;

    // And the words that go with it: what the hand-off is, and what it is handed over under. They
    // are the tree's own at its top rather than files written for a layout, because a second copy is
    // a copy that goes stale.
    for file in ["README.md", "LICENSE"] {
        fs::copy_file(&workspace.root().join(file), &build.join(file))?;
    }

    // Last, once everything is laid down: the label covers the whole tree rather than the files an
    // earlier export left.
    mark_export_integrity(&workspace)
}

/// Lays the export out.
#[chain]
pub fn handle_export(state: StateExport, ec: &mut ResExitCode) -> Next {
    run_step(ec, || export_artifacts(&state.userdata_name, false));

    empty_result!()
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
pub(crate) fn mark_export_integrity(workspace: &Workspace) -> Result<(), crate::Error> {
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
pub(crate) fn mark_export_integrity(_workspace: &Workspace) -> Result<(), crate::Error> {
    Ok(())
}

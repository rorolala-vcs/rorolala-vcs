//! The .NET side of the build: the solution, the two programs a layout holds, and the suites.
//!
//! A restore is made first and on its own, and what follows is told not to restore. The order is
//! load-bearing rather than incidental: a restore that reaches a feed which cannot be reached waits
//! rather than failing, so a machine that is offline with everything cached would hang in the middle
//! of the gate — and a build or a test that restores on its own asks that feed once per project it
//! touches.

use std::path::{Path, PathBuf};

use crate::{
    Error,
    fs::{copy_directory, copy_file, read_directories, read_files},
    paths::{DesktopPlugin, Workspace},
    platform::Platform,
    toolchain::Tool,
};

/// The configuration a project is built with: the only one anything here builds.
const CONFIGURATION: &str = "Release";

/// The directory a plugin's own output has under the .NET output root.
const PLUGIN_OUTPUT: &str = "bin/Release/net8.0";

/// The translations a program reads, beside it.
const TRANSLATIONS: &str = "i18n";

/// The plugins inside an export's Desktop directory.
const PLUGINS_IN_DESKTOP: &str = "plugins";

/// The icon sets inside an export's Desktop directory.
const ICON_SETS_IN_DESKTOP: &str = "icon-sets";

/// The banners inside an export's Desktop directory.
const BANNERS_IN_DESKTOP: &str = "launcher-banners";

/// The banner files the launcher may play.
const BANNER_EXTENSION: &str = "aseprite";

/// The icon set that is documentation rather than a set, and so does not travel with the program.
const EXAMPLE_ICON_SET: &str = "example";

/// The C# bindings' own file inside the .NET output: it is a library with nothing beside it.
const RSHARP_DLL: &str = "RolaSharp/bin/Release/net8.0/RolaSharp.dll";

/// Restores what `args` names, answering from what is already cached before the feed is asked.
///
/// The sources are taken away first, which resolves from the global packages folder without a word;
/// only a restore that cannot be satisfied that way goes to the feed, which is the one time waiting
/// for it is worth it.
///
/// # Errors
///
/// Reports the fallback restore's own failure when neither attempt succeeds.
pub fn restore(workspace: &Workspace, args: &[&str]) -> Result<(), Error> {
    let mut offline: Vec<&str> = vec!["restore"];
    offline.extend_from_slice(args);
    offline.push("--configfile");
    let configuration = workspace.nuget_offline().to_string_lossy().into_owned();
    offline.push(&configuration);

    if Tool::dotnet()
        .capture(offline)
        .is_ok_and(|output| output.status.success())
    {
        return Ok(());
    }

    let mut online: Vec<&str> = vec!["restore"];
    online.extend_from_slice(args);

    Tool::dotnet().run(online)
}

/// Restores and builds the whole solution, once, for everything the .NET side of an export is made
/// of.
///
/// A restore is per solution rather than per project, and a project restored on its own is the same
/// packages resolved over again — which one export pays once per project, four times over, for
/// nothing. What the projects are is the solution's own list, so nothing here names them.
///
/// # Errors
///
/// Reports the restore or the build's own failure.
pub fn build_solution(workspace: &Workspace) -> Result<(), Error> {
    let solution = workspace.solution().to_string_lossy().into_owned();
    restore(workspace, &[&solution])?;

    Tool::dotnet().run(["build", &solution, "-c", CONFIGURATION, "--no-restore"])
}

/// Builds and tests the solution: the Desktop program, the translations it reads, and the tests of
/// those translations.
///
/// The solution is built before it is tested, and deliberately: `test` builds only what a test
/// project depends on, and nothing depends on the Desktop program, so without the build a Desktop
/// program that does not compile would be found by neither.
///
/// # Errors
///
/// Reports the restore, the build or the tests' own failure.
pub fn check_solution(workspace: &Workspace) -> Result<(), Error> {
    let solution = workspace.solution().to_string_lossy().into_owned();
    restore(workspace, &[&solution])?;

    Tool::dotnet().run(["build", &solution, "-c", CONFIGURATION, "--no-restore"])?;

    Tool::dotnet().run(["test", &solution, "-c", CONFIGURATION, "--no-restore"])
}

/// Lints the .NET side: every rule the analyzer set carries, over the whole solution, with any
/// diagnostic failing the run.
///
/// Two things here are load-bearing:
///
/// * `TreatWarningsAsErrors` is turned *off* and `MSBuild`'s own `-warnAsError` turned *on*. The
///   compiler's switch stops the solution at the first project that fails, so one rule broken in
///   three programs would take three runs to find; `MSBuild`'s promotes the same warnings to errors
///   and carries on, so every one is reported in one run and the run still fails.
/// * `-t:Rebuild` rather than a plain build. The analyzers are the point of this gate, and a
///   project `MSBuild` calls up to date is one nothing analyzes — which a change to `.editorconfig`
///   or `Directory.Build.props` does not by itself undo.
///
/// # Errors
///
/// Reports the restore's or the rebuild's own failure.
pub fn lint_solution(workspace: &Workspace) -> Result<(), Error> {
    let solution = workspace.solution().to_string_lossy().into_owned();
    restore(workspace, &[&solution])?;

    Tool::dotnet().run([
        "msbuild",
        &solution,
        "-t:Rebuild",
        "-warnAsError",
        "-nologo",
        "-v:m",
        "-tl:off",
        "-p:Configuration=Release",
        "-p:AnalysisMode=All",
        "-p:AnalysisLevel=latest-all",
        "-p:EnforceCodeStyleInBuild=true",
        "-p:TreatWarningsAsErrors=false",
    ])
}

/// The suites under `tests/` that are run by the .NET test host, in directory order.
///
/// A suite is a project of its own whose name ends in `.IntegrationTests`, so adding one is adding a
/// directory.
///
/// # Errors
///
/// Reports [`Error::Io`] when the suites' directory cannot be read.
pub fn integration_suites(workspace: &Workspace) -> Result<Vec<PathBuf>, Error> {
    let directory = workspace.root().join("tests");
    let entries = std::fs::read_dir(&directory).map_err(|error| Error::Io {
        doing: "reading the .NET integration suites",
        detail: format!("{}: {error}", directory.display()),
    })?;

    let mut suites = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Error::Io {
            doing: "reading the .NET integration suites",
            detail: error.to_string(),
        })?;

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // A suite is the project a directory holds whose name ends in `.IntegrationTests`; the file
        // is named after the program it tests rather than after the directory it sits in, so the
        // directory is walked rather than the name matched.
        for project in read_files(&path)? {
            if project
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| name.ends_with(".IntegrationTests.csproj"))
            {
                suites.push(project);
            }
        }
    }

    suites.sort();

    Ok(suites)
}

/// Runs the .NET integration suites under `tests/`, one at a time.
///
/// A suite is run on its own rather than through a solution, and deliberately: a project a solution
/// does not list is built with its own default configuration, so a solution holding only the suites
/// would build the Desktop program they reference in Debug whatever `-c` said. Run this way, the
/// release reaches everything a suite references.
///
/// They run one at a time so that a suite that fails is named rather than lost among the output of
/// the others, and the ones after it still run, so one run reports every suite that failed.
///
/// # Errors
///
/// Reports [`Error::Status`] naming the count when any suite failed.
pub fn run_integration_suites(workspace: &Workspace) -> Result<(), Error> {
    let suites = integration_suites(workspace)?;
    let mut failed = Vec::new();

    for suite in &suites {
        println!("==> {}", suite.display());
        let path = suite.to_string_lossy().into_owned();

        let restore = restore(workspace, &[&path]);
        let test = restore.and_then(|()| {
            Tool::dotnet().run(["test", &path, "-c", CONFIGURATION, "--no-restore"])
        });

        if test.is_err() {
            failed.push(path);
        }
    }

    if failed.is_empty() {
        return Ok(());
    }

    println!("==> failed: {}", failed.join(" "));

    Err(Error::Status {
        program: "the .NET integration suites".to_owned(),
        code: 1,
    })
}

/// Publishes the Desktop program into the directory named.
///
/// `dotnet publish` is the form that produces a program with everything beside it that running it
/// needs, which is what an export holds; a plain build leaves a program whose dependencies are only
/// in the build tree.
///
/// # Errors
///
/// Reports the restore, the publish, or a copy's own failure.
pub fn publish_desktop(workspace: &Workspace, into: &Path) -> Result<(), Error> {
    let rid = runtime_identifier()?;
    let project = workspace.desktop_project().to_string_lossy().into_owned();
    let destination = into.to_string_lossy().into_owned();

    restore(workspace, &[&project, "-r", &rid])?;
    Tool::dotnet().run([
        "publish",
        &project,
        "-c",
        CONFIGURATION,
        "--no-restore",
        "-r",
        &rid,
        "--self-contained",
        "false",
        "-o",
        &destination,
    ])?;

    // The native library the Desktop's own bindings reach for, beside the program that binds it: a
    // plugin asks the C ABI through the C# bindings, which `DllImport` `rorolala`, and the runtime
    // looks for that in the program's own directory before anywhere else. Only the shared library
    // is taken — the import library is for a C link, and the header for a C compile.
    let library = workspace
        .release_dir()?
        .join(Platform::current().cargo_shared());
    copy_file(&library, &into.join(Platform::current().cargo_shared()))?;

    // The icon sets, laid beside the program rather than built into it: a tree with no sets is a
    // program that offers only the system's own icons, which is why this is a copy and not a
    // requirement.
    let sets = workspace.desktop_icon_sets();
    if sets.is_dir() {
        let destination = into.join(ICON_SETS_IN_DESKTOP);
        std::fs::create_dir_all(&destination).map_err(|error| Error::Io {
            doing: "creating the icon sets' directory",
            detail: format!("{}: {error}", destination.display()),
        })?;

        for set in read_directories(&sets)? {
            let Some(name) = set.file_name().and_then(std::ffi::OsStr::to_str) else {
                continue;
            };

            // `example` is documentation rather than a set, so it stays behind.
            if name == EXAMPLE_ICON_SET {
                continue;
            }

            copy_directory(&set, &destination.join(name))?;
        }
    }

    Ok(())
}

/// Publishes the launcher into the Desktop program's directory, and lays the banners beside it.
///
/// The launcher is published after the Desktop program rather than before: the two share a
/// directory, and what a publish lays down is what that program is made of. The launcher comes
/// second because it is the one that reads the other — the command line and the window are found by
/// where they sit beside it.
///
/// # Errors
///
/// Reports the restore, the publish, or a copy's own failure.
pub fn publish_launcher(workspace: &Workspace, into: &Path) -> Result<(), Error> {
    let rid = runtime_identifier()?;
    let project = workspace.launcher_project().to_string_lossy().into_owned();
    let destination = into.to_string_lossy().into_owned();

    restore(workspace, &[&project, "-r", &rid])?;
    Tool::dotnet().run([
        "publish",
        &project,
        "-c",
        CONFIGURATION,
        "--no-restore",
        "-r",
        &rid,
        "--self-contained",
        "false",
        "-o",
        &destination,
    ])?;

    let banners = workspace.launcher_banners();
    if banners.is_dir() {
        let destination = into.join(BANNERS_IN_DESKTOP);
        std::fs::create_dir_all(&destination).map_err(|error| Error::Io {
            doing: "creating the banners' directory",
            detail: format!("{}: {error}", destination.display()),
        })?;

        for banner in read_files(&banners)? {
            if banner.extension().and_then(std::ffi::OsStr::to_str) == Some(BANNER_EXTENSION)
                && let Some(name) = banner.file_name()
            {
                copy_file(&banner, &destination.join(name))?;
            }
        }
    }

    Ok(())
}

/// Lays each plugin that ships with the Desktop program where the program looks for it: its
/// assembly, its translations and any private dependency under `plugins/` beside the program.
///
/// The contract and Avalonia a plugin was built against are the host's own copies, delegated to
/// rather than carried, so laying them down would be laying down a second of each — but a
/// dependency the host does not carry is the plugin's own and travels with it.
///
/// # Errors
///
/// Reports a read or a copy's own failure, and [`Error::Manifest`] when the .NET output of a plugin
/// cannot be named.
pub fn publish_plugins(workspace: &Workspace, into: &Path) -> Result<(), Error> {
    for plugin in workspace.desktop_plugins()? {
        let output = plugin_output(workspace, &plugin)?;
        let name = plugin.name();

        let plugins = into.join(PLUGINS_IN_DESKTOP);
        std::fs::create_dir_all(&plugins).map_err(|error| Error::Io {
            doing: "creating the plugins' directory",
            detail: format!("{}: {error}", plugins.display()),
        })?;

        copy_file(
            &output.join(format!("{name}.dll")),
            &plugins.join(format!("{name}.dll")),
        )?;

        let translations = output.join(TRANSLATIONS);
        if translations.is_dir() {
            copy_directory(&translations, &plugins.join(TRANSLATIONS))?;
        }

        // A private dependency a plugin carries, laid beside it where its own load context probes.
        // What the host already carries is read from the host's own directory rather than from a
        // list: a second of any of them would be a second type of the same name.
        for file in read_files(&output)? {
            if !file
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
            {
                continue;
            }

            let Some(base) = file.file_name() else {
                continue;
            };

            if base == std::ffi::OsStr::new(&format!("{name}.dll")) || into.join(base).is_file() {
                continue;
            }

            copy_file(&file, &plugins.join(base))?;
        }
    }

    Ok(())
}

/// Lays the C# bindings down beside the native library they bind.
///
/// Nothing is built here: the bindings are a project of the solution, and built rather than
/// published even there — it is a library with nothing beside it, since the library it binds is the
/// export's own `lib/`.
///
/// # Errors
///
/// Reports the copy's own failure.
pub fn publish_rsharp(workspace: &Workspace) -> Result<(), Error> {
    let bindings = workspace.cs_target_dir()?.join(RSHARP_DLL);
    let into = workspace.build_dir()?.join("lib/RolaSharp.dll");

    copy_file(&bindings, &into)
}

/// The platform the Desktop program is laid out for, as .NET spells one.
///
/// A publish that names no platform carries the native libraries of every platform `NuGet` holds
/// them for — sixteen of them and half a gigabyte, of which one is ever loaded — so the layout names
/// the one it is for. It is the machine's own, asked of the SDK rather than worked out here, because
/// the SDK is what knows the machine.
///
/// # Errors
///
/// Reports [`Error::Tool`] when the SDK cannot be asked, and [`Error::Missing`] when its answer names
/// no RID.
pub fn runtime_identifier() -> Result<String, Error> {
    let output = Tool::dotnet().capture(["--info"])?;
    let info = String::from_utf8_lossy(&output.stdout);

    rid_of(&info).ok_or_else(|| Error::Missing {
        summary: "the host's RID could not be read, so the platform this layout is for is unknown"
            .to_owned(),
        advice: vec![
            "name it in DOTNET_RID, the way `dotnet --info` states it (for example win-x64)"
                .to_owned(),
        ],
    })
}

/// The RID `dotnet --info` states, when it states one.
fn rid_of(info: &str) -> Option<String> {
    info.lines()
        .find_map(|line| line.trim().strip_prefix("RID:"))
        .map(|rid| rid.trim().to_owned())
        .filter(|rid| !rid.is_empty())
}

/// A plugin's own output directory, under the .NET output root.
fn plugin_output(workspace: &Workspace, plugin: &DesktopPlugin) -> Result<PathBuf, Error> {
    Ok(workspace
        .cs_target_dir()?
        .join(plugin.name())
        .join(PLUGIN_OUTPUT))
}

#[cfg(test)]
mod tests {
    use super::rid_of;

    #[test]
    fn the_runtime_identifier_is_read_from_the_sdks_own_answer() {
        let info = "\nHost:\n  Version: 10.0.401\n  Architecture: x64\n  RID:         win-x64\n";
        assert_eq!(rid_of(info).as_deref(), Some("win-x64"));

        // An answer with no RID is an answer with no RID rather than an empty one.
        assert_eq!(rid_of("\nHost:\n  Version: 10.0.401\n"), None);
        assert_eq!(rid_of("  RID:   \n"), None);
    }
}

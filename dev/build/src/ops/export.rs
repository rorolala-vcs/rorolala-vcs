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
pub fn export_artifacts() -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;
    let platform = Platform::current();

    Tool::cargo().run(["run", "--quiet", "-p", "rorolala-dev-icons"])?;
    crate::build_everything()?;

    let build = workspace.build_dir()?;
    fs::remove(&build)?;

    let library = build.join("lib");
    let programs = build.join("bin");
    fs::create_directory(&library)?;
    fs::create_directory(&programs)?;

    let release = workspace.release_dir()?;
    fs::copy_file(
        &release.join(platform.cargo_shared()),
        &library.join(platform.shared_exported()),
    )?;
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
    dotnet::publish_desktop(&workspace, &desktop)?;
    dotnet::publish_launcher(&workspace, &desktop)?;
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
    run_step(ec, export_artifacts);

    empty_result!()
}

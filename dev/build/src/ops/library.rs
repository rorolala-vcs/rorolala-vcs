//! `lib`: the C ABI artifact, a release shared and static library, and, as a side effect of the
//! root build script, the C header that describes it.
//!
//! `export-lib` is the same build, laid out for hand-off under `build/lib` beside the C++ include
//! tree and the `CMake` package, so a C or C++ consumer builds against it. The C# bindings and the two
//! `NuGet` packages are laid there as well, since they are the same hand-off for a .NET consumer.

use std::ffi::OsStr;

use rorolala_utils_constants::USERDATA_NAME_ENV_VAR;

use crate::fs;
use crate::paths::Workspace;
use crate::platform::Platform;
use crate::toolchain::Tool;
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::run_step;

/// Builds the library and the header that describes it.
#[command(node = "lib")]
pub fn library() -> StateLibrary {
    StateLibrary
}

/// The state the command starts in: it names nothing, since the library is what is built.
#[derive(Grouped)]
pub struct StateLibrary;

/// Builds the release library the C ABI is handed over as, for `userdata_name`.
///
/// `--all-features` is what reaches the ABI the crate only carries behind a feature, and the header
/// is written by the root build script while the crate compiles, so building the artifact is also
/// what refreshes the header. The name is compiled into the library, which is what the C# bindings
/// and the Desktop program it is laid beside read back.
///
/// # Errors
///
/// Reports the build's own failure, and [`crate::Error::Directory`] when the workspace cannot name
/// where it builds.
pub fn build_library(userdata_name: &str) -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    ensure_header(&workspace)?;

    // A musl build has no shared library, and the reason is in rustc rather than here: a `cdylib` is
    // refused for a target whose C runtime is static, and the dynamic alternative needs a musl
    // `libgcc_s`, the unwinder is the one piece of the C toolchain `musl-tools` does not carry, so a
    // glibc machine cannot link one. What that target can hand a C consumer is the static library;
    // the header beside it is the same one every platform is given.
    if is_musl(&workspace) {
        return Tool::cargo().run_with_env(
            &[(USERDATA_NAME_ENV_VAR, OsStr::new(userdata_name))],
            [
                "rustc",
                "--release",
                "--all-features",
                "-p",
                "rorolala-ffi",
                "--crate-type",
                "staticlib",
            ],
        );
    }

    Tool::cargo().run_with_env(
        &[(USERDATA_NAME_ENV_VAR, OsStr::new(userdata_name))],
        ["build", "--release", "--all-features", "-p", "rorolala-ffi"],
    )
}

/// Writes the C header when a cached build left no copy of it.
///
/// The header is a side effect of the root build script, not an artifact cargo knows: a warm target
/// directory can keep the script fresh while `ffi_bindings/` is gone, which is a package that copies
/// a file nobody wrote. The bindgen crate the build script uses is called here instead, so the hand-off
/// has its header whether or not the script ran.
fn ensure_header(workspace: &Workspace) -> Result<(), crate::Error> {
    let directory = workspace.release_dir()?.join("ffi_bindings");

    if directory.join("rorolala_ffi.h").is_file() {
        return Ok(());
    }

    let roots: Vec<_> = ["src", "modules", "utils"]
        .into_iter()
        .map(|root| workspace.root().join(root))
        .collect();

    let config = rorolala_dev_bindgen::Config {
        source_roots: &roots,
        output_dir: &directory,
        enforce_naming: true,
    };

    rorolala_dev_bindgen::generate(&config)
        .map(|_| ())
        .map_err(|error| crate::Error::Io {
            doing: "writing the C ABI header",
            detail: error.to_string(),
        })
}

/// Builds the shared library the .NET suites load, and lays it beside the programs they are told are
/// there.
///
/// The suites are .NET programs, so the runtime they load is the machine's own, which is the glibc
/// one even when the programs under test were built against musl. A musl build has no shared library
/// of its own (see [`build_library`]), so the host triple is built here and its library is copied
/// where `ROLA_BIN_DIR` sends a suite: beside the programs. Every other target builds its library in
/// place, which is already that directory.
///
/// # Errors
///
/// Reports the build's own failure or the copy's, and [`crate::Error::Directory`] when the workspace
/// cannot name where it builds.
pub fn build_suite_library(workspace: &Workspace, userdata_name: &str) -> Result<(), crate::Error> {
    if !is_musl(workspace) {
        return build_library(userdata_name);
    }

    let host = format!("{}-unknown-linux-gnu", std::env::consts::ARCH);

    // `CC` is stated for this run because the run around it names `musl-gcc`: a dependency that
    // compiles C would otherwise be compiled by a musl driver into a library the glibc target it is
    // being built for cannot link.
    Tool::cargo().run_with_env(
        &[
            ("CC", OsStr::new("cc")),
            (USERDATA_NAME_ENV_VAR, OsStr::new(userdata_name)),
        ],
        [
            "build",
            "--release",
            "--all-features",
            "-p",
            "rorolala-ffi",
            "--target",
            host.as_str(),
        ],
    )?;

    let library = workspace
        .target_dir()?
        .join(&host)
        .join("release")
        .join("librorolala.so");

    crate::fs::copy_file(&library, &workspace.release_dir()?.join("librorolala.so"))
}

/// Whether this build is for a musl target, which makes no shared library.
fn is_musl(workspace: &Workspace) -> bool {
    workspace
        .target_triple()
        .is_some_and(|triple| triple.contains("musl"))
}

/// Builds the library and the header.
#[chain]
pub fn handle_library(_state: StateLibrary, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        build_library(&crate::userdata_name_from_environment())
    });

    empty_result!()
}

/// Builds the C ABI and lays the hand-off out under `build/lib`.
#[command(node = "export-lib")]
pub fn export_lib(args: Vec<String>) -> StateExportLibrary {
    StateExportLibrary {
        userdata_name: crate::userdata_name_from_args(&args),
    }
}

/// The state the command starts in: the userdata name compiled into the library it lays out.
#[derive(Grouped)]
pub struct StateExportLibrary {
    /// The name the library is built with, and the one the export is laid out for.
    userdata_name: String,
}

/// Builds the library and lays the C ABI hand-off out under `build/lib`.
///
/// The hand-off is more than the native library: the C# bindings and the two `NuGet` packages are
/// part of it, since a .NET consumer restores one instead of being told which file sits beside which
/// program. They are built and packed here, after the header the bindings are generated from.
///
/// # Errors
///
/// Reports [`crate::Error::UserDataName`] when the name is not one directory segment, and whatever
/// [`build_library`], [`lay_out_library`], the bindings' build or a package's failure reports after
/// that.
pub fn export_library(userdata_name: &str) -> Result<(), crate::Error> {
    let userdata_name = crate::checked_userdata_name(userdata_name)?;
    let workspace = Workspace::discover()?;

    build_library(userdata_name)?;
    lay_out_library(&workspace, Platform::current())?;

    crate::dotnet::build_rsharp(&workspace, userdata_name)?;
    crate::dotnet::publish_rsharp(&workspace)?;
    crate::nuget::pack_rsharp(&workspace)
}

/// Lays the C ABI hand-off into the export: the libraries, the header that describes them, the C++
/// include tree and the `CMake` package a consumer builds against.
///
/// Only what the build could produce is laid down: a musl build has no shared library, so it hands
/// over the static one alone (see [`build_library`]).
///
/// # Errors
///
/// Reports a directory the workspace cannot name, a copy, or the C++ layer's own generation.
pub fn lay_out_library(workspace: &Workspace, platform: Platform) -> Result<(), crate::Error> {
    let library = workspace.library_dir()?;
    fs::create_directory(&library)?;

    let release = workspace.release_dir()?;

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

    // The header is C, its declarations sit in an `extern "C"` block so a C++ caller can include it
    // as it is.
    let header = workspace.header()?;
    fs::copy_file(&header, &library.join("rorolala.h"))?;

    // The C++ layer: the hand-written headers, the generated layer beside them, and the C header
    // under the include root the generated files include it from. The generator runs here rather
    // than in a consumer's build, and its own compile step is what makes a generated header that
    // does not compile fail this export.
    crate::cpp::lay_out_includes(
        workspace,
        &workspace.build_dir()?.join("include"),
        &library.join("rorolala.h"),
    )?;

    // The package a CMake consumer finds, naming the same headers and the library laid out above.
    crate::cpp::write_package(workspace, &library)
}

/// Builds the library and lays it out under `build/lib`.
#[chain]
pub fn handle_export_library(state: StateExportLibrary, ec: &mut ResExitCode) -> Next {
    run_step(ec, || export_library(&state.userdata_name));

    empty_result!()
}

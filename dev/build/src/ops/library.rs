//! `lib`: the C ABI artifact, a release shared and static library, and, as a side effect of the
//! root build script, the C header that describes it.

use std::ffi::OsStr;

use crate::paths::Workspace;
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

/// Builds the release library the C ABI is handed over as.
///
/// `--all-features` is what reaches the ABI the crate only carries behind a feature, and the header
/// is written by the root build script while the crate compiles, so building the artifact is also
/// what refreshes the header.
///
/// # Errors
///
/// Reports the build's own failure, and [`crate::Error::Directory`] when the workspace cannot name
/// where it builds.
pub fn build_library() -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;

    // A musl build has no shared library, and the reason is in rustc rather than here: a `cdylib` is
    // refused for a target whose C runtime is static, and the dynamic alternative needs a musl
    // `libgcc_s`, the unwinder is the one piece of the C toolchain `musl-tools` does not carry, so a
    // glibc machine cannot link one. What that target can hand a C consumer is the static library;
    // the header beside it is the same one every platform is given.
    if is_musl(&workspace) {
        return Tool::cargo().run([
            "rustc",
            "--release",
            "--all-features",
            "-p",
            "rorolala-ffi",
            "--crate-type",
            "staticlib",
        ]);
    }

    Tool::cargo().run(["build", "--release", "--all-features", "-p", "rorolala-ffi"])
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
pub fn build_suite_library(workspace: &Workspace) -> Result<(), crate::Error> {
    if !is_musl(workspace) {
        return build_library();
    }

    let host = format!("{}-unknown-linux-gnu", std::env::consts::ARCH);

    // `CC` is stated for this run because the run around it names `musl-gcc`: a dependency that
    // compiles C would otherwise be compiled by a musl driver into a library the glibc target it is
    // being built for cannot link.
    Tool::cargo().run_with_env(
        &[("CC", OsStr::new("cc"))],
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
    run_step(ec, build_library);

    empty_result!()
}

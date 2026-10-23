//! `lib`: the C ABI artifact — a release shared and static library — and, as a side effect of the
//! root build script, the C header that describes it.

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
    // `libgcc_s` — the unwinder is the one piece of the C toolchain `musl-tools` does not carry, so a
    // glibc machine cannot link one. What that target can hand a C consumer is the static library;
    // the header beside it is the same one every platform is given.
    if workspace
        .target_triple()
        .is_some_and(|triple| triple.contains("musl"))
    {
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

/// Builds the library and the header.
#[chain]
pub fn handle_library(_state: StateLibrary, ec: &mut ResExitCode) -> Next {
    run_step(ec, build_library);

    empty_result!()
}

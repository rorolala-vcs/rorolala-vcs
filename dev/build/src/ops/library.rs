//! `lib`: the C ABI artifact 鈥?a release shared and static library 鈥?and, as a side effect of the
//! root build script, the C header that describes it.

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
pub fn build_library() -> Result<(), crate::Error> {
    Tool::cargo().run(["build", "--release", "--all-features", "-p", "rorolala-ffi"])
}

/// Builds the library and the header.
#[chain]
pub fn handle_library(_state: StateLibrary, ec: &mut ResExitCode) -> Next {
    run_step(ec, build_library);

    empty_result!()
}

//! `cpp-bindgen`: the C++ layer's generated headers, written into the export by hand.
//!
//! An export runs the generator as one of its steps; this is the same generation without the build
//! around it, so a header can be read and compiled while the layer above it is being written. What
//! it reads is the development build's C header, and what it writes is the include tree under
//! `build/`, the same shape an export leaves.

use crate::Error;
use crate::paths::Workspace;
use crate::run_step;
use crate::toolchain::Tool;
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;

/// Writes the generated headers from the development build's C header.
#[command(node = "cpp-bindgen")]
pub fn cpp_bindgen() -> StateCppBindgen {
    StateCppBindgen
}

/// The state the command starts in: it names nothing, since the tree says where both sides live.
#[derive(Grouped)]
pub struct StateCppBindgen;

/// Lays the include tree out from the header a development build writes.
///
/// The header is a side effect of the root build script, so the development build is run first: a
/// warm target directory can hold the library while the header it generated is gone.
///
/// # Errors
///
/// Reports the development build's failure, [`Error::Directory`] when the workspace cannot name
/// where it builds, and the generator's own failure.
pub fn generate_cpp_headers() -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    let header = workspace.debug_header()?;

    if !header.is_file() {
        Tool::cargo().run(["build", "--quiet", "-p", "rororala-ffi"])?;
    }

    crate::cpp::lay_out_includes(&workspace, &workspace.build_dir()?.join("include"), &header)
}

/// Runs the generator over the development header.
#[chain]
pub fn handle_cpp_bindgen(_state: StateCppBindgen, ec: &mut ResExitCode) -> Next {
    run_step(ec, generate_cpp_headers);

    empty_result!()
}

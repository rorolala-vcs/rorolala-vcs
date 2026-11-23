//! `cpp-header`: the C++ umbrella header, compiled as a translation unit of its own.
//!
//! One file that includes `<rola_plus_plus.hpp>` and nothing else answers three things at once: the
//! umbrella is self-sufficient, the generated and hand-written layers are included in an order that
//! compiles, and every `static_assert` the generated layer holds is true of the header beside it. It
//! is a check of the delivered include tree rather than of the sources, so what is compiled is the
//! same layout an export lays out.
//!
//! The generated layer is not in the tree, so it is written into a temporary directory under the
//! workspace before compiling and removed afterwards, the way every temporary here is.

use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{Error, paths::Workspace, toolchain::Tool};

use super::run_check;
use crate::Next;

/// The directory the check works in, inside the workspace so nothing is left outside it.
const SCRATCH: &str = "__cpp_header_check";

/// The translation unit a consumer's first include is.
const SOURCE: &str = "#include <rola_plus_plus.hpp>\nint main() {}\n";

/// The file the check writes that translation unit to.
const CHECK: &str = "include_check.cpp";

/// Checks that the C++ umbrella header compiles on its own.
#[command(node = "cpp-header")]
pub fn cpp_header() -> StateCppHeader {
    StateCppHeader
}

/// The state the check starts in: it names nothing, since the tree says where the headers are.
#[derive(Grouped)]
pub struct StateCppHeader;

/// Lays the include tree out, compiles the one include, and takes the tree away again.
fn run_cpp_header() -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    let header = workspace.header()?;

    if !header.is_file() {
        Tool::cargo().run([
            "build",
            "--quiet",
            "--release",
            "--all-features",
            "-p",
            "rororala-ffi",
        ])?;
    }

    let scratch = workspace.root().join(SCRATCH);
    let compiled = lays_out_and_compiles(&workspace, &header, &scratch);
    let _ = std::fs::remove_dir_all(&scratch);

    compiled
}

/// The check itself, over a scratch directory the caller removes whatever it answered.
fn lays_out_and_compiles(
    workspace: &Workspace,
    header: &Path,
    scratch: &Path,
) -> Result<(), Error> {
    let include = scratch.join("include");
    rola_build::cpp::lay_out_includes(workspace, &include, header)?;

    let source = scratch.join(CHECK);
    std::fs::write(&source, SOURCE).map_err(|error| Error::Io {
        doing: "writing the C++ include check",
        detail: format!("{}: {error}", source.display()),
    })?;

    let compiler = find_compiler().ok_or_else(|| Error::Missing {
        summary: "no C++23 compiler found to check the rola++ headers".to_owned(),
        advice: vec![
            "set `CXX` to a C++23 compiler, or put one on `PATH` as `c++` or `clang++`".to_owned(),
        ],
    })?;

    compiler.run([
        OsString::from("-std=c++23"),
        OsString::from("-Wall"),
        OsString::from("-Wextra"),
        OsString::from("-Werror"),
        OsString::from("-fsyntax-only"),
        OsString::from(format!("-I{}", include.display())),
        OsString::from(format!("-I{}", include.join("rola_plus_plus").display())),
        source.into_os_string(),
    ])
}

/// The C++23 compiler the check runs, from `CXX`, `c++`, `clang++`, or the LLVM install Windows
/// does not put on the path.
///
/// Each candidate is asked to compile an empty translation unit at C++23 before it is chosen. A
/// name on the path that is a compiler but a C++17 one answers the same way a missing one does, and
/// which is which is the candidate's own reply rather than a version number read off text.
fn find_compiler() -> Option<Tool> {
    let mut candidates: Vec<OsString> = Vec::new();

    if let Some(cxx) = std::env::var_os("CXX").filter(|value| !value.is_empty()) {
        candidates.push(cxx);
    }

    candidates.push(OsString::from("c++"));
    candidates.push(OsString::from("clang++"));

    #[cfg(windows)]
    candidates.push(OsString::from(r"C:\Program Files\LLVM\bin\clang++.exe"));

    candidates
        .into_iter()
        .find_map(|candidate| supports_cxx23(&candidate).then(|| Tool::at(candidate)))
}

/// Whether a program compiles an empty translation unit at C++23.
fn supports_cxx23(candidate: &OsStr) -> bool {
    let mut probe = Command::new(candidate);
    probe
        .args([
            "-std=c++23",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fsyntax-only",
            "-x",
            "c++",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let Ok(mut child) = probe.spawn() else {
        return false;
    };

    if let Some(mut input) = child.stdin.take() {
        let _ = input.write_all(b"int main() {}\n");
    }

    child.wait().is_ok_and(|status| status.success())
}

/// Lays the include tree out, compiles the one include, and reports how it ended.
#[chain]
pub fn handle_cpp_header(_state: StateCppHeader, ec: &mut ResExitCode) -> Next {
    run_check(
        "cpp-header",
        "c++ -std=c++23 -Wall -Wextra -Werror -fsyntax-only -I build/include include_check.cpp",
        ec,
        run_cpp_header,
    );

    empty_result!()
}

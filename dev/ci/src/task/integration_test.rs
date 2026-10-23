//! `integration-test`: the integration suites under `tests/`.
//!
//! What either kind of suite checks is the program a caller meets rather than the parts it is made
//! of: a cargo suite is a program rather than a test crate, because what it does is what a caller
//! does — it is run, and it complains, by ending non-zero, when what came back was not what it
//! expected. That is also what lets one do what a test cannot: serve a Vault, reach it, and stop it
//! again.
//!
//! The cargo suites are a workspace of their own, so they are run from beside it: where a suite is
//! run from is what picks its target directory out of `tests/.cargo/config.toml`, which is how
//! running them leaves nothing inside the workspace they test.

use std::path::{Path, PathBuf};

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{Error, paths::Workspace, toolchain::Tool};

use super::run_check;
use crate::Next;

/// Runs the integration suites under `tests/`.
#[command(node = "integration-test")]
pub fn integration_test() -> StateIntegrationTest {
    StateIntegrationTest
}

/// The state the check starts in: it names nothing, since every suite under `tests/` is what is run.
#[derive(Grouped)]
pub struct StateIntegrationTest;

/// The suite that compiles C against the built shared library, and so needs one to exist.
const FFI_SUITE: &str = "ffi-c";

/// Whether this build is for a musl target, which has no shared library to link against.
fn is_musl(workspace: &Workspace) -> bool {
    workspace
        .target_triple()
        .is_some_and(|triple| triple.contains("musl"))
}

/// Why a suite is not run here, when it is not.
///
/// Only the C ABI suite has a reason, and it has two: it links the shared library, which a musl
/// target does not make, and it is POSIX C, which a platform whose C toolchain is not is not.
fn skipped(suite: &str, workspace: &Workspace) -> Option<&'static str> {
    if suite != FFI_SUITE {
        return None;
    }

    if is_musl(workspace) {
        return Some("a musl build has no shared C ABI library to link");
    }

    if cfg!(windows) {
        return Some("the suite is POSIX C, and this platform's C toolchain is not");
    }

    None
}

/// Runs the cargo suites one at a time, then the .NET ones, and names every suite that failed.
fn run_integration_test() -> Result<(), Error> {
    let workspace = Workspace::discover()?;
    rola_build::build_programs()?;

    // The programs are named to a suite by where they are, which has to be said before the suites
    // are entered: from here on, everything is relative to `tests/`.
    let tests = workspace.root().join("tests");
    let bin_dir = workspace.release_dir()?;
    let mut failed: Vec<String> = Vec::new();

    for suite in cargo_suites(&tests)? {
        let name = suite
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or_default()
            .to_owned();

        // The C ABI is met by compiling C against the built shared library and linking it, and that
        // needs two things this run may not have: a shared library, which a musl build does not make
        // (rustc refuses a `cdylib` whose C runtime is static, and the dynamic one needs a musl
        // `libgcc_s` a glibc machine does not carry — see `rola_build::ops::library`), and a POSIX C
        // toolchain, which the suite is written for: `mkdtemp`, `cc -std=c11`, `-Wl,-rpath,`. The
        // suite is skipped where either is missing and run where both hold.
        if let Some(reason) = skipped(&name, &workspace) {
            println!("==> {name}/ skipped: {reason}");
            continue;
        }

        println!("==> {name}/");

        let manifest = suite.join("Cargo.toml");
        let manifest = manifest.to_string_lossy().into_owned();
        let status = Tool::cargo().status_in(
            &tests,
            &[("ROLA_BIN_DIR", bin_dir.as_os_str())],
            ["run", "--manifest-path", &manifest],
        )?;

        if !status.success() {
            failed.push(format!("{name}/"));
        }
    }

    // The .NET suites are run after them, all together, for the same reason: one run reports every
    // suite that failed.
    if let Err(error) = rola_build::dotnet::run_integration_suites(&workspace) {
        eprintln!("{error}");
        failed.push(".NET".to_owned());
    }

    if failed.is_empty() {
        return Ok(());
    }

    println!("==> failed: {}", failed.join(" "));

    Err(Error::Status {
        program: "the integration suites".to_owned(),
        code: 1,
    })
}

/// The suites under `tests/` that are cargo packages: a directory with a `Cargo.toml` is one.
fn cargo_suites(tests: &Path) -> Result<Vec<PathBuf>, Error> {
    let entries = std::fs::read_dir(tests).map_err(|error| Error::Io {
        doing: "reading the integration suites",
        detail: format!("{}: {error}", tests.display()),
    })?;

    let mut suites = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Error::Io {
            doing: "reading the integration suites",
            detail: error.to_string(),
        })?;

        let path = entry.path();
        if path.is_dir() && path.join("Cargo.toml").is_file() {
            suites.push(path);
        }
    }

    suites.sort();

    Ok(suites)
}

/// Runs the cargo suites, then the .NET ones.
#[chain]
pub fn handle_integration_test(_state: StateIntegrationTest, ec: &mut ResExitCode) -> Next {
    run_check(
        "integration-test",
        "cargo run --manifest-path tests/*/Cargo.toml; dotnet test tests/*/*.IntegrationTests.csproj",
        ec,
        run_integration_test,
    );

    empty_result!()
}

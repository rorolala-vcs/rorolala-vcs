//! `clippy`: the workspace linted, with a warning a failure.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::toolchain::Tool;

use super::run_check;
use crate::Next;

/// Lints the workspace, with warnings as errors.
#[command(node = "clippy")]
pub fn clippy() -> StateClippy {
    StateClippy
}

/// The state the check starts in: it names nothing, since the whole workspace is what is linted.
#[derive(Grouped)]
pub struct StateClippy;

/// Runs clippy over the workspace.
///
/// `--all-targets` is what reaches the tests and benches as well as the libraries, and
/// `--all-features` what reaches the code behind a feature nothing enables by default: a warning
/// there is a mistake that would otherwise be met on the day the feature is used.
#[chain]
pub fn handle_clippy(_state: StateClippy, ec: &mut ResExitCode) -> Next {
    run_check(
        "clippy",
        "cargo clippy --workspace --all-targets --all-features -- -D warnings",
        ec,
        || {
            Tool::cargo().run([
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ])
        },
    );

    empty_result!()
}

//! `fmt-check`: whether every crate in the workspace is formatted.
//!
//! Checks rather than rewrites: `fmt` is the command that rewrites, and a gate that rewrote the
//! tree it was handed would be a gate that reports on something other than what was committed.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::toolchain::Tool;

use super::run_check;
use crate::Next;

/// Checks that every crate is formatted, without rewriting anything.
#[command(node = "fmt-check")]
pub fn fmt_check() -> StateFmtCheck {
    StateFmtCheck
}

/// The state the check starts in: it names nothing, since every crate is what is checked.
#[derive(Grouped)]
pub struct StateFmtCheck;

/// Runs cargo's own formatting check over the workspace.
///
/// `--all` is what reaches every package the manifest declares and not only the one a command
/// happened to be run from, and the check's own output is inherited: what it has to say is the diff
/// it will not apply, which is for the reader rather than for this program.
#[chain]
pub fn handle_fmt_check(_state: StateFmtCheck, ec: &mut ResExitCode) -> Next {
    run_check("fmt-check", "cargo fmt --all -- --check", ec, || {
        Tool::cargo().run(["fmt", "--all", "--", "--check"])
    });

    empty_result!()
}

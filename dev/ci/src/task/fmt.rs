//! `fmt`: every crate in the workspace, formatted in place.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::toolchain::Tool;

use super::run_check;
use crate::Next;

/// Formats every crate in the workspace in place.
#[command(node = "fmt")]
pub fn fmt() -> StateFmt {
    StateFmt
}

/// The state the command starts in: it names nothing, since every crate is what is formatted.
#[derive(Grouped)]
pub struct StateFmt;

/// Runs cargo's formatter over the workspace.
#[chain]
pub fn handle_fmt(_state: StateFmt, ec: &mut ResExitCode) -> Next {
    run_check("fmt", "cargo fmt --all", ec, || {
        Tool::cargo().run(["fmt", "--all"])
    });

    empty_result!()
}

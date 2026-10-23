//! `cargo-clean`: cargo's own build output, removed.
//!
//! The export directory survives: it holds a hand-off, not a build.

use crate::toolchain::Tool;
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::run_step;

/// Removes cargo's build output.
#[command(node = "cargo-clean")]
pub fn cargo_clean() -> StateCargoClean {
    StateCargoClean
}

/// The state the command starts in: it names nothing, since cargo's whole output is what is removed.
#[derive(Grouped)]
pub struct StateCargoClean;

/// Runs cargo's own cleaner.
#[chain]
pub fn handle_cargo_clean(_state: StateCargoClean, ec: &mut ResExitCode) -> Next {
    run_step(ec, || Tool::cargo().run(["clean"]));

    empty_result!()
}

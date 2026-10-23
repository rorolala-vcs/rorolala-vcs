//! `doc`: the API documentation of every crate, built.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::toolchain::Tool;

use super::run_check;
use crate::Next;

/// Builds the API documentation for every crate in the workspace.
#[command(node = "doc")]
pub fn doc() -> StateDoc {
    StateDoc
}

/// The state the command starts in: it names nothing, since every crate is what is documented.
#[derive(Grouped)]
pub struct StateDoc;

/// Runs cargo's documentation builder over the workspace.
///
/// `--no-deps` is what keeps the dependencies' documentation out of it: what is being built is this
/// tree's own, and a dependency's is docs.rs's business.
#[chain]
pub fn handle_doc(_state: StateDoc, ec: &mut ResExitCode) -> Next {
    run_check(
        "doc",
        "cargo doc --workspace --no-deps --all-features",
        ec,
        || Tool::cargo().run(["doc", "--workspace", "--no-deps", "--all-features"]),
    );

    empty_result!()
}

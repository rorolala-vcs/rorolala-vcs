//! `doc-open`: the documentation built, then opened.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::toolchain::Tool;

use super::run_check;
use crate::Next;

/// Builds the API documentation, then opens it in a browser.
#[command(node = "doc-open")]
pub fn doc_open() -> StateDocOpen {
    StateDocOpen
}

/// The state the command starts in: it names nothing, since every crate is what is documented.
#[derive(Grouped)]
pub struct StateDocOpen;

/// Runs cargo's documentation builder, asking it to open what it built.
#[chain]
pub fn handle_doc_open(_state: StateDocOpen, ec: &mut ResExitCode) -> Next {
    run_check(
        "doc-open",
        "cargo doc --workspace --no-deps --all-features --open",
        ec,
        || {
            Tool::cargo().run([
                "doc",
                "--workspace",
                "--no-deps",
                "--all-features",
                "--open",
            ])
        },
    );

    empty_result!()
}

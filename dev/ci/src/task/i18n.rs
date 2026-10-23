//! `i18n`: whether every translation key a call site names is stated in both languages.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::toolchain::Tool;

use super::run_check;
use crate::Next;

/// Checks that every i18n key a call site names is stated by the translation files it reads.
#[command(node = "i18n")]
pub fn i18n() -> StateI18n {
    StateI18n
}

/// The state the check starts in: it names nothing, since every call site is what is checked.
#[derive(Grouped)]
pub struct StateI18n;

/// Runs the workspace's own i18n checker.
///
/// It is a crate of the workspace rather than a program beside it, so it is run through cargo: what
/// it reads is the tree's sources, and what it is built with is the workspace's own answer.
#[chain]
pub fn handle_i18n(_state: StateI18n, ec: &mut ResExitCode) -> Next {
    run_check(
        "i18n",
        "cargo run --quiet -p rorolala-dev-i18n-check",
        ec,
        || Tool::cargo().run(["run", "--quiet", "-p", "rorolala-dev-i18n-check"]),
    );

    empty_result!()
}

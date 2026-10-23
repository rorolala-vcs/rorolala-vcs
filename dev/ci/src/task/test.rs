//! `test`: the workspace's own tests, in the profile a test wants.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::toolchain::Tool;

use super::run_check;
use crate::Next;

/// Runs the workspace's tests.
#[command(node = "test")]
pub fn test() -> StateTest {
    StateTest
}

/// The state the check starts in: it names nothing, since the whole workspace is what is tested.
#[derive(Grouped)]
pub struct StateTest;

/// Runs the tests, deliberately on cargo's default profile.
///
/// A release build turns `debug_assert!` off, which is the opposite of what a test run wants: what
/// the tests are for is the checks a debug build makes.
#[chain]
pub fn handle_test(_state: StateTest, ec: &mut ResExitCode) -> Next {
    run_check("test", "cargo test --workspace --all-features", ec, || {
        Tool::cargo().run(["test", "--workspace", "--all-features"])
    });

    empty_result!()
}

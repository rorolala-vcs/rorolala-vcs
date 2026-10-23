//! `check`: the whole gate, and the order it is read in.
//!
//! The gate is composed rather than repeated: every step here is the command of its own name, run
//! in the order a failure is most cheaply met in. `fmt-check` comes first because it is instant and
//! it is the one that is forgotten 閳?`fmt` rewrites the sources in place, so nothing else here would
//! notice a crate that was left unformatted. The i18n check follows it for the same reason, and
//! `generated` after the build rather than before it: what it checks is whether the build changed
//! the files it generates, which only a build can answer.
//!
//! A step that fails stops the run, which is what a shell's `set -e` gave the scripts this replaces:
//! the status handed on is the step's own. What the run recorded is rendered afterwards, so a failed
//! gate is still one page.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use super::{
    StateClippy, StateDoc, StateDotnetCheck, StateDotnetLint, StateFmtCheck, StateGenerated,
    StateI18n, StateIntegrationTest, StateTest, clippy::handle_clippy, doc::handle_doc,
    dotnet_check::handle_dotnet_check, dotnet_lint::handle_dotnet_lint,
    fmt_check::handle_fmt_check, generated::handle_generated, i18n::handle_i18n,
    integration_test::handle_integration_test, run_check, test::handle_test,
};
use crate::{Next, reporter::Reporter};

/// The whole gate.
#[command(node = "check")]
pub fn check() -> StateCheck {
    StateCheck
}

/// The whole gate, under the name it was first known by: `all` and `check` are one run.
#[command(node = "all")]
pub fn all() -> StateCheck {
    StateCheck
}

/// The state the gate starts in: it names nothing, since every step of the gate is what is run.
#[derive(Grouped)]
pub struct StateCheck;

/// What one step of the gate does to the run.
type Step = fn(&mut ResExitCode);

/// Builds what the gate's later steps check 閳?the library, then the programs.
fn build(ec: &mut ResExitCode) {
    run_check(
        "build",
        "cargo build --release --all-features -p rorolala-ffi, then the programs",
        ec,
        rola_build::build_everything,
    );
}

/// Runs every step in order, stopping at the first failure.
#[chain]
pub fn handle_check(_state: StateCheck, ec: &mut ResExitCode) -> Next {
    // The report is of this run rather than of the runs before it.
    Reporter::new().clean();

    let steps: [Step; 10] = [
        |ec| {
            handle_fmt_check(StateFmtCheck, ec);
        },
        |ec| {
            handle_i18n(StateI18n, ec);
        },
        |ec| {
            handle_test(StateTest, ec);
        },
        build,
        |ec| {
            handle_generated(StateGenerated, ec);
        },
        |ec| {
            handle_clippy(StateClippy, ec);
        },
        |ec| {
            handle_doc(StateDoc, ec);
        },
        |ec| {
            handle_integration_test(StateIntegrationTest, ec);
        },
        |ec| {
            handle_dotnet_lint(StateDotnetLint, ec);
        },
        |ec| {
            handle_dotnet_check(StateDotnetCheck, ec);
        },
    ];

    for step in steps {
        step(ec);

        if ec.exit_code != 0 {
            break;
        }
    }

    // Whether the run passed or stopped, it is rendered: a reading of a failed run is the one most
    // worth having.
    if let Err(error) = Reporter::new().collect() {
        eprintln!("warning: {error}");
    }

    empty_result!()
}

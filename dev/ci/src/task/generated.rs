//! `generated`: whether the files the build writes into the tree are the ones the sources produce.
//!
//! Two kinds of file are generated into `src/` rather than into cargo's output, so that a change to
//! either shows up in a diff beside what caused it: `modules/chunking`'s registry and its module
//! list, written by that crate's build script, and each policy's `SCORE.md`, written by the harness.
//! The price of that arrangement is a generated file that can be committed while it disagrees with
//! its sources, and nothing in a build notices — the build would simply rewrite it. This is what
//! notices: generate, then ask git whether anything moved.
//!
//! What is compared is the working tree against `HEAD`, so this passes on a committed tree — which
//! is what a gate runs on. Before a commit it reports the generated files as changed, which is true:
//! they are not committed yet.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{paths::Workspace, toolchain::Tool};

use super::run_check;
use crate::Next;

/// The files generated into the tree rather than into cargo's output. The corpus travels with the
/// `SCORE.md` files the harness writes, and a corpus that changed without a note is a score that
/// silently means something else.
const GENERATED: [&str; 3] = [
    "modules/chunking/src/registry.rs",
    "modules/chunking/src/policies/mod.rs",
    "modules/chunking/corpus",
];

/// Checks that the generated files are the ones the sources produce.
#[command(node = "generated")]
pub fn generated() -> StateGenerated {
    StateGenerated
}

/// The state the check starts in: it names nothing, since the generated files are what is compared.
#[derive(Grouped)]
pub struct StateGenerated;

/// Runs the build that generates, then asks git whether anything moved.
fn run_generated() -> Result<(), rola_build::Error> {
    let workspace = Workspace::discover()?;

    Tool::cargo().run(["build", "--quiet", "-p", "rorolala-chunking"])?;

    // The score is taken in the profile the numbers were taken in, so that a difference is a
    // difference in the code or the corpus rather than in how the harness was built.
    Tool::cargo().run([
        "run",
        "--quiet",
        "--release",
        "-p",
        "rorolala-dev-chunker-harness",
        "--",
        "--score",
    ])?;

    let mut arguments: Vec<&str> = vec!["diff", "--exit-code", "--"];
    arguments.extend(GENERATED);

    // `--exit-code` is what makes a difference a failure: `git diff` alone always succeeds.
    Tool::git().run_in(workspace.root(), arguments)
}

/// Generates, then checks that nothing it wrote disagrees with what is committed.
#[chain]
pub fn handle_generated(_state: StateGenerated, ec: &mut ResExitCode) -> Next {
    run_check(
        "generated",
        "cargo run --release -p rorolala-dev-chunker-harness -- --score; git diff --exit-code",
        ec,
        run_generated,
    );

    empty_result!()
}

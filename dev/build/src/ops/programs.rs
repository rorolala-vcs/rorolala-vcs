//! `bin`: the release command line programs.
//!
//! Compiling them is also what refreshes the completion scripts mingling writes beside cargo's
//! output, so a program added to [`crate::paths::PROGRAMS`] is one a shell can be completed for
//! without anything here changing.

use crate::{
    paths::{PROGRAMS, PROGRAMS_WITHOUT_COMPLETIONS},
    toolchain::Tool,
};
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::run_step;

/// Builds the release command line programs.
#[command(node = "bin")]
pub fn programs() -> StatePrograms {
    StatePrograms
}

/// The state the command starts in: it names nothing, since every program is what is built.
#[derive(Grouped)]
pub struct StatePrograms;

/// Builds every program, named to cargo one at a time.
///
/// A program is named with its own `-p` rather than in one word that happens to hold several: cargo
/// takes one package per flag, and a name is a name.
///
/// # Errors
///
/// Reports the build's own failure.
pub fn build_programs() -> Result<(), crate::Error> {
    let mut arguments: Vec<String> = vec![
        "build".to_owned(),
        "--release".to_owned(),
        "--all-features".to_owned(),
    ];

    for program in PROGRAMS.into_iter().chain(PROGRAMS_WITHOUT_COMPLETIONS) {
        arguments.push("-p".to_owned());
        arguments.push(program.to_owned());
    }

    Tool::cargo().run(arguments)
}

/// Builds every program the tree ships.
#[chain]
pub fn handle_programs(_state: StatePrograms, ec: &mut ResExitCode) -> Next {
    run_step(ec, build_programs);

    empty_result!()
}

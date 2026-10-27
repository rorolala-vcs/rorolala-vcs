//! `rola-build`: what this workspace's artifacts are made of, and how they are laid out.
//!
//! This crate is the product side of the build — the library and the programs, the icons a package
//! carries, the export a hand-off is made of, and the package itself. What *checks* the tree is
//! `rola-ci`; the two are reached as `cargo dev` and `cargo ci`.
//!
//! A build step is a program of the workspace rather than a script beside it, written once and the
//! same on every platform.
//!
//! What a step needs before it can do anything — where the workspace is, where its output goes, how
//! this platform spells an artifact — is [`paths`] and [`platform`]; how a toolchain is run is
//! [`toolchain`]; and every failure a step reports is [`Error`].

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// mingling's macros expand the program's entries and chain functions, and the expansion does not
// carry the `const` a hand-written item would; `missing_const_for_fn` fires on the generated items,
// which cannot carry an allow of their own. The allow has to be this wide for that reason.
#![allow(clippy::missing_const_for_fn)]

use std::io::Write as _;

use mingling::{
    Grouped, RenderResult,
    macros::{gen_program, help, renderer},
    res::ResExitCode,
};

pub mod appimagetool;
pub mod dotnet;
pub mod error;
mod fs;
pub mod iscc;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod nfpm;
mod ops;
pub mod paths;
pub mod platform;
pub mod toolchain;

mod show_shape;

/// The chunking harness, run with the words after `chunker` unchanged.
///
/// It is a pass-through because what it takes is the harness's own options, which the harness prints
/// itself: a second description of them here would be the copy that drifts. It is reached before the
/// command tree, so `cargo dev chunker --help` is the harness's help rather than this program's.
pub use ops::chunker::passthrough as chunker_passthrough;

// The types a command generates live in the module that declares the command, while the program
// collects them by bare name at the crate root — that is why `gen_program!()` below can name them.
// A program that binds paths gets this from `pathf`'s re-exports; a build tool has no paths to bind,
// so its own modules are re-exported here instead.
// The wildcard is on purpose: what is put in scope is what the command macros generate, and a list
// written out by hand would be a second copy of their output that a rename would leave stale.
// The path is kept crate-private on purpose too — the re-export exists for `gen_program!()`, not for
// a caller of this library — which is why `pub(crate)` is not the redundancy clippy reads it as.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use ops::*;
// `gen_program!()` reaches these functions by name, so the re-export exists for the macro rather
// than for a Rust caller, and a wildcard is what that expansion expects.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use show_shape::*;

pub use error::Error;

/// Everything that ships, built: the library first, and the programs after it.
///
/// It is reached by name as well as by the `build` command, because the gate is a sequence of steps
/// and a step it runs is the same step a person runs.
pub use ops::build::build_everything;

/// The release command line programs, built.
///
/// Reached by name for the same reason as [`build_everything`]: the gate runs it as a step.
pub use ops::programs::build_programs;

/// The status of a run that named no command: it asked for nothing, so it is told what there is.
const EC_NOTHING_NAMED: i32 = 1;

/// The status of a run that named a command this program does not have.
const EC_UNKNOWN_COMMAND: i32 = 2;

/// A step that could not do its work, as the renderer is handed it.
#[derive(Grouped)]
pub struct FailedBuild {
    /// What stopped it.
    error: Error,
}

/// Runs one step of the build, and gives the run the status the step failed with.
///
/// A step is the tree's own program run as it always was, so its output goes to the terminal as it
/// runs; what a failure adds is the one line that says what could not be done.
pub(crate) fn run_step(ec: &mut ResExitCode, step: impl FnOnce() -> Result<(), Error>) {
    if let Err(error) = step() {
        eprintln!("{error}");
        ec.exit_code = error.exit_code();
    }
}

/// The program's own help page.
#[help]
pub fn help_global(_: EntryFallback) -> String {
    include_str!("../help.txt").trim_end().to_owned()
}

/// Answers a run whose arguments named no command of this program.
///
/// Naming nothing is not a mistake, so it is shown what there is; naming something that is not one
/// is the mistake, and it is told so.
#[renderer]
pub fn render_fallback(args: EntryFallback, ec: &mut ResExitCode) -> RenderResult {
    let mut result = RenderResult::new();

    if args.is_empty() {
        writeln!(result, "{}", include_str!("../help.txt").trim_end()).ok();
        ec.exit_code = EC_NOTHING_NAMED;
    } else {
        writeln!(result, "no such command: {}", args.join(" ")).ok();
        ec.exit_code = EC_UNKNOWN_COMMAND;
    }

    result
}

/// Reports a step's failure on stderr, and exits with the status it stands for.
///
/// A failure is a build step's answer rather than the program's own: what failed is what the step
/// ran, and its status is the one handed on.
#[renderer]
pub fn render_failed_build(failed: FailedBuild, ec: &mut ResExitCode) -> RenderResult {
    ec.exit_code = failed.error.exit_code();
    eprintln!("{}", failed.error);

    RenderResult::new()
}

gen_program!();

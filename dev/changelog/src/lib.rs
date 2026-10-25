//! `rola-changelog`: the editor behind `cargo changelog`.
//!
//! A changelog is a file with one version at a time open for writing. A draft says what is about to
//! be written; `write` merges it into the version the workspace is at, `archive` dates that version
//! and seals it, and `set-version` moves the workspace on to the next one. The shapes they read are
//! `document` and `draft`; where the files live is `paths`; the version in the manifest is
//! `manifest`; and every failure a command reports is [`Error`].

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

mod date;
mod document;
mod draft;
mod error;
mod git;
mod manifest;
mod ops;
mod paths;
mod setup;
mod tool;
mod version;

pub use error::Error;
pub use setup::{AllowDirtySetup, ResAllowDirty};

// The types a command generates live in the module that declares the command, while the program
// collects them by bare name at the crate root — that is why `gen_program!()` below can name them.
// A program that binds paths gets this from `pathf`'s re-exports; an editor has no paths to bind,
// so its own modules are re-exported here instead.
// The wildcard is on purpose: what is put in scope is what the command macros generate, and a list
// written out by hand would be a second copy of their output that a rename would leave stale.
// The path is kept crate-private on purpose too — the re-export exists for `gen_program!()`, not for
// a caller of this library — which is why `pub(crate)` is not the redundancy clippy reads it as.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use ops::*;

/// The status `get-version` exits with when the workspace states no version.
///
/// It is deliberately not 1: a script asking a tree that has no version yet is asking a question the
/// tree answered, and the answer is "none" rather than "the command failed".
pub const EC_NO_VERSION: i32 = 10;

/// The status of a run that named no command: it asked for nothing, so the help is shown.
const EC_NOTHING_NAMED: i32 = 1;

/// The status of a run that named a command this program does not have.
const EC_UNKNOWN_COMMAND: i32 = 2;

/// A command that could not do its work, as the renderer is handed it.
#[derive(Grouped)]
pub struct FailedChangelog {
    /// What stopped it.
    pub error: Error,
}

/// Runs one command's work, and gives the run the status the failure stands for.
///
/// What a command does is run a function; the function's own output goes wherever it was written,
/// and what a failure adds is the one line saying what could not be done.
pub(crate) fn run_step(ec: &mut ResExitCode, step: impl FnOnce() -> Result<(), Error>) {
    if let Err(error) = step() {
        eprintln!("{error}");
        ec.exit_code = error.exit_code();
    }
}

/// The program's own help page.
#[help]
pub fn help_global(_: EntryFallback) -> String {
    include_str!("../help.txt").to_owned()
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

/// Reports a command's failure on stderr, and exits with the status it stands for.
#[renderer]
pub fn render_failed_changelog(failed: FailedChangelog, ec: &mut ResExitCode) -> RenderResult {
    ec.exit_code = failed.error.exit_code();
    eprintln!("{}", failed.error);

    RenderResult::new()
}

gen_program!();

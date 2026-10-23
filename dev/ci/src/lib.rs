//! `rola-ci`: what rola is checked with.
//!
//! A check is a command of the workspace rather than a script beside it, so the gate is one program
//! with one exit status and one place each check lives. The programs it checks are built by
//! `rola-build`; the two are reached as `cargo ci` and `cargo dev`.
//!
//! The checks still under `./run.sh` are being moved here one at a time, so both entry points work
//! until the last one has arrived.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]

use std::io::Write as _;

use mingling::{
    RenderResult,
    macros::{gen_program, help, renderer},
    res::ResExitCode,
};

mod cmd;
mod git;
mod reporter;
mod task;

// The types a command generates live in the module that declares the command, while the program
// collects them by bare name at the crate root — that is why `gen_program!()` below can name them.
// A program that binds paths gets this from `pathf`'s re-exports; a gate has no paths to bind, so
// its own modules are re-exported here instead.
// The wildcard is on purpose: what is put in scope is what the command macros generate, and a list
// written out by hand would be a second copy of their output that a rename would leave stale.
// The path is kept crate-private on purpose too — the re-export exists for `gen_program!()`, not for
// a caller of this library — which is why `pub(crate)` is not the redundancy clippy reads it as.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use cmd::*;
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use task::*;

/// The status of a run that named no command: it asked for nothing, so it is told what there is.
const EC_NOTHING_NAMED: i32 = 1;

/// The status of a run that named a check this program does not have.
const EC_UNKNOWN_COMMAND: i32 = 2;

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

gen_program!();

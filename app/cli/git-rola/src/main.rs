//! Git support for rola, as a program Git reaches for as `git rola`.
//!
//! A command is a node of this program's dispatch tree, a chain that does the work, and a renderer
//! that writes the answer through mingling's buffer. What it says is git's: `fatal:` and `error:`
//! on stderr, the command's own usage when a line is not one it reads, and the version and the
//! usage list on stdout. The command line is read before the dispatch tree sees it, because git's
//! rules are about the command boundary and the framework's global flags are not; see [`options`].
//!
//! The words come from `i18n/` in the language git would speak, chosen from `LANGUAGE`, `LC_ALL`,
//! `LC_MESSAGES` and `LANG`; see [`locale`]. A pager is chosen the way git chooses one, and both
//! streams are written through it; see [`output`].

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// mingling's macros expand the program's chains and renderers, and the expansion does not carry the
// `const` a hand-written item would; `missing_const_for_fn` fires on the generated items, which
// cannot carry an allow of their own.
#![allow(clippy::missing_const_for_fn)]

rust_i18n::i18n!("i18n", fallback = "en");

use mingling::{
    Grouped,
    macros::{buffer, gen_program, r_eprintln, renderer},
    res::ResExitCode,
    setup::ExitCodeSetup,
};
use rust_i18n::t;

mod bring;
mod check;
mod commands;
mod exit_codes;
mod git_config;
mod locale;
mod options;
mod output;
mod pathspec;
mod quote;
mod source;
mod update;
mod upstream;
mod usage;
mod wildmatch;
mod wrap;

use crate::exit_codes::{EC_ERR, EC_FAILED, EC_USAGE};
use crate::output::OutputSetup;

/// A command that could not be made, said the way git says it.
#[derive(Grouped)]
pub struct Fatal {
    /// What could not be done, as the run says it.
    pub message: String,
    /// The status the run ends with.
    pub code: i32,
}

impl Fatal {
    /// A failure that ends the run with [`EC_ERR`], which is git's `fatal:`.
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: EC_ERR,
        }
    }
}

/// Renders a command that could not be made the way git says it.
#[renderer(buffer)]
pub fn render_fatal(error: Fatal, ec: &mut ResExitCode) {
    r_eprintln!("{}{}", t!("message.fatal"), error.message);
    ec.exit_code = error.code;
}

/// A command line whose operands its command does not take, answered with that command's usage.
#[derive(Grouped)]
pub struct Refused {
    /// The command the operands were written for.
    pub command: &'static str,
}

/// Renders a command line the command does not read: its usage, on stderr, with git's status.
///
/// git writes the usage alone here rather than an `error:` line, which is what `git var GIT_PAGER
/// extra` does: the line was readable, it just named something the command has no use for.
#[renderer(buffer)]
pub fn render_refused(refused: Refused, ec: &mut ResExitCode) {
    r_eprintln!("{}", usage::command(refused.command));
    ec.exit_code = EC_USAGE;
}

/// A name `restore` was given that selects no material, said the way git says a pathspec that
/// matched nothing.
#[derive(Grouped)]
pub struct PathspecRefused {
    /// Why the name selected nothing.
    pub refusal: crate::pathspec::Refusal,
}

/// Renders a pathspec that selected nothing.
#[renderer(buffer)]
pub fn render_pathspec_refused(refused: PathspecRefused, ec: &mut ResExitCode) {
    let said = match &refused.refusal {
        pathspec::Refusal::NoMatch(name) => t!("restore.err_pathspec", name = name.as_str()),
        pathspec::Refusal::Outside(name) => t!("restore.err_outside", name = name.as_str()),
        pathspec::Refusal::Invalid(name) => t!("restore.err_path", name = name.as_str()),
    };

    r_eprintln!("{}{said}", t!("message.error"));
    ec.exit_code = EC_FAILED;
}

/// A `bring` run that named something it cannot sign in, said as the run that asked for it.
#[derive(Grouped)]
pub struct BringRefused {
    /// Why the reference, the destination or the landing cannot be used.
    pub refusal: crate::bring::Refusal,
}

/// Renders a bring that was refused before anything was written.
#[renderer(buffer)]
pub fn render_bring_refused(refused: BringRefused, ec: &mut ResExitCode) {
    let said = match &refused.refusal {
        bring::Refusal::Unknown { reference, vault } => t!(
            "bring.err_unknown",
            reference = reference.as_str(),
            vault = vault.as_str()
        ),
        bring::Refusal::Unnamed { id, vault } => t!(
            "bring.err_unnamed",
            uuid = id.to_string(),
            vault = vault.as_str()
        ),
        bring::Refusal::Destination => t!("bring.err_destination"),
        bring::Refusal::Outside(path) => t!("bring.err_outside", path = path.as_str()),
        bring::Refusal::Taken(path) => t!("bring.err_taken", path = path.as_str()),
        bring::Refusal::Known(uuid) => t!("bring.err_known", uuid = uuid.to_string()),
        bring::Refusal::PathKnown(path) => t!("bring.err_path_known", path = path.as_str()),
    };

    r_eprintln!("{}{said}", t!("message.error"));
    ec.exit_code = EC_FAILED;
}

/// Asks the kernel to kill the process on `SIGPIPE`, the way a Unix filter is expected to die.
///
/// Rust starts every process with `SIGPIPE` ignored, so a write to a pipe whose reader has gone,
/// `git rola check | head`, or a pager that has been quit, returns `EPIPE` instead. Dying on the
/// signal is what every other Unix tool does, it is what git does, and it leaves the status a pipe
/// expects instead of a panic over the output the reader was consuming.
#[cfg(unix)]
fn restore_default_sigpipe() {
    // SAFETY: single-threaded at this point, and the call only resets `SIGPIPE` to the disposition
    // a fresh process already carries; it installs no handler of our own.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// Leaves the process as it is: Windows has no `SIGPIPE`, and a broken pipe is reported to the
/// write itself rather than delivered as a signal.
#[cfg(not(unix))]
fn restore_default_sigpipe() {}

fn main() {
    restore_default_sigpipe();
    locale::select();

    // The words are read here rather than by the framework, because git's reading of them is about
    // where a command's own words begin: see `options` for what decides an answer and what is left
    // to the dispatch tree.
    let words: Vec<String> = std::env::args().skip(1).collect();

    match options::decide(&words) {
        options::Decision::Draw(answer) => options::draw(answer),
        options::Decision::Wrap { verb, rest } => std::process::exit(wrap::run(&verb, &rest)),
        options::Decision::Dispatch => {}
    }

    let mut program = ThisProgram::new();
    program.with_setup(ExitCodeSetup);
    program.with_setup(OutputSetup);
    program.exec_and_exit();
}

gen_program!();

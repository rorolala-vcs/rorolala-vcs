//! Git support for rola, as a program Git reaches for as `git rola`.
//!
//! A command is a node of this program's dispatch tree, a chain that does the work, and a renderer
//! that writes the answer through mingling's buffer. The output is git's, not rola's: quiet on
//! success, one line per thing that changed where a command lists things, and every failure on
//! stderr as `git-rola: ...`.
//!
//! What it says is translated the way the rest of rola says it: every word comes from `i18n/` in
//! the language the run speaks, and the language is chosen by `--lang` or the environment.

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
    hook::ProgramHook,
    macros::{buffer, gen_program, help, r_eprintln, r_println, renderer},
    picker::UnmatchedArgs,
    res::ResExitCode,
    setup::DefaultSetup,
};
use rust_i18n::t;

mod check;
mod commands;
mod exit_codes;
mod lang;
mod pathspec;
mod source;
mod update;
mod wrap;

use crate::exit_codes::{EC_ERR, EC_OK, EC_USAGE};
use crate::lang::LanguageSetup;

/// A command that could not be made, said the way the program says it.
#[derive(Grouped)]
pub struct Fatal {
    /// What could not be done, as the run says it.
    pub message: String,
    /// The status the run ends with.
    pub code: i32,
}

impl Fatal {
    /// A failure that ends the run with [`EC_ERR`].
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: EC_ERR,
        }
    }
}

/// Renders a command that could not be made the way the program says it.
#[renderer(buffer)]
pub fn render_fatal(error: Fatal, ec: &mut ResExitCode) {
    r_eprintln!("git-rola: {}", error.message);
    ec.exit_code = error.code;
}

/// Answers a run that named something this program does not read.
pub(crate) fn usage() -> Next {
    Fatal {
        message: t!("git_rola.err_usage").to_string(),
        code: EC_USAGE,
    }
    .into()
}

/// Answers the run the dispatch tree reached for anything it was given no command for.
///
/// A bare `git rola` and a mistyped word both end here; there is no default action to take on a
/// work tree, so both are refused rather than made to look like success.
#[renderer(buffer)]
pub fn render_fallback(_args: EntryFallback, ec: &mut ResExitCode) {
    r_eprintln!("git-rola: {}", t!("git_rola.err_usage"));
    ec.exit_code = EC_USAGE;
}

/// Prints the help a run asks for.
///
/// git's own help is a usage line and a list of what the program can do, so that is what is said
/// here — the list is the program's, not a page of prose. It goes to stdout and ends the run with
/// success, the way `git help` does.
#[help(buffer)]
pub fn help_fallback(_: EntryFallback, ec: &mut ResExitCode) {
    r_println!("{}", t!("git_rola.help").trim_end());
    ec.exit_code = EC_OK;
}

/// Whether a run left words the command did not read.
pub(crate) fn refused(unmatched: &UnmatchedArgs) -> bool {
    !unmatched.is_empty()
}

/// Whether the run asked for help or the version.
///
/// The flags are taken out of the arguments before a hook runs, so the question is read from the
/// command line the process was started with. A run that asked one is not a run of a git command,
/// and the words that follow must not be handed to git as if it were.
fn asks_a_question() -> bool {
    std::env::args().any(|word| matches!(word.as_str(), "-h" | "--help" | "-v" | "--version"))
}

fn main() {
    let mut program = ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.with_setup(LanguageSetup);
    program.with_hook(
        ProgramHook::empty()
            // `--version` is a question answered before any command: git says which program it is
            // in one line, so that is what is written.
            .on_version(|info| {
                let version = env!("CARGO_PKG_VERSION");
                info.result
                    .println(t!("git_rola.version", version = version));
            })
            .on_pre_dispatch(|info| {
                if asks_a_question() {
                    // There is one help, at the program's root, and every `--help` asks for it: a
                    // command's own words are a question about the program rather than a run of
                    // that command, so the dispatch tree is left with the fallback entry.
                    info.arguments.clear();
                } else {
                    // git has no hook before a checkout, a merge or a pull, so the words that name
                    // one of those are answered here, before the dispatch tree sees them: the
                    // materials are taken out, git is run, and they are put back.
                    wrap::intercept(info.arguments);
                }
            }),
    );
    program.exec_and_exit();
}

gen_program!();

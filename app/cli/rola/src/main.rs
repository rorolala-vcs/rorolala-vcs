#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]

rust_i18n::i18n!("i18n", fallback = "en");

use mingling::{
    macros::{buffer, gen_program, help, r_eprintln, r_println, renderer},
    res::ResExitCode,
    setup::DefaultSetup,
};
use rorolala_cli_setups::RorolalaSetup;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

mod cmd_create;
mod cmd_init;
mod error;
mod exit_codes;
mod tools;

use crate::exit_codes::EC_HELP;

fn main() {
    let mut program = ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.with_setup(RorolalaSetup);
    program.exec_and_exit();
}

/// Prints the help a run falls back to when no command is named.
///
/// A run whose arguments name no command — `rola`, or `rola -h` — reaches no entry of its
/// own, so it lands on the fallback. What it is shown there is this: the whole of what the
/// program can do.
#[help(buffer)]
pub(crate) fn help_rola(_: EntryFallback, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("rola.help")).trim());
    ec.exit_code = EC_HELP;
}

#[renderer(buffer)]
pub(crate) fn render_fallback(args: EntryFallback) {
    r_println!("Command not found: {}", args.join(" "));
}

gen_program!();

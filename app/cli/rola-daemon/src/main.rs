#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// mingling's macros expand the program's entries and chain functions, and the expansion does not
// carry the `const` a hand-written item would; `missing_const_for_fn` fires on the generated items,
// which cannot carry an allow of their own. The allow has to be this wide for that reason.
#![allow(clippy::missing_const_for_fn)]
// A chain function is handed its previous value by value, and its result is routed by the
// framework rather than read by a caller; both lints would fire on the shape every chain has.
#![allow(clippy::must_use_candidate, clippy::needless_pass_by_value)]

rust_i18n::i18n!("i18n", fallback = "en");

use mingling::{
    Grouped,
    macros::{buffer, chain, gen_program, help, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ColorizeSetup, ConcurrencySetup, FormatSetup, LanguageSetup, RorolalaDefaultSetup,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::{
    cmd_start::StateStart,
    exit_codes::{EC_HELP, EC_UNKNOWN_COMMAND},
    failure::failure,
    port::PortSetup,
};

mod cmd_check;
mod cmd_list;
mod cmd_register;
mod cmd_remove;
mod cmd_start;
mod errors;
mod exit_codes;
mod failure;
mod port;

fn main() {
    // A console that will not take VT output would show the escape sequences rather than colour, so
    // the colour is dropped instead of the run: the daemon's work is the service, not its palette.
    #[cfg(windows)]
    if colored::control::set_virtual_terminal(true).is_err() {
        rorolala_utils_cli_theme::set_enabled(false);
    }

    let mut program = ThisProgram::new();
    program.with_setup(RorolalaDefaultSetup);
    program.with_setup(LanguageSetup);
    program.with_setup(ColorizeSetup);
    program.with_setup(ConcurrencySetup);
    program.with_setup(PortSetup);
    program.with_setup(FormatSetup::new(t!("format.conflict_json").to_string()));
    program.exec_and_exit();
}

/// The default a run names nothing for.
///
/// A run that named nothing asked for the daemon; a run that named something no command answers
/// to is a mistake.
#[chain]
pub fn handle_fallback(args: EntryFallback) -> Next {
    if args.is_empty() {
        StateStart.into()
    } else {
        ErrorUnknownCommand {
            typed: args.join(" "),
        }
        .into()
    }
}

/// Prints the help a run asks for.
#[help(buffer)]
pub fn help_daemon(_: EntryFallback, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("daemon.help")).trim());
    ec.exit_code = EC_HELP;
}

/// Error: the run named a command this program does not have.
#[derive(Grouped)]
pub struct ErrorUnknownCommand {
    /// What was typed, as it was typed.
    typed: String,
}

impl Failure for ErrorUnknownCommand {
    fn name(&self) -> &'static str {
        "error_unknown_command"
    }

    fn reason(&self) -> String {
        t!("daemon.err_unknown_command", command = self.typed).to_string()
    }
}

failure!(ErrorUnknownCommand);

/// Reports the word that named no command.
#[renderer(buffer)]
pub fn render_error_unknown_command(error: ErrorUnknownCommand, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!(
        "{}",
        help_line!(t!("daemon.err_unknown_command_help").trim())
    );
    ec.exit_code = EC_UNKNOWN_COMMAND;
}

gen_program!();

//! The `rola explain` command: what a number or a name the program left behind means.
//!
//! `explain` is a namespace rather than one thing to say — an exit code is what it explains
//! today, and what it explains is what the subcommands under it grow into. Each of those is
//! therefore two layers deep, `explain exit-code` for one, so that adding another is adding
//! another word rather than reshaping this one.
//!
//! What is explained is generated: the keys for the exit codes live beside the codes
//! themselves, `build.rs` turns them into
//! [`explain_ec`](crate::exit_codes::explain_ec), and the words they name live in the
//! locale files, so a code is spoken in the run's language rather than in the program's.
//!
//! The code a run is usually asking about is the one it just ended with, so `explain
//! exit-code` with nothing named reaches for the code the run before this one ended with,
//! kept under the user's local data directory.

use mingling::{
    Grouped,
    macros::{arg, buffer, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{self, EC_ERR_EXPLAIN_NO_LASTEC, EC_ERR_EXPLAIN_UNKNOWN, EC_HELP};
use crate::lastec;

#[help(buffer)]
pub fn help_explain(_: EntryExplain, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("explain.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryExplain)]
pub fn desc_explain() -> Description {
    t!("explain.cmd_explain_description").to_string().into()
}

/// Names what `explain` can explain.
///
/// `explain` is a namespace, so reaching it with none of its subcommands is a question about
/// it rather than about one of them, and it is answered the way a question about a command
/// is: with what it can do.
#[command(node = "explain")]
pub fn explain() -> Next {
    ResultExplainHelp.into()
}

/// Result: what `explain` can explain was named.
///
/// The same answer [`help_explain`] gives, since a run that reached `explain` with nothing
/// and a run that asked it for help are asking the same thing. What it says is written out
/// twice rather than shared, because what puts it on the screen — `r_eprintln!` — is a
/// buffer only the attributes open.
#[derive(Grouped)]
pub struct ResultExplainHelp;

#[renderer(buffer)]
pub fn render_result_explain_help(_: ResultExplainHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("explain.help")).trim());
    ec.exit_code = EC_HELP;
}

#[help(buffer)]
pub fn help_explain_exit_code(_: EntryExplainExitCode, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("explain_exit_code.help")).trim());

    // The codes the help has just promised are listed below it, read where they are
    // generated: what a code means is the one thing this command was asked to say, so the
    // help says it for every code there is rather than only for the one being asked about.
    for code in exit_codes::CODES {
        r_eprintln!("{code:>3}  {}", exit_codes::explain_ec(*code));
    }

    ec.exit_code = EC_HELP;
}

#[metadata(EntryExplainExitCode)]
pub fn desc_explain_exit_code() -> Description {
    t!("explain_exit_code.cmd_explain_exit_code_description")
        .to_string()
        .into()
}

/// Explains the exit code named, or the one the last run ended with.
///
/// Naming no code asks about the run just before this one, whose exit code is recorded as
/// it ends. A code the program states is spoken in the run's language; one it does not is
/// reported rather than passed over, whether it was named or read from the last run, since
/// a number no code stands for is not something there is anything of the program to say.
///
/// # Errors
///
/// Renders [`ErrorNoLastExitCode`] when no code was named and none was recorded,
/// [`ErrorUnknownExitCode`] when the code named is not one the program states, and
/// [`ErrorLastExitCodeUnknown`] when the recorded one is not.
#[command(node = "explain.exit-code")]
pub fn explain_exit_code(args: EntryExplainExitCode) -> Next {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what
    // makes this about the run before.
    let named: Option<i32> = args.pick(&arg![Option<i32>]).unwrap();

    let code = match named {
        Some(code) => code,
        None => match lastec::last() {
            Some(code) => code,
            None => return ErrorNoLastExitCode.into(),
        },
    };

    if !exit_codes::has_ec(code) {
        return if named.is_some() {
            ErrorUnknownExitCode { code }.into()
        } else {
            ErrorLastExitCodeUnknown { code }.into()
        };
    }

    ResultExitCode {
        code,
        meaning: exit_codes::explain_ec(code),
    }
    .into()
}

/// Result: an exit code was explained.
#[derive(Grouped)]
pub struct ResultExitCode {
    /// The code that was asked about.
    code: i32,
    /// What it means, in the run's language.
    meaning: String,
}

#[renderer(buffer)]
pub fn render_result_exit_code(result: ResultExitCode) {
    r_println!(
        "{}",
        t!(
            "explain_exit_code.result",
            code = result.code,
            meaning = result.meaning
        )
        .trim()
    );
}

/// Error: no code was named, and none was recorded to explain.
#[derive(Grouped)]
pub struct ErrorNoLastExitCode;

#[renderer(buffer)]
pub fn render_error_no_last_exit_code(_: ErrorNoLastExitCode, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("explain_exit_code.err_no_lastec").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("explain_exit_code.err_no_lastec_help").trim())
    );
    ec.exit_code = EC_ERR_EXPLAIN_NO_LASTEC;
}

/// Error: the code named is not one the program states.
#[derive(Grouped)]
pub struct ErrorUnknownExitCode {
    /// The code that is not one the program states.
    code: i32,
}

#[renderer(buffer)]
pub fn render_error_unknown_exit_code(error: ErrorUnknownExitCode, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("explain_exit_code.err_unknown_code", code = error.code).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("explain_exit_code.err_unknown_code_help").trim())
    );
    ec.exit_code = EC_ERR_EXPLAIN_UNKNOWN;
}

/// Error: the code the last run ended with is not one the program states.
#[derive(Grouped)]
pub struct ErrorLastExitCodeUnknown {
    /// The code the last run ended with.
    code: i32,
}

#[renderer(buffer)]
pub fn render_error_last_exit_code_unknown(error: ErrorLastExitCodeUnknown, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("explain_exit_code.err_lastec_unknown", code = error.code).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("explain_exit_code.err_lastec_unknown_help").trim())
    );
    ec.exit_code = EC_ERR_EXPLAIN_UNKNOWN;
}

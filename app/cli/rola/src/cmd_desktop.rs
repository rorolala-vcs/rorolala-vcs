//! The `rola desktop` command: open the Desktop program that sits beside this one.
//!
//! The command line is a way to work without a window, and the Desktop program is the window
//! for the work that suits. The two are exported together — this program at `bin/rola` and the
//! Desktop program at `bin/desktop/` — so a run reaches it by starting from where this program
//! was itself run from: nothing is named, and nothing is looked up beyond that.
//!
//! What the two would otherwise each work out for themselves is handed over with the program:
//! the language this run speaks, and the directory it was made in. A window is then opened onto
//! the same work, in the same language, as the run that asked for it.
//!
//! The program is run as a child and **waited for** rather than started and left behind: this command
//! is a way to the window, and a way to it lasts as long as the window does. What the window ends with
//! is what this run ends with, so a caller that waits reads the window's own answer.

use std::path::{Path, PathBuf};
use std::process::Command;

use mingling::{
    Grouped,
    macros::{buffer, chain, command, help, metadata, r_eprintln, renderer, routeify},
    metadata::Description,
    res::{ResCurrentDir, ResExitCode},
};
use rorolala_cli_setups::ResLanguage;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_DESKTOP_ENDED, EC_ERR_DESKTOP_LAUNCH_FAILED, EC_ERR_DESKTOP_NOT_FOUND, EC_HELP,
};
use crate::failure::failure;

/// The directory, beside this program, the Desktop program is exported into.
const DESKTOP_DIR: &str = "desktop";

/// The Desktop program's name, before the suffix a platform gives a program.
const DESKTOP_PROGRAM: &str = "RorolalaDesktop";

/// What the language this run speaks is handed over under.
///
/// One argument, written as a name and a value rather than as two arguments, so what a run
/// says and where it was made are told apart by name and neither has to be counted for.
const LANGUAGE_ARG: &str = "-Lang:";

/// What the directory this run was made in is handed over under.
const DIRECTORY_ARG: &str = "-CurrentDir:";

#[help(buffer)]
pub fn help_desktop(_: EntryDesktop, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("desktop.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryDesktop)]
pub fn desc_desktop() -> Description {
    t!("desktop.cmd_desktop_description").to_string().into()
}

/// Opens the Desktop program that sits beside this one.
///
/// The Desktop program is where the work that a windowless run does not suit is done, so this is
/// a way to it rather than a way to work: the program is started and the run ends, leaving it to
/// the reader. It sits in a `desktop` directory beside the program that was run — the layout
/// `./run.sh export` lays down — so a run reaches it without being told where it is.
///
/// # Errors
///
/// Renders [`ErrorDesktopNotFound`] when there is no Desktop program beside this one, and
/// [`ErrorDesktopFailed`] when there is one but it would not start.
#[command(node = "desktop")]
pub fn desktop() -> StateDesktop {
    StateDesktop
}

/// The state a run that opens the Desktop program starts in.
///
/// It names nothing: where the program sits is where this one was run from, so the whole of
/// what the run is told is already in where that is.
#[derive(Grouped)]
pub struct StateDesktop;

#[chain(routeify)]
pub fn handle_desktop(_state: StateDesktop, language: &ResLanguage, cwd: &ResCurrentDir) -> Next {
    // Where this program was run from is what the Desktop program sits beside, so a run that
    // cannot say where it is cannot find it either.
    let here = match std::env::current_exe() {
        Ok(here) => here,
        Err(error) => {
            return ErrorDesktopFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let program = desktop_program(&here);

    if !program.is_file() {
        return ErrorDesktopNotFound.into();
    }

    // Run and waited for rather than started and left: the window is the whole of what this command does,
    // so the command lasts as long as the window does. What the program ended with is what this run ends
    // with — the two are one thing to whoever asked for a window, and a caller that waits for a program is
    // entitled to its answer.
    match Command::new(&program)
        .args([language_arg(language), directory_arg(cwd)])
        .status()
    {
        Ok(status) => ResultDesktopEnded {
            code: status.code(),
        }
        .into(),
        Err(error) => ErrorDesktopFailed {
            cause: error.to_string(),
        }
        .into(),
    }
}

/// The argument that hands over the language this run speaks.
fn language_arg(language: &ResLanguage) -> String {
    format!("{LANGUAGE_ARG}{}", language.as_str())
}

/// The argument that hands over the directory this run was made in.
fn directory_arg(cwd: &ResCurrentDir) -> String {
    format!("{DIRECTORY_ARG}{}", cwd.display())
}

/// Where the Desktop program sits, given where this program was run from.
///
/// A platform names a program with a suffix or without, so the name is put together rather than
/// written out once per platform.
fn desktop_program(here: &Path) -> PathBuf {
    let suffix = std::env::consts::EXE_SUFFIX;
    let name = format!("{DESKTOP_PROGRAM}{suffix}");
    let dir = here.parent().unwrap_or_else(|| Path::new("."));

    dir.join(DESKTOP_DIR).join(name)
}

/// Result: the Desktop program was waited for and has ended.
#[derive(Grouped)]
pub struct ResultDesktopEnded {
    /// The code it ended with, or nothing when a signal ended it rather than an ordinary return.
    code: Option<i32>,
}

#[renderer(buffer)]
pub fn render_result_desktop_ended(result: ResultDesktopEnded, ec: &mut ResExitCode) {
    // What the program ended with is what this run ends with. A program a signal ended has no code of its
    // own, and is one that did not end well, which is what this run says in its place.
    ec.exit_code = result.code.unwrap_or(EC_ERR_DESKTOP_ENDED);
}

/// Error: there is no Desktop program beside this one.
#[derive(Grouped)]
pub struct ErrorDesktopNotFound;

impl Failure for ErrorDesktopNotFound {
    fn name(&self) -> &'static str {
        "error_desktop_not_found"
    }

    fn reason(&self) -> String {
        t!("desktop.err_not_found").trim().to_string()
    }
}

failure!(ErrorDesktopNotFound);

#[renderer(buffer)]
pub fn render_error_desktop_not_found(error: ErrorDesktopNotFound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("desktop.err_not_found_help").trim()));
    ec.exit_code = EC_ERR_DESKTOP_NOT_FOUND;
}

/// Error: the Desktop program would not start.
#[derive(Grouped)]
pub struct ErrorDesktopFailed {
    /// Why it would not start.
    cause: String,
}

impl Failure for ErrorDesktopFailed {
    fn name(&self) -> &'static str {
        "error_desktop_failed"
    }

    fn reason(&self) -> String {
        t!("desktop.err_launch_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorDesktopFailed);

#[renderer(buffer)]
pub fn render_error_desktop_failed(error: ErrorDesktopFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("desktop.err_launch_failed_help").trim())
    );
    ec.exit_code = EC_ERR_DESKTOP_LAUNCH_FAILED;
}

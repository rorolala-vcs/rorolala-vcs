//! The `rola desktop` command: open the Desktop program that sits beside this one.
//!
//! The command line is a way to work without a window, and the Desktop program is the window
//! for the work that suits. The two are exported together — this program at `bin/rola` and the
//! Desktop program at `bin/desktop/` — so a run reaches it by starting from where this program
//! was itself run from: nothing is named, and nothing is looked up beyond that.
//!
//! What the two would otherwise each work out for themselves is handed over with the program:
//! the language this run speaks, the directory to open onto, and where this program itself is. The
//! directory is the Workspace the run was made in rather than the run's own directory, since the work is
//! the Workspace — the tree is rooted at its directory, and the Layouts and the ownership the browser
//! shows are read against it — so a window opened onto a directory inside one would be a window onto part
//! of the work. A run no Workspace holds opens onto the directory it was made in, which is all there is
//! to open onto. And this program's own path is what the window's file operations run, which is what makes
//! them the operations of the program that was asked for the window rather than of whatever a `PATH` holds.
//!
//! The program is run as a child and **waited for** rather than started and left behind: this command
//! is a way to the window, and a way to it lasts as long as the window does. What the window ends with
//! is what this run ends with, so a caller that waits reads the window's own answer.

use std::path::{Path, PathBuf};
use std::process::Command;

use mingling::{
    Grouped, Suggest,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, renderer, routeify, suggest,
    },
    metadata::Description,
    res::{ResCurrentDir, ResExitCode},
};
use rorolala_cli_setups::ResLanguage;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;
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

/// What the directory a window is to open onto is handed over under.
///
/// The program is started in the same directory, so this is what the two agree on rather than the only
/// way the answer reaches it: a program that reads where it is reads the directory it was started in,
/// and one that reads its command line reads this.
const DIRECTORY_ARG: &str = "-CurrentDir:";

/// What this program's own path is handed over under, as the environment holds it.
///
/// The Desktop carries its file operations out by running `rola`, and the command templates name the
/// program through this rather than through the bare word: what is run beside this window is the program
/// that was asked for the window, and a `PATH` is the machine's answer rather than this one's. It is the
/// environment rather than an argument because a child of the window — the command itself — inherits it,
/// which is what lets a template name a program it was never told about.
const EXE_ENV: &str = "ROLA_EXE";

#[help(buffer)]
pub fn help_desktop(_: EntryDesktop, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("desktop.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryDesktop)]
pub fn desc_desktop() -> Description {
    t!("desktop.cmd_desktop_description").to_string().into()
}

/// Completes what `rola desktop` can be given next.
///
/// The command names nothing at all, so there is nothing to offer rather than a filename that
/// would not be accepted.
#[completion(EntryDesktop)]
pub fn complete_desktop() -> Suggest {
    suggest!()
}

/// Opens the Desktop program that sits beside this one.
///
/// The Desktop program is where the work that a windowless run does not suit is done, so this is
/// a way to it rather than a way to work: the program is started and the run ends, leaving it to
/// the reader. It sits in a `desktop` directory beside the program that was run — the layout
/// `./run.sh export` lays down — so a run reaches it without being told where it is.
///
/// The window opens onto the Workspace the run was made in, or onto the directory it was made in
/// when no Workspace holds it. See [`opening_at`] for what a Workspace is the answer rather than the
/// run's own directory.
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

    // What the window is to open onto, which is what the program is both started in and told: started in,
    // because that is what a program that reads where it is reads, and told, because the two programs
    // state the same thing rather than each working it out.
    let at = opening_at(cwd);

    // Run and waited for rather than started and left: the window is the whole of what this command does,
    // so the command lasts as long as the window does. What the program ended with is what this run ends
    // with — the two are one thing to whoever asked for a window, and a caller that waits for a program is
    // entitled to its answer.
    match Command::new(&program)
        .current_dir(&at)
        .env(EXE_ENV, &here)
        .args([language_arg(language), directory_arg(&at)])
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

/// The directory a window is to open onto: the Workspace holding `cwd`, or `cwd` itself.
///
/// The work is the Workspace, so opening onto a directory inside one would be opening onto part of the
/// work: the tree is rooted at the Workspace's own directory, and the Layouts, the ownership and the
/// hiding the browser reads are all read against it. A run in a Vault, or in no place the program
/// knows, has nothing wider to open onto, and opens onto where it was made.
fn opening_at(cwd: &Path) -> PathBuf {
    Workspace::locate(cwd).map_or_else(|| cwd.to_path_buf(), |held| held.get_root().to_path_buf())
}

/// The argument that hands over the language this run speaks.
fn language_arg(language: &ResLanguage) -> String {
    format!("{LANGUAGE_ARG}{}", language.as_str())
}

/// The argument that hands over the directory a window is to open onto.
fn directory_arg(at: &Path) -> String {
    format!("{DIRECTORY_ARG}{}", at.display())
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::opening_at;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-desktop-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_run_inside_a_workspace_opens_onto_its_root() {
        let root = scratch("inside");
        let under = root.join("art").join("scenes");

        // The data directory is what makes a directory a Workspace, and one made here is enough to be
        // found from under it: what is looked up is the Workspace, not what it holds.
        fs::create_dir_all(root.join(".rola")).unwrap();
        fs::create_dir_all(&under).unwrap();

        assert_eq!(opening_at(&under), root);
        assert_eq!(opening_at(&root), root);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_run_no_workspace_holds_opens_where_it_was_made() {
        let dir = scratch("outside");

        assert_eq!(opening_at(&dir), dir);

        let _ = fs::remove_dir_all(&dir);
    }
}

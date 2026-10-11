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
    Grouped, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, renderer, routeify,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg},
    res::{ResCurrentDir, ResExitCode},
};
use rorolala_cli_setups::ResLanguage;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;
use rust_i18n::t;

use crate::Next;
use crate::complete::{filling_flag, offer, strip_written, typing_flag};
use crate::exit_codes::{
    EC_ERR_DESKTOP_ENDED, EC_ERR_DESKTOP_LAUNCH_FAILED, EC_ERR_DESKTOP_NOT_FOUND,
    EC_ERR_DESKTOP_WINDOW_STYLE, EC_HELP,
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

/// What the style the window's own frame is drawn in is handed over under.
///
/// The three names are the Desktop program's own — `Windows`, `Mac`, `TillingWM` — and what a run writes is
/// one of the three lower-case ones: what a person types and what two programs agree on need not be spelled
/// the same way, and the one that is handed over is the one the program's own code names.
const STYLE_ARG: &str = "-WindowStyle:";

/// How the Desktop program is to draw a window's own frame.
///
/// The three are the three ways a frame is drawn there, and a run naming one is asking for that way rather
/// than for the machine's own answer (see [`ARG_WINDOW_STYLE`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowStyle {
    /// Three buttons at the right: minimize, maximize and close.
    Windows,
    /// Three coloured lights at the left.
    Mac,
    /// No buttons: the window manager outside is the frame.
    TillingWm,
}

impl WindowStyle {
    /// The style a written name is, or nothing when it names none of the three.
    ///
    /// Read without regard to case, because the two spellings one name has — this one and the Desktop
    /// program's — are one name to whoever wrote it.
    fn named(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "windows" => Some(Self::Windows),
            "mac" => Some(Self::Mac),
            "tillingwm" => Some(Self::TillingWm),
            _ => None,
        }
    }

    /// The name the Desktop program reads this style by.
    fn argument(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::Mac => "Mac",
            Self::TillingWm => "TillingWM",
        }
    }
}

/// The styles a run may ask for, as it writes them.
const WINDOW_STYLES: [&str; 3] = ["windows", "mac", "tillingwm"];

/// The style the window's own frame is to be drawn in.
///
/// Absent, the Desktop program draws the frame its own machine asks for, which is what a run that says
/// nothing about it gets: the window is opened where it is opened from, and how a frame is drawn there is
/// the machine's answer rather than a caller's.
const ARG_WINDOW_STYLE: PickerArg<'static, Option<String>> = arg![window_style: Option<String>];

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
/// The command names nothing at all, so there is no filename to offer; what it can be told is how to draw
/// the window's frame, and there are three answers to that.
#[completion(EntryDesktop)]
pub fn complete_desktop(ctx: ShellContext) -> Suggest {
    if filling_flag(&ctx, &ARG_WINDOW_STYLE) {
        return offer(&ctx, WINDOW_STYLES);
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_WINDOW_STYLE: t!("desktop.complete.window_style"),
            },
        );
    }

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
/// `--window-style` chooses how the window's own frame is drawn — `windows` for the three buttons at the
/// right, `mac` for the three lights at the left, `tillingwm` for no buttons at all. Absent, the Desktop
/// program draws the frame the machine it is opened on asks for.
///
/// # Errors
///
/// Renders [`ErrorDesktopWindowStyle`] when `--window-style` names none of the three, [`ErrorDesktopNotFound`]
/// when there is no Desktop program beside this one, and [`ErrorDesktopFailed`] when there is one but it
/// would not start.
#[command(node = "desktop", entry = EntryDesktop)]
pub fn desktop(args: EntryDesktop) -> Next {
    let written: Option<String> = match args.pick(&ARG_WINDOW_STYLE).to_result() {
        Ok(written) => written,
        Err(next) => return next,
    };

    // A style the run does not know is refused here rather than handed over: the Desktop program leaves a
    // name it does not know to the machine, so a run that wrote one wrong would open a window drawn one way
    // while being asked for another.
    let style = match written {
        None => None,
        Some(name) => match WindowStyle::named(&name) {
            Some(style) => Some(style),
            None => return ErrorDesktopWindowStyle { written: name }.into(),
        },
    };

    StateDesktop { style }.into()
}

/// The state a run that opens the Desktop program starts in.
///
/// It names nothing of where the program is: that is where this one was run from, so the whole of what is
/// told of the place is already in where that is. What is left is how the window is to be drawn, which is
/// the machine's own answer until a run says otherwise.
#[derive(Grouped, Clone, Copy)]
pub struct StateDesktop {
    /// The style the frame is to be drawn in, or nothing for the machine's own.
    style: Option<WindowStyle>,
}

#[chain(routeify)]
pub fn handle_desktop(state: StateDesktop, language: &ResLanguage, cwd: &ResCurrentDir) -> Next {
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
    let mut told = vec![language_arg(language), directory_arg(&at)];

    if let Some(style) = state.style {
        told.push(style_arg(style));
    }

    match Command::new(&program)
        .current_dir(&at)
        .env(EXE_ENV, &here)
        .args(told)
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

/// The argument that hands over the style the window's own frame is to be drawn in.
fn style_arg(style: WindowStyle) -> String {
    format!("{STYLE_ARG}{}", style.argument())
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

/// Error: the style a run asked for is not one of the three.
#[derive(Grouped)]
pub struct ErrorDesktopWindowStyle {
    /// What was written after the flag.
    written: String,
}

impl Failure for ErrorDesktopWindowStyle {
    fn name(&self) -> &'static str {
        "error_desktop_window_style"
    }

    fn reason(&self) -> String {
        t!("desktop.err_window_style", written = self.written)
            .trim()
            .to_string()
    }
}

failure!(ErrorDesktopWindowStyle);

#[renderer(buffer)]
pub fn render_error_desktop_window_style(error: ErrorDesktopWindowStyle, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("desktop.err_window_style_help").trim()));
    ec.exit_code = EC_ERR_DESKTOP_WINDOW_STYLE;
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

    use super::{WindowStyle, opening_at};

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

    #[test]
    fn every_style_is_read_by_the_name_a_run_writes() {
        assert_eq!(WindowStyle::named("windows"), Some(WindowStyle::Windows));
        assert_eq!(WindowStyle::named("mac"), Some(WindowStyle::Mac));
        assert_eq!(WindowStyle::named("tillingwm"), Some(WindowStyle::TillingWm));

        // The two programs spell the names their own way, and one name is what both of them mean.
        assert_eq!(WindowStyle::named("TillingWM"), Some(WindowStyle::TillingWm));
        assert_eq!(WindowStyle::named("MAC"), Some(WindowStyle::Mac));
    }

    #[test]
    fn a_name_that_is_not_one_of_the_three_names_no_style() {
        assert_eq!(WindowStyle::named(""), None);
        assert_eq!(WindowStyle::named("gnome"), None);
        assert_eq!(WindowStyle::named("tiling"), None);
    }

    #[test]
    fn a_style_is_handed_over_by_the_name_the_program_reads() {
        assert_eq!(WindowStyle::Windows.argument(), "Windows");
        assert_eq!(WindowStyle::Mac.argument(), "Mac");
        assert_eq!(WindowStyle::TillingWm.argument(), "TillingWM");
    }
}

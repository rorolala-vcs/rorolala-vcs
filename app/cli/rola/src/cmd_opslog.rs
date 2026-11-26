//! The `rola opslog` command: the operations this Workspace recorded, newest first.
//!
//! Each operation is one file under `.rola/cache/log/`, written by a run that changed the Layout or
//! the files, and this lists them in the order `rola undo` takes them off. What a group holds is not
//! printed: a reader who wants the changes asks `rola undo` to move them, and one who asks about a
//! file's history asks `rola tweak`.

use librorolala::write::Journal;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd, warn_line};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{EC_ERR_TWEAK, EC_HELP};
use crate::failure::failure;

#[help(buffer)]
pub fn help_opslog(_: EntryOpslog, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("opslog.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryOpslog)]
pub fn desc_opslog() -> Description {
    t!("opslog.description").to_string().into()
}

/// Completes what `rola opslog` can be given next.
///
/// The command names no argument of its own, so there is nothing to answer.
#[completion(EntryOpslog)]
pub fn complete_opslog(_ctx: ShellContext) -> Suggest {
    suggest!()
}

/// Lists the operations recorded in this Workspace, newest first
///
/// The list is the stack `rola undo` reads: one line per recorded operation, numbered from the
/// newest, with the command that made it, when it ended, and how many changes it holds.
///
/// A log file that does not read is skipped with a warning rather than failing the list, since the
/// others are still worth showing.
///
/// # Errors
///
/// Renders the run-not-in-a-workspace failure when the run is not inside a Workspace, and
/// [`ErrorOpslogRead`] when the log cannot be listed.
#[command(node = "opslog", entry = EntryOpslog)]
pub fn opslog(args: EntryOpslog) -> Next {
    // The command names nothing, so a word it was given is one it does not have.
    let picked = args.pick(&UNMATCHED).to_result();
    let unmatched = match picked {
        Ok(unmatched) => unmatched,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "opslog") {
        return next;
    }

    StateOpslog.into()
}

/// The state `rola opslog` starts in: it names nothing.
#[derive(Grouped)]
pub struct StateOpslog;

#[chain(routeify)]
pub fn handle_opslog(_: StateOpslog, workspace: &mut LazyRes<ResWorkspace>) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to read.
    let held = workspace.get_ref().as_ref().unwrap();

    let files = match Journal::files(held) {
        Ok(files) => files,
        Err(error) => return ErrorOpslogRead::new(error.to_string()).into(),
    };

    // Which files read is this run's to know; a file that does not read is skipped by name, since
    // the others are still worth listing.
    let mut items = Vec::new();
    for path in files.iter().rev() {
        match Journal::read(path) {
            Ok(group) => items.push(Operation {
                command: group.command,
                time: group.time,
                count: group.effects.len(),
            }),
            Err(_) => eprintln!(
                "{}",
                warn_line!(t!("opslog.warn_skipped", file = path.display().to_string()).trim())
            ),
        }
    }

    ResultOpslog { items }.into()
}

/// One operation as the list draws it.
#[derive(Debug, Clone)]
pub struct Operation {
    /// The command that made the changes.
    pub command: String,
    /// When it ended, in UTC.
    pub time: String,
    /// How many changes it holds.
    pub count: usize,
}

/// Result: the operations recorded in this Workspace, newest first.
#[derive(Grouped)]
pub struct ResultOpslog {
    /// One operation a line, newest first.
    items: Vec<Operation>,
}

#[renderer(buffer)]
pub fn render_result_opslog(result: ResultOpslog) {
    if result.items.is_empty() {
        r_println!("{}", t!("opslog.empty").trim());
    } else {
        r_println!("{}", t!("opslog.header").trim());

        for (index, item) in result.items.iter().enumerate() {
            r_println!(
                "{}",
                t!(
                    "opslog.item",
                    number = index + 1,
                    command = item.command,
                    time = item.time,
                    count = item.count
                )
            );
        }
    }
}

/// Error: the operation log could not be read.
#[derive(Grouped)]
pub struct ErrorOpslogRead {
    /// What the read failed with.
    reason: String,
}

impl ErrorOpslogRead {
    /// The failure `reason` describes.
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl Failure for ErrorOpslogRead {
    fn name(&self) -> &'static str {
        "error_opslog_read"
    }

    fn reason(&self) -> String {
        t!("opslog.err_read", reason = self.reason)
            .trim()
            .to_string()
    }
}

failure!(ErrorOpslogRead);

#[renderer(buffer)]
pub fn render_error_opslog_read(error: ErrorOpslogRead, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("opslog.err_read_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

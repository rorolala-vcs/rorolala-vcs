//! The `rola undo` command: put back what the last recorded operations changed.
//!
//! A run pops the newest operations off the stack `rola opslog` lists and reverses them, newest
//! first. How the tree stands against each operation, and what reversing one does, is
//! [`librorolala::write::undo`](mod@librorolala::write::undo)'s: what is here is the shape of the
//! command, the question it asks when an operation stands only in part, the words it prints and the
//! exit code it ends on.

// The confirmation is the resource the injection hands the handler and not something an undo owns:
// taking a copy would read a state of its own, so it is taken by reference, which is what the
// injection gives it and what every other handler that asks a question does.
#![allow(clippy::trivially_copy_pass_by_ref)]

use librorolala::write::undo::{self, UndoGroup, UndoOptions, UndoRefusal};
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    confirm::YesConfirm,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::{ResConfirm, ResExitCode},
};
use rorolala_cli_setups::{ResForce, ResStorageSet, ResVCSIndex, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_TWEAK, EC_HELP};
use crate::failure::failure;
use crate::layout::ErrorLayoutNoCurrent;

/// How many operations to undo, newest first.
const COUNT: PickerArg<'static, Option<String>> = arg![Option<String>];

/// Move only the record back, leaving the work on disk as it is.
const ARG_SOFT: PickerArg<'static, Flag> = arg![soft: Flag];

#[help(buffer)]
pub fn help_undo(_: EntryUndo, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("undo.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryUndo)]
pub fn desc_undo() -> Description {
    t!("undo.description").to_string().into()
}

/// Completes what `rola undo` can be given next.
///
/// The count is a number this program cannot know, and the only word worth offering is the flag.
#[completion(EntryUndo)]
pub fn complete_undo(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_SOFT: t!("undo.complete.soft"),
            },
        );
    }

    suggest!()
}

/// Undoes the last recorded operations, newest first
///
/// `COUNT` is how many recorded operations to undo, from the newest, and defaults to one. An
/// operation whose every change still stands is undone at once; one that stands only in part asks
/// whether to go on, and one that stands nowhere stops the run and leaves the stack below it.
///
/// `--force` continues past a file that changed after the operation and writes over it, and
/// `--soft` moves only the Layout back and leaves every file as it is.
///
/// # Errors
///
/// Renders [`ErrorUndoNoLog`] when the Workspace records nothing, [`ErrorUndoCount`] when `COUNT` is
/// not a number from one to the number of recorded operations, [`ErrorUndoFlags`] when `--force` and
/// `--soft` are given together, [`ErrorUndoChanged`] when a file changed after the operation,
/// [`ErrorUndoAbsent`] when a file the operation left is gone, [`ErrorUndoFailed`] when the Layout or
/// the store refuses, [`ErrorLayoutShouldInWorkspace`](crate::layout::ErrorLayoutShouldInWorkspace)
/// when the run is not inside a Workspace, and
/// [`ErrorLayoutNoCurrent`] when the Workspace has no Layout to move back.
#[command(node = "undo", entry = EntryUndo)]
pub fn undo(args: EntryUndo, force: &ResForce) -> Next {
    let picked = args.pick(COUNT).pick(ARG_SOFT).pick(&UNMATCHED).to_result();
    let (count, soft, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "undo") {
        return next;
    }

    StateUndo {
        count,
        soft: matches!(soft, Flag::Active),
        force: **force,
    }
    .into()
}

/// The state an undo starts in.
#[derive(Grouped)]
pub struct StateUndo {
    /// How many operations were named, as they were written.
    count: Option<String>,
    /// Whether only the record is moved back.
    soft: bool,
    /// Whether a file that changed is written over.
    force: bool,
}

#[chain(routeify)]
pub fn handle_undo(
    state: StateUndo,
    workspace: &mut LazyRes<ResWorkspace>,
    storage: &mut LazyRes<ResStorageSet>,
    index: &mut LazyRes<ResVCSIndex>,
    confirm: &ResConfirm,
) -> Next {
    let StateUndo { count, soft, force } = state;

    // The two ask for different undos: one writes over a file, the other writes none at all. Refused
    // before anything is read, so a run that asked for both is told that rather than about its
    // Workspace or its flags.
    if force && soft {
        return ErrorUndoFlags.into();
    }

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to read.
    let held = workspace.get_ref().as_ref().unwrap();

    let layout = match held.layout_if_there() {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutNoCurrent.into(),
        Err(error) => return failed("undo", error.to_string()),
    };

    let options = UndoOptions { count, soft, force };
    let mut ask =
        |_: &UndoGroup| confirm.ask::<YesConfirm>(&format!("{} [y/n] ", t!("undo.partial").trim()));

    match undo::undo(
        held,
        &layout,
        index.get_ref().as_ref(),
        storage.get_ref().storage(),
        &options,
        &mut ask,
    ) {
        Ok(outcome) => settled(outcome.undone),
        Err(refusal) => refused(refusal),
    }
}

/// What a run that walked the stack answers with.
fn settled(mut undone: Vec<String>) -> Next {
    match undone.len() {
        0 => ResultUndo::none().into(),
        1 => ResultUndoOne {
            command: undone.remove(0),
        }
        .into(),
        count => ResultUndo {
            count,
            commands: undone.join(", "),
        }
        .into(),
    }
}

/// What a refused undo is told as.
fn refused(refusal: UndoRefusal) -> Next {
    match refusal {
        UndoRefusal::NoLog => ErrorUndoNoLog.into(),
        UndoRefusal::Count { written, available } => ErrorUndoCount {
            count: written,
            available,
        }
        .into(),
        UndoRefusal::Flags => ErrorUndoFlags.into(),
        UndoRefusal::Changed { path } => ErrorUndoChanged { path }.into(),
        UndoRefusal::Absent { path } => ErrorUndoAbsent { path }.into(),
        UndoRefusal::Failed { command, reason } => ErrorUndoFailed { command, reason }.into(),
    }
}

/// The failure a read that could not be made is told as.
fn failed(command: &str, reason: String) -> Next {
    ErrorUndoFailed {
        command: command.to_owned(),
        reason,
    }
    .into()
}

/// Result: one operation was undone.
#[derive(Grouped)]
pub struct ResultUndoOne {
    /// The command that made it.
    command: String,
}

#[renderer(buffer)]
pub fn render_result_undo_one(result: ResultUndoOne, ec: &mut ResExitCode) {
    r_println!("{}", t!("undo.result_one", command = result.command).trim());
    let _ = ec;
}

/// Result: several operations were undone, or none was.
#[derive(Grouped)]
pub struct ResultUndo {
    /// How many were undone.
    count: usize,
    /// The commands that made them, newest first.
    commands: String,
}

impl ResultUndo {
    /// The result of a run that found nothing it could undo.
    fn none() -> Self {
        Self {
            count: 0,
            commands: String::new(),
        }
    }
}

#[renderer(buffer)]
pub fn render_result_undo(result: ResultUndo, ec: &mut ResExitCode) {
    // A run that found nothing it could undo leaves the stack as it was and says nothing: the
    // operations are still there for `rola opslog` to name.
    if result.count > 0 {
        r_println!(
            "{}",
            t!(
                "undo.result",
                count = result.count,
                commands = result.commands
            )
            .trim()
        );
    }

    let _ = ec;
}

/// Error: the Workspace records no operation.
#[derive(Grouped)]
pub struct ErrorUndoNoLog;

impl Failure for ErrorUndoNoLog {
    fn name(&self) -> &'static str {
        "error_undo_no_log"
    }

    fn reason(&self) -> String {
        t!("undo.err_no_log").trim().to_string()
    }
}

failure!(ErrorUndoNoLog);

#[renderer(buffer)]
pub fn render_error_undo_no_log(_: ErrorUndoNoLog, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("undo.err_no_log").trim()));
    r_eprintln!("{}", help_line!(t!("undo.err_no_log_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the count names no operation.
#[derive(Grouped)]
pub struct ErrorUndoCount {
    /// The number as it was written.
    count: String,
    /// How many operations are recorded.
    available: usize,
}

impl Failure for ErrorUndoCount {
    fn name(&self) -> &'static str {
        "error_undo_count"
    }

    fn reason(&self) -> String {
        t!(
            "undo.err_count",
            count = self.count,
            available = self.available
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorUndoCount);

#[renderer(buffer)]
pub fn render_error_undo_count(error: ErrorUndoCount, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_count_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a file changed after the operation was recorded.
#[derive(Grouped)]
pub struct ErrorUndoChanged {
    /// The file.
    path: String,
}

impl Failure for ErrorUndoChanged {
    fn name(&self) -> &'static str {
        "error_undo_changed"
    }

    fn reason(&self) -> String {
        t!("undo.err_changed", path = self.path).trim().to_string()
    }
}

failure!(ErrorUndoChanged);

#[renderer(buffer)]
pub fn render_error_undo_changed(error: ErrorUndoChanged, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_changed_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a file the operation left is gone.
#[derive(Grouped)]
pub struct ErrorUndoAbsent {
    /// The file.
    path: String,
}

impl Failure for ErrorUndoAbsent {
    fn name(&self) -> &'static str {
        "error_undo_absent"
    }

    fn reason(&self) -> String {
        t!("undo.err_absent", path = self.path).trim().to_string()
    }
}

failure!(ErrorUndoAbsent);

#[renderer(buffer)]
pub fn render_error_undo_absent(error: ErrorUndoAbsent, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_absent_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: `--force` and `--soft` were given together.
#[derive(Grouped)]
pub struct ErrorUndoFlags;

impl Failure for ErrorUndoFlags {
    fn name(&self) -> &'static str {
        "error_undo_flags"
    }

    fn reason(&self) -> String {
        t!("undo.err_flags").trim().to_string()
    }
}

failure!(ErrorUndoFlags);

#[renderer(buffer)]
pub fn render_error_undo_flags(_: ErrorUndoFlags, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("undo.err_flags").trim()));
    r_eprintln!("{}", help_line!(t!("undo.err_flags_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the undo could not be carried out.
#[derive(Grouped)]
pub struct ErrorUndoFailed {
    /// The command whose operation failed.
    command: String,
    /// What failed.
    reason: String,
}

impl Failure for ErrorUndoFailed {
    fn name(&self) -> &'static str {
        "error_undo_failed"
    }

    fn reason(&self) -> String {
        t!(
            "undo.err_failed",
            command = self.command,
            reason = self.reason
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorUndoFailed);

#[renderer(buffer)]
pub fn render_error_undo_failed(error: ErrorUndoFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

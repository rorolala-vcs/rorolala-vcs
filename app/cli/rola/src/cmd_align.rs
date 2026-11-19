//! The `rola align` command: bringing a tree and the Layout it works from back into agreement.
//!
//! A reading of the tree ([`rola layout tree-diff`](crate::layout::cmd_layout_tree_diff)) says how
//! the two stand apart, and this is what acts on that: a path the Layout names and the tree no
//! longer holds is confirmed gone, and a path that moved is confirmed moved. Between the two, a
//! move the reading only guessed at can be asserted, and one it guessed wrong can be taken back.
//!
//! `rola track` refuses to run while the Layout names a path the tree does not hold, since recording
//! works on a tree that is settled; this is what settles it. A move, on the other hand, `track`
//! confirms on its own, naming a path that moved records it where it is now.
//!
//! The order the settling is done in is the library's, see [`librorolala::write::align`]: what
//! stays here is choosing the mode the flags name, reading the run's resources, and saying the
//! result in the command's own words.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped. The progress
// setting is one of those resources and is taken by reference, which is what the injection gives it.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use librorolala::status::tracked_vault;
use librorolala::write::WriteError;
use librorolala::write::align::{
    Carry, Did, Item, Messages, Mode, MoveKind, Options, Outcome, Refusal, settle,
};
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResConcurrency, ResCurrentRemoteVault, ResOffline, ResProgressSetting, ResStorageSet,
    ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd, warn_line};
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{filling_flag, positional, strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_ALIGN, EC_ERR_ALIGN_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutExists, ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutShouldInWorkspace, chosen,
};
use crate::progress::Reporting;

/// Confirm that a path the Layout names but the tree does not hold is gone.
const ARG_DELETE: PickerArg<'static, Flag> = arg![delete: Flag];

/// Confirm that a path that moved has moved.
const ARG_RENAME: PickerArg<'static, Flag> = arg![rename: Flag];

/// Take back a move the reading guessed at, leaving a loss and a gain.
const ARG_BREAK: PickerArg<'static, Flag> = arg![break: Flag];

/// Assert that the named path moved to `FILE`, so the next reading sees it as a move.
const ARG_MOVE_TO: PickerArg<'static, Option<String>> = arg![move: Option<String>];

/// Put a moved file back where the Layout names it.
const ARG_RESTORE_MOVE: PickerArg<'static, Flag> = arg![restore_move: Flag];

/// Put a changed file back to the version the Layout names.
const ARG_RESTORE_MODIFY: PickerArg<'static, Flag> = arg![restore_modify: Flag];

/// Put a file inside a tree back to what the tree has staged for it.
const ARG_RESTORE_STAGED: PickerArg<'static, Flag> = arg![restore_staged: Flag];

/// Put a deleted file back, taking the version the Layout names out of the store.
const ARG_RESTORE_DELETE: PickerArg<'static, Flag> = arg![restore_delete: Flag];

/// Put a path back to what the Layout names, whichever of the three it needs.
const ARG_RESTORE: PickerArg<'static, Flag> = arg![restore: Flag];

/// Put a move's file back where the Layout names it and as the Layout names it, undoing the
/// content as well as the place.
const ARG_COMPLETELY: PickerArg<'static, Flag> = arg![completely: Flag];

#[help(buffer)]
pub fn help_align(_: EntryAlign, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("align.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryAlign)]
pub fn desc_align() -> Description {
    t!("align.description").to_string().into()
}

/// Completes what `rola align` can be given next.
///
/// The path is the tree's, so the filesystem answers it; `--move` names where a file went, which is
/// a path too. Everything else the command takes is one of its modes, and those are the flags put
/// to a word that starts one.
#[completion(EntryAlign)]
pub fn complete_align(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_DELETE: t!("align.complete.delete"),
                ARG_RENAME: t!("align.complete.rename"),
                ARG_BREAK: t!("align.complete.break"),
                ARG_MOVE_TO: t!("align.complete.move"),
                ARG_RESTORE_MOVE: t!("align.complete.restore_move"),
                ARG_RESTORE_MODIFY: t!("align.complete.restore_modify"),
                ARG_RESTORE_STAGED: t!("align.complete.restore_staged"),
                ARG_RESTORE_DELETE: t!("align.complete.restore_delete"),
                ARG_RESTORE: t!("align.complete.restore"),
                ARG_COMPLETELY: t!("align.complete.completely"),
                crate::ignore::ARG_INCLUDE_IGNORED: t!("ignore.complete.include_ignored"),
            },
        );
    }

    if filling_flag(&ctx, &ARG_MOVE_TO) || positional(&ctx, "align") == 0 {
        Suggest::file_comp()
    } else {
        suggest!()
    }
}

/// Brings the tree and the Layout back into agreement about a path
///
/// `PATH` is read where the run was made, as a shell would read it: `.` is the directory the run is
/// in and everything under it, and a name that lands on a directory selects everything under it.
/// A single path need not be there, a path the Layout names and the tree does not is exactly what
/// `--delete` acts on.
///
/// Exactly one of the modes is given. `--delete` confirms that a lost path is gone. `--rename`
/// confirms a move the reading found, whether `PATH` is where it was or where it went. `--break`
/// takes a move back, so the two paths become a loss and a gain again; a move of two identical files
/// is a fact and is refused. `--move TO` asserts that `PATH` moved to `TO`, whether a file or a tree,
/// which is for a move the reading could not guess at; the next reading then sees it as one to
/// confirm.
///
/// `--restore-move` puts a moved file back where the Layout names it, `--restore-modify` writes the
/// version the Layout names back over a changed file, `--restore-staged` writes what a tree has staged
/// back over a changed file inside it, and `--restore-delete` takes a deleted file back out of the
/// store and puts it where the Layout names it; `--restore` does whichever of the three the path needs.
/// A move that was edited as well is put back first, so it is left changed at its old path for
/// `--restore-modify` to settle.
///
/// A name that lands on a directory, `.` always does, and `--restore` are what act on many paths
/// at once: every path under it is put back by what it needs, one line each, and nothing under it
/// is a failure. A move is settled first, so content is only put back once the file is where the
/// Layout names it; a path the tree holds and no Layout names is left where it lies. Other modes
/// act on one path only. `--completely` says to put a move back as well as where it was, and means
/// something only for a move being put back.
///
/// # Errors
///
/// Renders [`ErrorAlignArgument`] when no path or not exactly one mode was given,
/// [`ErrorAlignCompletely`] when `--completely` was given for something it cannot mean,
/// [`ErrorAlignNotLost`] or [`ErrorAlignNoRename`] when the path is not what the mode acts on,
/// [`ErrorAlignNotModified`] or [`ErrorAlignNotRestorable`] when there is nothing to put back,
/// [`ErrorAlignNotStaged`] when `--restore-staged` names a path a tree has not staged,
/// [`ErrorAlignRestoreExists`] when putting a file back would land on one already there,
/// [`ErrorAlignIgnored`] when the path named is one the ignore rules govern,
/// [`ErrorAlignRealMove`] when `--break` names a move of identical files, [`ErrorAlignMove`] when
/// `--move` names somewhere that is not a fresh file or tree, and [`ErrorAlignFailed`] otherwise.
#[command(node = "align", entry = EntryAlign)]
pub fn align(args: EntryAlign) -> Next {
    let picked = args
        .pick(ARG_DELETE)
        .pick(ARG_RENAME)
        .pick(ARG_BREAK)
        .pick(ARG_MOVE_TO)
        .pick(ARG_RESTORE_MOVE)
        .pick(ARG_RESTORE_MODIFY)
        .pick(ARG_RESTORE_STAGED)
        .pick(ARG_RESTORE_DELETE)
        .pick(ARG_RESTORE)
        .pick(ARG_COMPLETELY)
        .pick(crate::ignore::ARG_INCLUDE_IGNORED)
        .pick(arg![Option<String>])
        .pick(&UNMATCHED)
        .to_result();
    let (
        delete,
        rename,
        break_,
        move_to,
        restore_move,
        restore_modify,
        restore_staged,
        restore_delete,
        restore,
        completely,
        include_ignored,
        path,
        unmatched,
    ) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "align") {
        return next;
    }

    let Some(path) = path else {
        return ErrorAlignArgument.into();
    };

    let completely = matches!(completely, Flag::Active);

    let mut modes = Vec::new();
    if matches!(delete, Flag::Active) {
        modes.push(Mode::Delete);
    }
    if matches!(rename, Flag::Active) {
        modes.push(Mode::Rename);
    }
    if matches!(break_, Flag::Active) {
        modes.push(Mode::Break);
    }
    if let Some(target) = move_to {
        modes.push(Mode::Move(target));
    }
    if matches!(restore_move, Flag::Active) {
        modes.push(Mode::RestoreMove);
    }
    if matches!(restore_modify, Flag::Active) {
        modes.push(Mode::RestoreModify);
    }
    if matches!(restore_staged, Flag::Active) {
        modes.push(Mode::RestoreStaged);
    }
    if matches!(restore_delete, Flag::Active) {
        modes.push(Mode::RestoreDelete);
    }
    if matches!(restore, Flag::Active) {
        modes.push(Mode::Restore);
    }

    if modes.len() != 1 {
        return ErrorAlignArgument.into();
    }

    // UNWRAP: the list was just checked to hold exactly one mode.
    let mode = modes.pop().unwrap();

    // A move is the one thing with both a place and content of its own to put back, so it is the
    // one thing `--completely` can mean.
    if completely && !matches!(mode, Mode::Restore | Mode::RestoreMove) {
        return ErrorAlignCompletely.into();
    }

    StateAlign {
        path,
        mode,
        completely,
        include_ignored: matches!(include_ignored, Flag::Active),
    }
    .into()
}

/// The state an alignment starts in.
#[derive(Grouped)]
pub struct StateAlign {
    /// The path to act on, as it was written.
    path: String,
    /// What is to be done with it.
    mode: Mode,
    /// Whether a move is to be put back as well as returned to where it was.
    completely: bool,
    /// Whether an entry the ignore rules govern may be selected anyway.
    include_ignored: bool,
}

#[chain]
pub fn handle_align(
    state: StateAlign,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResStorageSet>,
    index: &mut LazyRes<ResVCSIndex>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: &ResOffline,
    concurrency: &ResConcurrency,
    progress: &ResProgressSetting,
) -> Next {
    let StateAlign {
        path,
        mode,
        completely,
        include_ignored,
    } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    // Read after the Layout, since asking the Workspace for its own Layout is a borrow of it: what
    // is held from here on is the Workspace itself, and the resource is not asked again.
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return align_failed(error),
    };

    // Content a restore writes comes out of the store, and the store may not hold it: what is
    // missing is asked of the Vaults the Workspace has bound. A run that cannot say which account it
    // acts as has none to ask, and then restores only what is here.
    let account = current
        .get_ref()
        .must_bind()
        .ok()
        .and_then(|name| account_named(&name, Some(held), None).ok());
    let tracked = tracked_vault(held);
    let primary = fetch::primary(remote.get_ref(), tracked.as_deref());
    let candidates = fetch::candidates(remote.get_ref(), primary);
    let rules = crate::ignore::rules(held, include_ignored);
    let messages = messages();

    // The watcher is started before the first piece a restore may ask the Vaults for: a run that
    // asks for nothing is over before it draws anything, and a run that asks is watched from the
    // first connection rather than from the moment the asking is over.
    //
    // The way to say anything is scoped to the run itself: the reader stops when the last of them is
    // gone, so a clone that outlived the run would have the run waiting on a reader that is waiting
    // on the run. A sync holds its progress the same way.
    let reporting = Reporting::start(*progress);
    let cancellation = crate::cancel::on_ctrl_c();
    let settled = {
        let watching = reporting.progress();
        let carry = Carry {
            workspace: held,
            layout: &layout,
            cwd: &cwd,
            rules: &rules,
            storage: storage.get_ref().storage(),
            index: index.get_ref().as_ref(),
            account: account.as_ref(),
            candidates: &candidates,
            offline: **offline,
            concurrency: **concurrency,
            progress: &watching,
            messages: &messages,
        };

        settle(
            &carry,
            &path,
            &mode,
            Options {
                completely,
                include_ignored,
            },
            cancellation,
        )
    };

    // What came from the Vaults is said before the watcher is let go: a reader told nothing while
    // the run was asking would be told after the bars it was watching had gone. A run that brought
    // nothing says nothing.
    report_brought(&settled.took);
    reporting.finish();

    align_next(settled.outcome)
}

/// The words the library says a failure in, which have no category of its own.
fn messages() -> Messages {
    Messages {
        not_recorded: t!("align.err_not_recorded").trim().to_owned(),
        unsupported: t!("align.err_tree_unsupported").trim().to_owned(),
    }
}

/// Says what the Vaults handed over while the run worked, in one line.
///
/// It is the line a sync says about the content it brought, in the same words: the exchange that
/// brought it is the library's, but what a reader is told is the command's to say.
fn report_brought(took: &[(String, usize)]) {
    if took.is_empty() {
        return;
    }

    let count: usize = took.iter().map(|(_, count)| *count).sum();
    let vaults: Vec<&str> = took.iter().map(|(name, _)| name.as_str()).collect();

    eprintln!(
        "{}",
        warn_line!(t!("fetch.brought", count = count, vaults = vaults.join(", ")).trim())
    );
}

/// What the run came to, as the command draws it.
fn align_next(outcome: Result<Outcome, WriteError>) -> Next {
    match outcome {
        Ok(Outcome::Paths(items)) => ResultAlign { items }.into(),
        Ok(Outcome::Tree { path }) => ResultAlignTree { path }.into(),
        Ok(Outcome::TreeFile { path, staged }) => ResultAlignTreeFile { path, staged }.into(),
        Ok(Outcome::Refused(refusal)) => refusal_state(refusal),
        Err(error) => align_write_error(error),
    }
}

/// Turns one refusal into what the run is told.
fn refusal_state(refusal: Refusal) -> Next {
    match refusal {
        Refusal::Argument => ErrorAlignArgument.into(),
        Refusal::Ignored { path } => ErrorAlignIgnored { path }.into(),
        Refusal::NotLost { path } => ErrorAlignNotLost { path }.into(),
        Refusal::NotModified { path } => ErrorAlignNotModified { path }.into(),
        Refusal::NotStaged { path } => ErrorAlignNotStaged { path }.into(),
        Refusal::NotRestorable { path } => ErrorAlignNotRestorable { path }.into(),
        Refusal::RestoreExists { path } => ErrorAlignRestoreExists { path }.into(),
        Refusal::NoRename { path } => ErrorAlignNoRename { path }.into(),
        Refusal::RealMove { from, to } => ErrorAlignRealMove { from, to }.into(),
        Refusal::Move { path, kind } => ErrorAlignMove { path, kind }.into(),
        Refusal::TreeUnsupported => ErrorAlignTreeUnsupported.into(),
        // A change to the Layout is not this command's failure to word: it is rendered the way every
        // other command renders it, with the exit code a caller branching on a missing or taken name
        // is given.
        Refusal::LayoutExists => ErrorLayoutExists.into(),
        Refusal::LayoutMissing => ErrorLayoutMissing.into(),
        Refusal::LayoutFailed { cause } => ErrorLayoutFailed::new(cause).into(),
        Refusal::Failed { cause } => ErrorAlignFailed { cause }.into(),
    }
}

/// The failure a write the run stopped on is told as.
///
/// A store or an index the run does not have is said in the command's own words, as it always was;
/// everything else is the library's description of what failed, which is what the failure carries.
fn align_write_error(error: WriteError) -> Next {
    let cause = match error {
        WriteError::NoStore => t!("align.err_no_store").trim().to_owned(),
        WriteError::NoIndex => t!("align.err_no_index").trim().to_owned(),
        other => other.to_string(),
    };

    ErrorAlignFailed { cause }.into()
}

/// The failure an alignment that could not be made is answered with.
fn align_failed(cause: impl std::fmt::Display) -> Next {
    ErrorAlignFailed {
        cause: cause.to_string(),
    }
    .into()
}

/// Result: the tree and the Layout were brought back into agreement about paths.
#[derive(Grouped)]
pub struct ResultAlign {
    /// What was done, in the order it was done.
    items: Vec<Item>,
}

#[renderer(buffer)]
pub fn render_result_align(result: ResultAlign) {
    for item in &result.items {
        let said = match item.did {
            Did::Deleted => t!("align.result_deleted", what = item.what),
            Did::Moved => t!("align.result_moved", what = item.what),
            Did::Broke => t!("align.result_broke", what = item.what),
            Did::Asserted => t!("align.result_asserted", what = item.what),
            Did::RestoredMove => t!("align.result_restored_move", what = item.what),
            Did::RestoredCompletely => {
                t!("align.result_restored_completely", what = item.what)
            }
            Did::RestoredModify => t!("align.result_restored_modify", what = item.what),
            Did::RestoredDelete => t!("align.result_restored_delete", what = item.what),
            Did::RestoredMerge => t!("align.result_restored_merge", what = item.what),
            Did::Unmerged => t!("align.result_unmerged", what = item.what),
        };

        r_println!("{}", said.trim());
    }
}

/// Result: one file inside a tree was put back.
#[derive(Grouped)]
pub struct ResultAlignTreeFile {
    /// The file as a person knows it.
    path: String,
    /// Whether it was put back to the staging area rather than the recorded version.
    staged: bool,
}

#[renderer(buffer)]
pub fn render_result_align_tree_file(result: ResultAlignTreeFile, ec: &mut ResExitCode) {
    let key = if result.staged {
        "align.result_tree_file_staged"
    } else {
        "align.result_tree_file"
    };

    r_println!("{}", t!(key, path = result.path).trim());
    let _ = ec;
}

/// Result: the work of a tree was put back.
#[derive(Grouped)]
pub struct ResultAlignTree {
    /// The tree as a person knows it.
    path: String,
}

#[renderer(buffer)]
pub fn render_result_align_tree(result: ResultAlignTree, ec: &mut ResExitCode) {
    r_println!("{}", t!("align.result_tree", path = result.path).trim());
    let _ = ec;
}

/// Error: `rola align` was given no path, or not exactly one mode.
#[derive(Grouped)]
pub struct ErrorAlignArgument;

impl Failure for ErrorAlignArgument {
    fn name(&self) -> &'static str {
        "error_align_argument"
    }

    fn reason(&self) -> String {
        t!("align.err_argument").trim().to_string()
    }
}

failure!(ErrorAlignArgument);

#[renderer(buffer)]
pub fn render_error_align_argument(_: ErrorAlignArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("align.err_argument").trim()));
    r_eprintln!("{}", help_line!(t!("align.err_argument_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: `--completely` was given for something it cannot mean.
#[derive(Grouped)]
pub struct ErrorAlignCompletely;

impl Failure for ErrorAlignCompletely {
    fn name(&self) -> &'static str {
        "error_align_completely"
    }

    fn reason(&self) -> String {
        t!("align.err_completely").trim().to_string()
    }
}

failure!(ErrorAlignCompletely);

#[renderer(buffer)]
pub fn render_error_align_completely(_: ErrorAlignCompletely, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("align.err_completely").trim()));
    r_eprintln!("{}", help_line!(t!("align.err_completely_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path named is not one the Layout names and the tree does not hold.
#[derive(Grouped)]
pub struct ErrorAlignNotLost {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNotLost {
    fn name(&self) -> &'static str {
        "error_align_not_lost"
    }

    fn reason(&self) -> String {
        t!("align.err_not_lost", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNotLost);

#[renderer(buffer)]
pub fn render_error_align_not_lost(error: ErrorAlignNotLost, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_not_lost_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path named is one the ignore rules govern, so it cannot be selected.
#[derive(Grouped)]
pub struct ErrorAlignIgnored {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignIgnored {
    fn name(&self) -> &'static str {
        "error_align_ignored"
    }

    fn reason(&self) -> String {
        t!("align.err_ignored", path = self.path).trim().to_string()
    }
}

failure!(ErrorAlignIgnored);

#[renderer(buffer)]
pub fn render_error_align_ignored(error: ErrorAlignIgnored, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_ignored_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path named is not a side of a move the reading found.
#[derive(Grouped)]
pub struct ErrorAlignNoRename {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNoRename {
    fn name(&self) -> &'static str {
        "error_align_no_rename"
    }

    fn reason(&self) -> String {
        t!("align.err_no_rename", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNoRename);

#[renderer(buffer)]
pub fn render_error_align_no_rename(error: ErrorAlignNoRename, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_no_rename_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path is not a change the Layout names, so there is no version to go back to.
#[derive(Grouped)]
pub struct ErrorAlignNotModified {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNotModified {
    fn name(&self) -> &'static str {
        "error_align_not_modified"
    }

    fn reason(&self) -> String {
        t!("align.err_not_modified", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNotModified);

#[renderer(buffer)]
pub fn render_error_align_not_modified(error: ErrorAlignNotModified, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_not_modified_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: a tree has nothing staged for the path named.
#[derive(Grouped)]
pub struct ErrorAlignNotStaged {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNotStaged {
    fn name(&self) -> &'static str {
        "error_align_not_staged"
    }

    fn reason(&self) -> String {
        t!("align.err_not_staged", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNotStaged);

#[renderer(buffer)]
pub fn render_error_align_not_staged(error: ErrorAlignNotStaged, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_not_staged_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: putting a moved file back would land on one already there.
#[derive(Grouped)]
pub struct ErrorAlignRestoreExists {
    /// Where it would land.
    path: String,
}

impl Failure for ErrorAlignRestoreExists {
    fn name(&self) -> &'static str {
        "error_align_restore_exists"
    }

    fn reason(&self) -> String {
        t!("align.err_restore_exists", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignRestoreExists);

#[renderer(buffer)]
pub fn render_error_align_restore_exists(error: ErrorAlignRestoreExists, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_restore_exists_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path is neither a move nor a change, so there is nothing to put back.
#[derive(Grouped)]
pub struct ErrorAlignNotRestorable {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNotRestorable {
    fn name(&self) -> &'static str {
        "error_align_not_restorable"
    }

    fn reason(&self) -> String {
        t!("align.err_not_restorable", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNotRestorable);

#[renderer(buffer)]
pub fn render_error_align_not_restorable(error: ErrorAlignNotRestorable, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_not_restorable_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: `--break` named a move of two files that hold the same bytes.
#[derive(Grouped)]
pub struct ErrorAlignRealMove {
    /// Where the Layout named it.
    from: String,
    /// Where the tree holds it.
    to: String,
}

impl Failure for ErrorAlignRealMove {
    fn name(&self) -> &'static str {
        "error_align_real_move"
    }

    fn reason(&self) -> String {
        t!("align.err_real_move", from = self.from, to = self.to)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignRealMove);

#[renderer(buffer)]
pub fn render_error_align_real_move(error: ErrorAlignRealMove, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_real_move_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: what `--move` named cannot be moved onto.
#[derive(Grouped)]
pub struct ErrorAlignMove {
    /// What was named.
    path: String,
    /// What is wrong with it.
    kind: MoveKind,
}

impl Failure for ErrorAlignMove {
    fn name(&self) -> &'static str {
        "error_align_move"
    }

    fn reason(&self) -> String {
        let said = match self.kind {
            MoveKind::Missing => t!("align.err_move_missing", path = self.path),
            MoveKind::NotATree => t!("align.err_move_not_a_tree", path = self.path),
            MoveKind::Tracked => t!("align.err_move_tracked", path = self.path),
        };

        said.trim().to_string()
    }
}

failure!(ErrorAlignMove);

#[renderer(buffer)]
pub fn render_error_align_move(error: ErrorAlignMove, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_move_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: a per-file mode was asked of a tree.
#[derive(Grouped)]
pub struct ErrorAlignTreeUnsupported;

impl Failure for ErrorAlignTreeUnsupported {
    fn name(&self) -> &'static str {
        "error_align_tree_unsupported"
    }

    fn reason(&self) -> String {
        t!("align.err_tree_unsupported").trim().to_string()
    }
}

failure!(ErrorAlignTreeUnsupported);

#[renderer(buffer)]
pub fn render_error_align_tree_unsupported(_: ErrorAlignTreeUnsupported, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("align.err_tree_unsupported").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("align.err_tree_unsupported_help").trim())
    );
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the alignment could not be made.
#[derive(Grouped)]
pub struct ErrorAlignFailed {
    /// Why it could not.
    cause: String,
}

impl Failure for ErrorAlignFailed {
    fn name(&self) -> &'static str {
        "error_align_failed"
    }

    fn reason(&self) -> String {
        t!("align.err_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignFailed);

#[renderer(buffer)]
pub fn render_error_align_failed(error: ErrorAlignFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_failed_help").trim()));
    ec.exit_code = EC_ERR_ALIGN;
}

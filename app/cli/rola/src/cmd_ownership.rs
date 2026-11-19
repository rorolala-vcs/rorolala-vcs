//! The `rola hold` and `rola giveup` commands: taking entries and letting them go, safely.
//!
//! `rola layout req-ownership` and `rola layout giveup-ownership` are the Vault's own doors to
//! ownership, and they are the whole of what either moves. What they cannot do is look at the work
//! this side is standing on: whether what is here is the version the Vault names, whether it has
//! been changed since, and whether the run is about to hand over something it meant to keep. That
//! is what these two are for.
//!
//! Each fetches the Vault's Layout first, since every gate is read against what the Vault holds
//! now, and each writes what the Vault agreed to into that copy afterwards, so a reading next
//! shows the ownership as it now is. What they move is every entry the names stand for, a file is
//! itself, a directory is every file under it, and `.` is the directory the run was made in, and
//! the gates are read for all of them before any ownership moves: a batch that cannot be done whole
//! is not half done, unless `--allow-partial` says the doable part is worth doing anyway.
//!
//! The gates are the same for both, save for who is expected to hold the entry and for what a tree
//! with unrecorded work means: a claim may be made over work written before it was locked, while
//! letting go insists the tree agrees with the version being handed over. `--force` moves past the
//! version, never past the holder.

// The handlers' parameters are the resources the framework injects, so how many of them there are is
// what the commands need of the run rather than a signature this program shaped. The progress setting
// is one of those resources and is taken by reference, which is what the injection gives it. The run
// itself is one reading of every gate followed by one exchange, and splitting it would pass the same
// six values through a second signature rather than make any part of it clearer.
#![allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use librorolala::daemon::action_fetch_layout;
use librorolala::ignore::IgnoreRuleSet;
use librorolala::layout::Layout;
use librorolala::workspace::Workspace;
use librorolala::workspace_analyze::{TreeDiff, tree_diff};
use librorolala::write::ownership::{Kind as Own, Options, Refusal, apply, expand, plan};
use librorolala::write::{Cancellation, WriteError};
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResCurrentRemoteVault, ResForce, ResOffline, ResProgressSetting, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_progress::Progress;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{strip_written, typing_flag};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_LAYOUT_ARGUMENT, EC_ERR_LAYOUT_OWNERSHIP, EC_HELP};
use crate::failure::failure;
use crate::keys::account_named;
use crate::layout::{ErrorLayoutFailed, cached_vault_copy};
use crate::progress::Reporting;

/// How alike two text files have to be to count as the same file moved, when nothing is said.
const ALIKE: f32 = 0.6;

/// The word the command that moves ownership this way is known by.
const fn own_name(kind: Own) -> &'static str {
    match kind {
        Own::Hold => "hold",
        Own::Giveup => "giveup",
    }
}

/// Move what can be moved when not everything named can be.
const ARG_ALLOW_PARTIAL: PickerArg<'static, Flag> = arg![allow_partial: Flag];

#[help(buffer)]
pub fn help_hold(_: EntryHold, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("hold.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryHold)]
pub fn desc_hold() -> Description {
    t!("hold.description").to_string().into()
}

/// Completes what `rola hold` can be given next.
#[completion(EntryHold)]
pub fn complete_hold(ctx: ShellContext) -> Suggest {
    complete_ownership(&ctx)
}

/// Completes what `rola giveup` can be given next.
#[completion(EntryGiveup)]
pub fn complete_giveup(ctx: ShellContext) -> Suggest {
    complete_ownership(&ctx)
}

/// The completion the two commands that move ownership share.
///
/// Every word that is not a flag names where a file sits, so the filesystem answers it; the one
/// flag says whether a batch that cannot be done whole may be done in part.
fn complete_ownership(ctx: &ShellContext) -> Suggest {
    if typing_flag(ctx) {
        return strip_written(
            ctx,
            suggest! {
                ARG_ALLOW_PARTIAL: t!("hold.complete.allow_partial"),
            },
        );
    }

    Suggest::file_comp()
}

/// Claims entries for this run's account
///
/// Each `PATH` is where a file sits in the Layout being worked in; ownership is about the `Uuid`
/// that Layout names the path by, in the Vault the Layout tracks. A `PATH` that is a directory is
/// taken recursively, every file under it, and `.` is the directory the run was made in, so
/// `rola hold .` claims everything in the work that is not already claimed. Files that directory
/// brings in which the Layout does not name have no `Uuid` to be about, and are passed over.
///
/// That Vault's Layout is fetched first, since every gate below is read against what it holds now,
/// and every gate is read for every entry before any ownership moves:
///
/// - the entry must be one the Vault already holds: one it never took is nothing to claim;
/// - what is here must be the version the Vault names, `--force` moves past this. What the tree
///   holds on top of it need not be recorded: a version that matches is one nobody else has moved,
///   so work written before it was claimed still claims safely;
/// - the entry must be held by nobody: claiming what another account holds is not a claim, and
///   `--force` does not move this.
///
/// `--allow-partial` is what a run that knows some of them will not pass asks for: the ones that do
/// are claimed, the ones that do not are said, and the run ends unhappily either way.
///
/// # Errors
///
/// Renders [`ErrorOwnershipNoTrack`] when the Layout tracks no Vault, [`ErrorOwnershipName`] when a
/// name is not a file or directory inside the Workspace, and, unless `--allow-partial` is asked
/// for, whatever one of the gates refused, [`ErrorOwnershipPath`], [`ErrorOwnershipUpstream`],
/// [`ErrorOwnershipVersion`], [`ErrorOwnershipChanged`] or [`ErrorOwnershipTaken`], for the first
/// one, or [`ErrorOwnershipRefused`] when several were refused at once.
#[command(node = "hold", entry = EntryHold)]
pub fn hold(args: EntryHold, force: &ResForce, offline: &ResOffline) -> Next {
    // What a claim writes is the Vault's own Layout, and the fetch before it is not the point of
    // the command but a step of it: there is no offline claim to make.
    if **offline {
        return ErrorOffline.into();
    }

    let picked = args
        .pick_or_route(arg![Vec<String>], || {
            ErrorOwnershipArgument {
                command: "hold".to_owned(),
            }
            .to_chain()
        })
        .pick(ARG_ALLOW_PARTIAL)
        .pick(&UNMATCHED)
        .to_result();
    let (paths, allow_partial, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "hold") {
        return next;
    }

    // Picking several cannot fail for want of them, an absent list is an empty one, so a run
    // that named nothing is told so here rather than reading as a run with nothing to do.
    if paths.is_empty() {
        return ErrorOwnershipArgument {
            command: "hold".to_owned(),
        }
        .into();
    }

    StateHold {
        paths,
        force: **force,
        allow_partial: matches!(allow_partial, Flag::Active),
    }
    .into()
}

/// The state a claim starts in.
#[derive(Grouped)]
pub struct StateHold {
    /// Where the files sit in the Layout being worked in.
    paths: Vec<String>,
    /// Whether the version and the tree are to be gone past.
    force: bool,
    /// Whether what can be claimed is claimed when not everything can be.
    allow_partial: bool,
}

#[chain(routeify)]
pub fn handle_hold(
    state: StateHold,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    progress: &ResProgressSetting,
) -> Next {
    let StateHold {
        paths,
        force,
        allow_partial,
    } = state;

    run(
        Own::Hold,
        &paths,
        force,
        allow_partial,
        workspace,
        remote,
        current,
        progress,
    )
}

#[help(buffer)]
pub fn help_giveup(_: EntryGiveup, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("giveup.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryGiveup)]
pub fn desc_giveup() -> Description {
    t!("giveup.description").to_string().into()
}

/// Lets go of entries this run's account holds
///
/// Each `PATH` is where a file sits in the Layout being worked in; ownership is about the `Uuid`
/// that Layout names the path by, in the Vault the Layout tracks. A `PATH` that is a directory is
/// taken recursively, every file under it, and `.` is the directory the run was made in, so
/// `rola giveup .` lets go of everything in the work this account holds. Files that directory
/// brings in which the Layout does not name have no `Uuid` to be about, and are passed over.
///
/// That Vault's Layout is fetched first, since every gate below is read against what it holds now,
/// and every gate is read for every entry before any ownership moves:
///
/// - the entry must be one the Vault already holds: one it never took is nothing to let go of;
/// - what is here must be the version the Vault names, and nothing in it may be unrecorded,
///   `--force` moves past this;
/// - the entry must be held by this account: another account's is not this run's to let go of, and
///   one nobody holds is nothing to let go of at all. `--force` does not move this.
///
/// `--allow-partial` is what a run that knows some of them will not pass asks for: the ones that do
/// are let go of, the ones that do not are said, and the run ends unhappily either way.
///
/// # Errors
///
/// Renders [`ErrorOwnershipNoTrack`] when the Layout tracks no Vault, [`ErrorOwnershipName`] when a
/// name is not a file or directory inside the Workspace, and, unless `--allow-partial` is asked
/// for, whatever one of the gates refused, [`ErrorOwnershipPath`], [`ErrorOwnershipUpstream`],
/// [`ErrorOwnershipVersion`], [`ErrorOwnershipChanged`] or [`ErrorOwnershipNotHeld`], for the
/// first one, or [`ErrorOwnershipRefused`] when several were refused at once.
#[command(node = "giveup", entry = EntryGiveup)]
pub fn giveup(args: EntryGiveup, force: &ResForce, offline: &ResOffline) -> Next {
    // What letting go writes is the Vault's own Layout, and the fetch before it is not the point
    // of the command but a step of it: there is no offline release to make.
    if **offline {
        return ErrorOffline.into();
    }

    let picked = args
        .pick_or_route(arg![Vec<String>], || {
            ErrorOwnershipArgument {
                command: "giveup".to_owned(),
            }
            .to_chain()
        })
        .pick(ARG_ALLOW_PARTIAL)
        .pick(&UNMATCHED)
        .to_result();
    let (paths, allow_partial, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "giveup") {
        return next;
    }

    // Picking several cannot fail for want of them, an absent list is an empty one, so a run
    // that named nothing is told so here rather than reading as a run with nothing to do.
    if paths.is_empty() {
        return ErrorOwnershipArgument {
            command: "giveup".to_owned(),
        }
        .into();
    }

    StateGiveup {
        paths,
        force: **force,
        allow_partial: matches!(allow_partial, Flag::Active),
    }
    .into()
}

/// The state a release starts in.
#[derive(Grouped)]
pub struct StateGiveup {
    /// Where the files sit in the Layout being worked in.
    paths: Vec<String>,
    /// Whether the version and the tree are to be gone past.
    force: bool,
    /// Whether what can be let go of is let go of when not everything can be.
    allow_partial: bool,
}

#[chain(routeify)]
pub fn handle_giveup(
    state: StateGiveup,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    progress: &ResProgressSetting,
) -> Next {
    let StateGiveup {
        paths,
        force,
        allow_partial,
    } = state;

    run(
        Own::Giveup,
        &paths,
        force,
        allow_partial,
        workspace,
        remote,
        current,
        progress,
    )
}

/// Runs one of the two commands, with a reader watching what the Vault is asked for.
///
/// Split from the handlers so that the reader is started and waited for around the run exactly once,
/// however many ways it has of returning, the ownership moves themselves are one exchange, but the
/// gates before them are many ways out.
fn run(
    kind: Own,
    paths: &[String],
    force: bool,
    allow_partial: bool,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    progress: &ResProgressSetting,
) -> Next {
    let reporting = Reporting::start(*progress);
    let cancellation = crate::cancel::on_ctrl_c();

    // The handle is let go of before the reader is waited for: the last handle dropping is what tells
    // the reader there is nothing more to come.
    let next = {
        let progress = reporting.progress();
        change(
            kind,
            paths,
            force,
            allow_partial,
            workspace,
            remote,
            current,
            &progress,
            cancellation,
        )
    };

    reporting.finish();

    next
}

/// Moves the ownership of every entry the names stand for, once the gates have all been read.
///
/// `routeify` is what lets the two handlers above be one: what this returns is the same `Next` a
/// handler would, so a failure the framework knows (no Workspace, no Vault to reach, no account)
/// is carried out of here just as it would be out of either of them.
#[routeify]
fn change(
    kind: Own,
    names: &[String],
    force: bool,
    allow_partial: bool,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    progress: &Progress,
    cancellation: &Cancellation,
) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let layout = match held.layout_if_there() {
        Ok(Some(layout)) => layout,
        Ok(None) => {
            return ErrorOwnershipArgument {
                command: own_name(kind).to_owned(),
            }
            .into();
        }
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    let Some(track) = (match held.layout_track() {
        Ok(track) => track,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    }) else {
        return ErrorOwnershipNoTrack.into();
    };

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return ErrorOwnershipFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let vault_name = remote.get_ref().name_or_default(track)?;
    let target = remote.get_ref().vault_or_default(vault_name.clone())?;
    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    // Every gate is read against what the Vault holds now, so its copy is brought up to date first
    //, and that copy is what is written afterwards, so a reading agrees with the Vault. That fetch is
    // the wait a reader watches for, and it is said as a bar of its own the way a sync says its plan:
    // the ownership that follows is a second wait rather than the same one, and a reader shown only the
    // second would see a bar that appeared when the slow half was already over.
    let mut step = progress.begin("", Some(2));

    action_fetch_layout(held, &account, target.to_string(), vault_name.clone())?;
    step.advance_by(1);

    let copy = match cached_vault_copy(held, &vault_name) {
        Ok(copy) => copy,
        Err(next) => return next,
    };

    let selected = match expand(held, &cwd, names) {
        Ok(selected) => selected,
        Err(name) => return ErrorOwnershipName { name }.into(),
    };
    let diff = match settling(held, &layout, force) {
        Ok(diff) => diff,
        Err(next) => return next,
    };

    // What a tree with unrecorded work is judged by is the rules it is read with, which the command
    // loads once: the library only asks whether the reading agrees with the recorded baseline.
    let rules = crate::ignore::rules(held, false);
    let (wanted, mut failed, skipped) = match plan(
        kind,
        Options { force },
        held,
        &layout,
        &copy,
        &account_name,
        &selected,
        diff.as_ref(),
        &rules,
        cancellation,
    ) {
        Ok(planned) => planned,
        Err(error) => return ownership_write_error(&error),
    };
    step.advance_by(1);

    // The fetch is over before the ownership begins, so its bar is let go of here rather than left standing
    // beside the one that follows: a sync lets go of its plan the same way.
    step.finish();

    // A batch that cannot be done whole is not half done: every gate was read before any ownership
    // moved, and what did not pass is the whole of what the run is told about.
    if !failed.is_empty() && !allow_partial {
        if failed.len() == 1 {
            return refusal_state(failed.remove(0));
        }

        return ErrorOwnershipRefused {
            command: own_name(kind).to_owned(),
            failures: failed.iter().map(describe).collect(),
        }
        .into();
    }

    // What the bar is called belongs to whoever watches the run: the exchange runs on both ends, and
    // the Vault's end has no words of its own.
    let name = match kind {
        Own::Hold => t!("ownership.holding").trim().to_string(),
        Own::Giveup => t!("ownership.giving").trim().to_string(),
    };
    let outcome = match apply(
        kind,
        wanted,
        held,
        &copy,
        &account,
        &target,
        &name,
        progress,
        cancellation,
    ) {
        Ok(outcome) => outcome,
        Err(error) => return ownership_write_error(&error),
    };
    failed.extend(outcome.failed);

    ResultOwnership {
        kind,
        entries: outcome
            .entries
            .into_iter()
            .map(|item| OwnershipItem {
                path: item.path,
                uuid: item.uuid,
                owner: item.owner,
            })
            .collect(),
        skipped,
        failed: failed.iter().map(describe).collect(),
    }
    .into()
}

/// The failure a write the ownership workflow returned is told as.
///
/// Every gate has a failure of its own and is mapped by [`refusal_state`]; what reaches here is a
/// write that stopped before the gates were read, which today is only cancellation.
fn ownership_write_error(error: &WriteError) -> Next {
    ErrorOwnershipFailed {
        cause: error.to_string(),
    }
    .into()
}

/// The reading a giveup's gates judge a tree by, or nothing when `--force` moves past it.
///
/// Ownership is asked about entries the Layout names, which the ignore rules have no jurisdiction
/// over, so the reading is taken unfiltered.
fn settling(held: &Workspace, layout: &Layout, force: bool) -> Result<Option<TreeDiff>, Next> {
    if force {
        return Ok(None);
    }

    tree_diff(layout, held, ALIKE, &IgnoreRuleSet::empty())
        .map(Some)
        .map_err(|error| ErrorLayoutFailed::new(error.to_string()).into())
}

/// The JSON an ownership action takes as its input: the words a bar is drawn under, and the entries.
///
/// A list is not a thing the C ABI can hand over, so the pair crosses as one JSON string. Both this
/// command and the Vault's own doors build it here, so the shape is written once rather than guessed
/// at wherever an action happens to be called.
///
/// # Errors
///
/// Whatever encoding the pair fails with, which is nothing a caller can bring about.
pub fn ownership_input(name: String, ids: Vec<String>) -> Result<String, serde_json::Error> {
    serde_json::to_string(&(name, ids))
}

/// What was refused, in the words the run is told it in.
fn describe(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Path(path) => format!("{path}: {}", t!("ownership.why_path").trim()),
        Refusal::Upstream(path) => format!("{path}: {}", t!("ownership.why_upstream").trim()),
        Refusal::Version(path) => format!("{path}: {}", t!("ownership.why_version").trim()),
        Refusal::Changed(path) => format!("{path}: {}", t!("ownership.why_changed").trim()),
        Refusal::Taken { path, owner } => {
            format!(
                "{path}: {}",
                t!("ownership.why_taken", owner = owner).trim()
            )
        }
        Refusal::NotHeld {
            path,
            owner: Some(owner),
        } => format!(
            "{path}: {}",
            t!("ownership.why_not_yours", owner = owner).trim()
        ),
        Refusal::NotHeld { path, owner: None } => {
            format!("{path}: {}", t!("ownership.why_nobody").trim())
        }
        Refusal::Failed(cause) => cause.clone(),
    }
}

/// Turns one refusal into what the run is told.
#[routeify]
fn refusal_state(refusal: Refusal) -> Next {
    match refusal {
        Refusal::Path(path) => ErrorOwnershipPath { path }.into(),
        Refusal::Upstream(path) => ErrorOwnershipUpstream { path }.into(),
        Refusal::Version(path) => ErrorOwnershipVersion { path }.into(),
        Refusal::Changed(path) => ErrorOwnershipChanged { path }.into(),
        Refusal::Taken { path, owner } => ErrorOwnershipTaken { path, owner }.into(),
        Refusal::NotHeld { path, owner } => ErrorOwnershipNotHeld { path, owner }.into(),
        Refusal::Failed(cause) => ErrorOwnershipFailed { cause }.into(),
    }
}

/// Error: `rola hold` or `rola giveup` was given arguments it cannot use.
#[derive(Grouped)]
pub struct ErrorOwnershipArgument {
    /// Which of the two was run.
    pub command: String,
}

impl Failure for ErrorOwnershipArgument {
    fn name(&self) -> &'static str {
        "error_ownership_argument"
    }

    fn reason(&self) -> String {
        t!("ownership.err_argument", command = self.command)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipArgument);

#[renderer(buffer)]
pub fn render_error_ownership_argument(error: ErrorOwnershipArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_argument_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: a name is not a file or a directory inside the Workspace.
#[derive(Grouped)]
pub struct ErrorOwnershipName {
    /// What was named, or why the tree could not be read.
    pub name: String,
}

impl Failure for ErrorOwnershipName {
    fn name(&self) -> &'static str {
        "error_ownership_name"
    }

    fn reason(&self) -> String {
        t!("ownership.err_name", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipName);

#[renderer(buffer)]
pub fn render_error_ownership_name(error: ErrorOwnershipName, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_name_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: the Layout being worked in tracks no Vault.
#[derive(Grouped)]
pub struct ErrorOwnershipNoTrack;

impl Failure for ErrorOwnershipNoTrack {
    fn name(&self) -> &'static str {
        "error_ownership_no_track"
    }

    fn reason(&self) -> String {
        t!("ownership.err_no_track").trim().to_string()
    }
}

failure!(ErrorOwnershipNoTrack);

#[renderer(buffer)]
pub fn render_error_ownership_no_track(error: ErrorOwnershipNoTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_no_track_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the Layout names nothing at the path given.
#[derive(Grouped)]
pub struct ErrorOwnershipPath {
    /// The path the Layout names nothing at.
    pub path: String,
}

impl Failure for ErrorOwnershipPath {
    fn name(&self) -> &'static str {
        "error_ownership_path"
    }

    fn reason(&self) -> String {
        t!("ownership.err_path", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipPath);

#[renderer(buffer)]
pub fn render_error_ownership_path(error: ErrorOwnershipPath, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_path_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the Vault's Layout names nothing for the `Uuid` the path is named by.
#[derive(Grouped)]
pub struct ErrorOwnershipUpstream {
    /// The path the Vault names nothing for.
    pub path: String,
}

impl Failure for ErrorOwnershipUpstream {
    fn name(&self) -> &'static str {
        "error_ownership_upstream"
    }

    fn reason(&self) -> String {
        t!("ownership.err_upstream", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipUpstream);

#[renderer(buffer)]
pub fn render_error_ownership_upstream(error: ErrorOwnershipUpstream, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_upstream_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: what is here is not the version the Vault's Layout names.
#[derive(Grouped)]
pub struct ErrorOwnershipVersion {
    /// The path that is at another version.
    pub path: String,
}

impl Failure for ErrorOwnershipVersion {
    fn name(&self) -> &'static str {
        "error_ownership_version"
    }

    fn reason(&self) -> String {
        t!("ownership.err_version", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipVersion);

#[renderer(buffer)]
pub fn render_error_ownership_version(error: ErrorOwnershipVersion, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_version_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: what is here has changes the Layout does not name.
#[derive(Grouped)]
pub struct ErrorOwnershipChanged {
    /// The path that was changed.
    pub path: String,
}

impl Failure for ErrorOwnershipChanged {
    fn name(&self) -> &'static str {
        "error_ownership_changed"
    }

    fn reason(&self) -> String {
        t!("ownership.err_changed", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipChanged);

#[renderer(buffer)]
pub fn render_error_ownership_changed(error: ErrorOwnershipChanged, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_changed_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the entry is held by another account, so it is not free to claim.
#[derive(Grouped)]
pub struct ErrorOwnershipTaken {
    /// The path that is held.
    pub path: String,
    /// The account that holds it.
    pub owner: String,
}

impl Failure for ErrorOwnershipTaken {
    fn name(&self) -> &'static str {
        "error_ownership_taken"
    }

    fn reason(&self) -> String {
        t!("ownership.err_held", path = self.path, owner = self.owner)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipTaken);

#[renderer(buffer)]
pub fn render_error_ownership_taken(error: ErrorOwnershipTaken, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_held_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the entry is not this account's to let go of.
#[derive(Grouped)]
pub struct ErrorOwnershipNotHeld {
    /// The path that is not this account's.
    pub path: String,
    /// The account that holds it, or nothing when nobody does.
    pub owner: Option<String>,
}

impl Failure for ErrorOwnershipNotHeld {
    fn name(&self) -> &'static str {
        "error_ownership_not_held"
    }

    fn reason(&self) -> String {
        self.owner.as_ref().map_or_else(
            || {
                t!("ownership.err_nobody", path = self.path)
                    .trim()
                    .to_string()
            },
            |owner| {
                t!("ownership.err_not_yours", path = self.path, owner = owner)
                    .trim()
                    .to_string()
            },
        )
    }
}

failure!(ErrorOwnershipNotHeld);

#[renderer(buffer)]
pub fn render_error_ownership_not_held(error: ErrorOwnershipNotHeld, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_not_held_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: several of the entries named cannot have their ownership moved.
#[derive(Grouped)]
pub struct ErrorOwnershipRefused {
    /// Which command was run.
    pub command: String,
    /// What could not be moved, one line each.
    pub failures: Vec<String>,
}

impl Failure for ErrorOwnershipRefused {
    fn name(&self) -> &'static str {
        "error_ownership_refused"
    }

    fn reason(&self) -> String {
        t!(
            "ownership.err_refused",
            command = self.command,
            count = self.failures.len()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorOwnershipRefused);

#[renderer(buffer)]
pub fn render_error_ownership_refused(error: ErrorOwnershipRefused, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));

    for failure in &error.failures {
        r_eprintln!("{}", err_line!(failure));
    }

    r_eprintln!("{}", help_line!(t!("ownership.err_refused_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the exchange could not be made.
#[derive(Grouped)]
pub struct ErrorOwnershipFailed {
    /// What the exchange failed with.
    pub cause: String,
}

impl Failure for ErrorOwnershipFailed {
    fn name(&self) -> &'static str {
        "error_ownership_failed"
    }

    fn reason(&self) -> String {
        t!("ownership.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipFailed);

#[renderer(buffer)]
pub fn render_error_ownership_failed(error: ErrorOwnershipFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_failed_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// One entry whose ownership was moved.
#[derive(Serialize)]
pub struct OwnershipItem {
    /// Where the file sits in the Layout.
    path: String,
    /// The `Uuid` ownership was moved for.
    uuid: String,
    /// The account that holds it now, or nothing when nobody does.
    owner: Option<String>,
}

/// Result: the ownership of the entries named was moved.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultOwnership {
    /// Which way it moved.
    kind: Own,
    /// What moved, in path order.
    entries: Vec<OwnershipItem>,
    /// How many files a directory brought in that the Layout does not name.
    skipped: usize,
    /// What was not moved, one line each.
    failed: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_ownership(result: ResultOwnership, ec: &mut ResExitCode) {
    for item in &result.entries {
        let said = match result.kind {
            Own::Hold => t!("ownership.result_held", path = item.path),
            Own::Giveup => t!("ownership.result_given", path = item.path),
        };

        r_println!("{}", said.trim());
    }

    if result.skipped > 0 {
        r_println!("{}", t!("ownership.skipped", count = result.skipped).trim());
    }

    for failure in &result.failed {
        r_eprintln!("{}", err_line!(failure));
    }

    if !result.failed.is_empty() {
        ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
    }
}

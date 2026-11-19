//! The `rola sync` command: moving a Layout's work to and from the Vault it tracks.
//!
//! A Layout tracks one Vault, the name in its `TRACK` file, and this is how the two are brought
//! together. What the Vault holds now is fetched first, since the plan is made from it, and the
//! plan says what is to move and which way. `--dry-run` stops there: it prints the plan and does
//! nothing, which is what the run that wants to know before it is changed asks for.
//!
//! What goes up is the plan's creations and versions: a `Uuid` only this Layout holds is made an
//! entry of the Vault's Layout at the path the work tracks it at, and a version the Vault is behind
//! on is set. What comes down is a version the Vault is ahead on: what it stored is written into
//! the tree, and the version into the Layout. Content is moved around the Layout either way, the
//! index and the store, unless `--no-index` or `--no-storage` says otherwise, and `--no-layout`
//! leaves the Layout out of it entirely: no plan is made, the index is exchanged both ways, and the
//! stored content is uploaded. A direction is a narrowing of the plan, so `--up-only` and
//! `--down-only` are refused beside it.
//!
//! A file changed here that the Vault is ahead on is refused before anything is written, so a pull
//! never overwrites what the run does not know about; merging is not here yet.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates,
// so a lint allowed on the handler below is reported as defined twice. The allow lives here,
// where it covers the one signature that needs it.
#![allow(clippy::trivially_copy_pass_by_ref)]

use librorolala::daemon::action_fetch_layout;
use librorolala::layout::Layout;
use librorolala::storage::RorolalaStorage;
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
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
    ResConcurrency, ResCurrentRemoteVault, ResForce, ResOffline, ResProgressSetting,
    ResRorolalaStorage, ResVCSIndex, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd, warn_line};
use rorolala_utils_constants::{DEFAULT_LAYOUT_NAME, VAULT_LAYOUT_NAME};
use rorolala_utils_progress::Progress;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{strip_written, typing_flag};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_SYNC, EC_ERR_SYNC_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutFailed, ErrorLayoutNoCurrent, ErrorLayoutNotCached, failed, readonly_layout_copy,
};
use crate::progress::Reporting;
use crate::sync::{self, Direction, Entry, Kind};
use crate::vcs_index::{ErrorVcsIndexNoIndex, runtime as index_runtime};

/// Only what this Layout holds goes up.
const ARG_UP_ONLY: PickerArg<'static, Flag> = arg![up_only: Flag];

/// Only what the Vault holds comes down.
const ARG_DOWN_ONLY: PickerArg<'static, Flag> = arg![down_only: Flag];

/// Both directions; the one taken when neither is named.
const ARG_BOTH: PickerArg<'static, Flag> = arg![both: Flag];

/// Show the plan and change nothing.
const ARG_DRY_RUN: PickerArg<'static, Flag> = arg![dry_run: Flag];

/// Leave the index alone; only the Layout and the store move.
const ARG_NO_INDEX: PickerArg<'static, Flag> = arg![no_index: Flag];

/// Leave the store alone; only the Layout and the index move.
const ARG_NO_STORAGE: PickerArg<'static, Flag> = arg![no_storage: Flag];

/// Leave the Vault's Layout alone; only the index and the store move.
const ARG_NO_LAYOUT: PickerArg<'static, Flag> = arg![no_layout: Flag];

#[help(buffer)]
pub fn help_sync(_: EntrySync, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("sync.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntrySync)]
pub fn desc_sync() -> Description {
    t!("sync.description").to_string().into()
}

/// Completes what `rola sync` can be given next.
///
/// The command names nothing: what moves is decided by the plan the Layout and the Vault make, and
/// the whole of what a caller chooses is which parts of it are left alone.
#[completion(EntrySync)]
pub fn complete_sync(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_UP_ONLY: t!("sync.complete.up_only"),
                ARG_DOWN_ONLY: t!("sync.complete.down_only"),
                ARG_BOTH: t!("sync.complete.both"),
                ARG_DRY_RUN: t!("sync.complete.dry_run"),
                ARG_NO_INDEX: t!("sync.complete.no_index"),
                ARG_NO_STORAGE: t!("sync.complete.no_storage"),
                ARG_NO_LAYOUT: t!("sync.complete.no_layout"),
            },
        );
    }

    suggest!()
}

/// Brings the Layout being worked in together with the Vault it tracks
///
/// The Vault is the one the Layout's `TRACK` names, which the Workspace has
/// [bound](crate::vault::cmd_vault_bind). What the Vault holds now is fetched first, and the plan
/// says what is to move: a `Uuid` only this Layout holds goes up at the path the work tracks it at,
/// a version the Vault is behind on goes up when this account holds it, one this Layout is behind
/// on comes down, and a `Uuid` only the Vault holds is named rather than brought, `bring` is
/// what brings one.
///
/// `--up-only` and `--down-only` keep the plan to one direction; neither, or `--both`, is both.
/// `--force` is what lets a version whose chain below the Vault's has moved apart go up anyway.
/// `--dry-run` prints the plan and changes nothing. What the plan names then has its content
/// moved: the index and the store, unless `--no-index` or `--no-storage` leaves one behind.
///
/// `--no-layout` leaves the Layout itself behind, and a run that reads no Layout has no plan for a
/// direction to narrow: it exchanges the index both ways and uploads the stored content, and naming
/// `--up-only` or `--down-only` beside it is refused.
///
/// # Errors
///
/// Renders [`ErrorSyncNoTrack`] when the Layout tracks no Vault, [`ErrorSyncBlocked`] when a file
/// changed here is behind the Vault, [`ErrorSyncFailed`] when the exchange could not be made,
/// [`ErrorSyncNoLayoutDirection`] when a direction is named with `--no-layout`, and the exchange's
/// own failures otherwise.
#[command(node = "sync", entry = EntrySync)]
pub fn sync(
    args: EntrySync,
    force: &ResForce,
    offline: &ResOffline,
    concurrency: &ResConcurrency,
) -> Next {
    // A sync is the exchange with the Vault, whole: there is no local half of it an offline run
    // could be left with.
    if **offline {
        return ErrorOffline.into();
    }

    // UNWRAP: Picking cannot fail: a positional that is absent is `None` and a flag that is absent
    // is `Inactive`, neither of them an error.
    let (up_only, down_only, both, dry_run, no_index, no_storage, no_layout, unmatched) = args
        .pick(ARG_UP_ONLY)
        .pick(ARG_DOWN_ONLY)
        .pick(ARG_BOTH)
        .pick(ARG_DRY_RUN)
        .pick(ARG_NO_INDEX)
        .pick(ARG_NO_STORAGE)
        .pick(ARG_NO_LAYOUT)
        .pick(&UNMATCHED)
        .unwrap();

    if let Some(next) = crate::unmatched::refused(&unmatched, "sync") {
        return next;
    }

    let up = matches!(up_only, Flag::Active);
    let down = matches!(down_only, Flag::Active);
    let both = matches!(both, Flag::Active);
    let no_layout = matches!(no_layout, Flag::Active);

    // At most one direction may be named; naming none is both.
    if [up, down, both].into_iter().filter(|named| *named).count() > 1 {
        return ErrorSyncArgument.into();
    }

    let direction = match (up, down) {
        (true, _) => Direction::Up,
        (_, true) => Direction::Down,
        _ => Direction::Both,
    };

    // A run that reads no Layout has no plan for a direction to narrow: what it does is fixed,
    // the index both ways and the store up, so a direction is refused rather than dropped.
    if no_layout && (up || down) {
        return ErrorSyncNoLayoutDirection.into();
    }

    StateSync {
        direction,
        forced: **force,
        dry_run: matches!(dry_run, Flag::Active),
        concurrency: **concurrency,
        skip: Skip {
            index: matches!(no_index, Flag::Active),
            storage: matches!(no_storage, Flag::Active),
            layout: no_layout,
        },
    }
    .into()
}

/// The state a sync starts in.
#[derive(Grouped, Clone, Copy)]
pub struct StateSync {
    /// How far the run is to go.
    direction: Direction,
    /// Whether a version whose chain moved apart may go up anyway.
    forced: bool,
    /// How many connections the content is carried over when there is more than one key to move.
    concurrency: usize,
    /// Whether the plan is to be printed rather than carried out.
    dry_run: bool,
    /// What the run is told not to move.
    skip: Skip,
}

/// What a run is told not to move.
#[derive(Clone, Copy)]
pub struct Skip {
    /// Whether the index is left alone.
    index: bool,
    /// Whether the store is left alone.
    storage: bool,
    /// Whether the Vault's Layout is left alone, leaving the index and the store to move.
    layout: bool,
}

#[chain(routeify)]
pub fn handle_sync(
    state: StateSync,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    progress: &ResProgressSetting,
) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    // The Layout being worked in is the one the plan is about; there is one per Workspace, so it is
    // the one a run is working in.
    let layout = match held.layout_if_there() {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutNoCurrent.into(),
        Err(error) => return failed(&error),
    };

    let Some(track) = (match held.layout_track() {
        Ok(track) => track,
        Err(error) => return failed(&error),
    }) else {
        return ErrorSyncNoTrack.into();
    };

    let vault_name = remote.get_ref().name_or_default(track)?;
    let target = remote.get_ref().vault_or_default(vault_name.clone())?;

    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    let runtime = match index_runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };

    // What a run works through: every piece the two sides are reached by, and how it is watched.
    let carry = Carry {
        workspace: held,
        account: &account,
        target: &target,
        layout: &layout,
        index,
        store: storage.get_ref().as_ref(),
        remote: remote.get_ref(),
        runtime: &runtime,
    };

    // What the Vault holds now is what the plan is made from, so its copy is brought up to date
    // first, the same fetch `rola layout fetch` makes. A run that is not about the Layout at all,
    // which `--no-layout` says, makes no plan and reads no copy. One watching covers the whole run:
    // the steps that make the plan and carry it out are one thing to a reader, and a bar waiting on
    // each part would never have been let out at all.
    let reporting = Reporting::start(*progress);

    let plan = if state.skip.layout {
        None
    } else {
        match plan_with(
            &reporting.progress(),
            &carry,
            state,
            &account_name,
            &vault_name,
        ) {
            Ok(plan) => plan,
            Err(PlanRefusal::NotCached) => {
                reporting.finish();

                return ErrorLayoutNotCached {
                    layout: VAULT_LAYOUT_NAME.to_owned(),
                    vault: vault_name,
                }
                .into();
            }
            Err(PlanRefusal::Failed(cause)) => {
                reporting.finish();

                return ErrorLayoutFailed::new(cause).into();
            }
        }
    };

    // What a run that wants to know before it is changed asks for: the plan, and nothing done.
    if state.dry_run {
        let next = planned(
            plan.as_ref(),
            state,
            DEFAULT_LAYOUT_NAME.to_owned(),
            vault_name,
        );

        reporting.finish();

        return next;
    }

    // A file changed here that the Vault is ahead on is a pull that would overwrite what this side
    // does not know about, so it is refused before anything is written.
    if let Some(plan) = &plan
        && plan.entries.iter().any(|entry| entry.kind == Kind::Blocked)
    {
        reporting.finish();

        return ErrorSyncBlocked.into();
    }

    let outcome = carry_with(
        &reporting.progress(),
        plan.as_ref(),
        state,
        &carry,
        &vault_name,
        &account_name,
    );
    reporting.finish();

    match outcome {
        Ok(carried) => done(
            plan.as_ref(),
            DEFAULT_LAYOUT_NAME.to_owned(),
            vault_name,
            carried,
        ),
        Err(error) => error.into(),
    }
}

/// What a run that was carried out answers with.
///
/// What a run with no plan did is not in the three plan counts, there was no plan, so the counts
/// of what crossed are carried through beside them, and which pair is printed follows from whether
/// there was a Layout to read.
fn done(plan: Option<&sync::Plan>, layout: String, vault: String, carried: Carried) -> Next {
    ResultSyncDone {
        layout,
        vault,
        applied: carried.applied,
        received: carried.received,
        up_to_date: plan.map_or(0, |plan| plan.up_to_date),
        failed: carried.failed,
        layout_read: plan.is_some(),
        index_objects: carried.index_objects,
        storage_objects: carried.storage_objects,
    }
    .into()
}

/// What a run that only wants to know asks for: the plan, and nothing done.
fn planned(plan: Option<&sync::Plan>, state: StateSync, layout: String, vault: String) -> Next {
    let (entries, up_to_date, forced) = plan.map_or_else(
        || (Vec::new(), 0, state.forced),
        |plan| (plan.entries.clone(), plan.up_to_date, plan.forced),
    );

    ResultSyncPlan {
        layout,
        vault,
        forced,
        entries,
        up_to_date,
        layout_read: plan.is_some(),
        index: !state.skip.index,
        storage: !state.skip.storage,
    }
    .into()
}

/// Fetches the Vault's Layout and builds the plan, saying what it is doing while it does.
///
/// A fetch and a plan are the two steps a run makes before it knows what is to move, and only the
/// first is an exchange: a reader watching a large Layout come over, or a large tree read against
/// the index, has the bar to watch either way.
///
/// # Errors
///
/// Returns [`PlanRefusal::NotCached`] when the copy the plan is read from is not there, and
/// [`PlanRefusal::Failed`] when the fetch or the plan could not be made.
fn plan_with(
    progress: &Progress,
    carry: &Carry<'_>,
    state: StateSync,
    account_name: &str,
    vault_name: &str,
) -> Result<Option<sync::Plan>, PlanRefusal> {
    let mut bar = progress.begin("", Some(2));

    action_fetch_layout(
        carry.workspace,
        carry.account,
        carry.target.to_string(),
        vault_name.to_owned(),
    )
    .map_err(|error| PlanRefusal::Failed(error.to_string()))?;
    bar.advance_by(1);

    let Some(dir) = readonly_layout_copy(carry.workspace, vault_name, VAULT_LAYOUT_NAME) else {
        return Err(PlanRefusal::NotCached);
    };
    let remote_layout = match Layout::open_existing(&dir) {
        Ok(layout) => layout,
        // A directory with none of the Layout in it is what a fetch that did not finish leaves, and it is not a copy
        // to plan against: the plan is refused as a run that has not fetched is.
        Err(error) if error.is_not_found() => return Err(PlanRefusal::NotCached),
        Err(error) => return Err(PlanRefusal::Failed(error.to_string())),
    };

    let mut plan = carry
        .runtime
        .block_on(sync::plan(
            carry.workspace,
            carry.layout,
            &remote_layout,
            carry.index,
            account_name,
            state.direction,
            state.forced,
            // The command has no way to stop a run, so the token it hands the library is never
            // set: a `rola sync` runs to its end, exactly as it did before it was cancellable.
            &librorolala::write::Cancellation::new(),
        ))
        .map_err(|error| PlanRefusal::Failed(error.to_string()))?;
    vault_name.clone_into(&mut plan.vault);
    bar.advance_by(1);

    Ok(Some(plan))
}

/// What a run works through while it carries a plan out.
struct Carry<'a> {
    /// The Workspace being worked in.
    workspace: &'a librorolala::workspace::Workspace,
    /// The account the exchange is spoken as.
    account: &'a librorolala::auth::Account,
    /// The Vault being reached.
    target: &'a librorolala::protocol::VaultAddress,
    /// The Layout being worked in, whose versions a pull sets.
    layout: &'a Layout,
    /// The index below both sides.
    index: &'a VCSIndex,
    /// The store content is written from, when the run has one.
    store: Option<&'a RorolalaStorage>,
    /// The Vaults the run may reach, for content that is not here.
    remote: &'a ResCurrentRemoteVault,
    /// The runtime the asynchronous exchanges are waited on.
    runtime: &'a tokio::runtime::Runtime,
}

/// What carrying a plan out came to.
struct Carried {
    /// How many entries were written to the Vault.
    applied: usize,
    /// How many versions were brought down from the Vault.
    received: usize,
    /// What was not done, one line each.
    failed: Vec<String>,
    /// How many stored objects the index named, which is what a run with no plan asked for.
    index_objects: usize,
    /// How many units of stored content the run sent.
    storage_objects: usize,
}

/// Writes what a plan names, and moves the content that gives it meaning.
///
/// Content is moved before the Layout is touched either way: a version is only a meaning once what
/// it stored is here, and what goes up names versions the Vault has to be able to read.
///
/// The two exchanges say their own progress (they are actions, and an action reports itself) and
/// what is left, one entry at a time, is said here, since no action knows it is part of a longer
/// run. The watching the bars are drawn into is the caller's.
///
/// The exchange is asynchronous and a command is not, so the runtime made by the caller waits for
/// it, and what it is doing is said to a reader on a thread of its own.
///
/// # Errors
///
/// Returns [`ErrorSyncFailed`] when the exchange could not be made.
fn carry_with(
    progress: &Progress,
    plan: Option<&sync::Plan>,
    state: StateSync,
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
) -> Result<Carried, ErrorSyncFailed> {
    let messages = librorolala::write::sync_apply::Messages {
        malformed: t!("sync.write_malformed").trim().to_owned(),
        no_content: t!("sync.write_no_content").trim().to_owned(),
        missing: t!("sync.write_missing").trim().to_owned(),
        force: t!("sync.write_force").trim().to_owned(),
        refused: t!("sync.write_refused").trim().to_owned(),
        kind_mismatch: t!("sync.kind_mismatch").trim().to_owned(),
        written: t!("sync.write_written").trim().to_owned(),
        exists: t!("sync.write_exists").trim().to_owned(),
        taken: t!("sync.write_taken").trim().to_owned(),
    };
    let primary = fetch::primary(carry.remote, Some(vault_name));
    let candidates = fetch::candidates(carry.remote, primary);
    let skip = librorolala::write::sync_apply::Skip {
        index: state.skip.index,
        storage: state.skip.storage,
        layout: state.skip.layout,
    };
    let options = librorolala::write::sync_apply::Options {
        forced: state.forced,
        concurrency: state.concurrency,
        skip,
    };
    let sync_carry = librorolala::write::sync_apply::Carry {
        workspace: carry.workspace,
        account: carry.account,
        target: carry.target,
        layout: carry.layout,
        index: carry.index,
        store: carry.store,
        candidates: &candidates,
        messages: &messages,
        runtime: carry.runtime,
    };
    let applied = librorolala::write::sync_apply::apply(
        progress,
        plan,
        options,
        &sync_carry,
        vault_name,
        account_name,
        // The token a command hands the library is never set, so the run goes on to its end.
        &librorolala::write::Cancellation::new(),
    )
    .map_err(|cause| ErrorSyncFailed {
        cause: cause.to_string(),
    })?;

    if applied.missing_layout {
        eprintln!("{}", warn_line!(t!("sync.warn_layout_not_fetched").trim()));
    }
    if !applied.took.is_empty() {
        let count: usize = applied.took.iter().map(|(_, count)| count).sum();
        let vaults: Vec<&str> = applied.took.iter().map(|(name, _)| name.as_str()).collect();
        eprintln!(
            "{}",
            warn_line!(t!("fetch.brought", count = count, vaults = vaults.join(", ")).trim())
        );
    }

    Ok(Carried {
        applied: applied.applied,
        received: applied.received,
        failed: applied.failed,
        index_objects: applied.index_objects,
        storage_objects: applied.storage_objects,
    })
}
enum PlanRefusal {
    /// The Vault's Layout has not been fetched, so there is no copy to read.
    NotCached,
    /// The copy could not be read, or the plan could not be made.
    Failed(String),
}

/// Brings one entry's version down: what it stored into the tree, and the version into the Layout.
///
/// The path the file is at here is this side's own and is not moved by a pull; what the pull sets
/// is the version and who holds it, since the Vault is where both are kept.
///
/// # Errors
///
/// Returns [`ErrorSyncFailed`] when the index could not be read.
/// Error: the Layout being worked in tracks no Vault.
#[derive(Grouped)]
pub struct ErrorSyncNoTrack;

impl Failure for ErrorSyncNoTrack {
    fn name(&self) -> &'static str {
        "error_sync_no_track"
    }

    fn reason(&self) -> String {
        t!("sync.err_no_track").trim().to_string()
    }
}

failure!(ErrorSyncNoTrack);

#[renderer(buffer)]
pub fn render_error_sync_no_track(error: ErrorSyncNoTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("sync.err_no_track_help").trim()));
    ec.exit_code = EC_ERR_SYNC;
}

/// Error: a file changed here is behind the Vault, so a pull would overwrite it.
///
/// It is refused before anything is written: what is here is not something a run may decide about,
/// and merging is not here yet.
#[derive(Grouped)]
pub struct ErrorSyncBlocked;

impl Failure for ErrorSyncBlocked {
    fn name(&self) -> &'static str {
        "error_sync_blocked"
    }

    fn reason(&self) -> String {
        t!("sync.err_blocked").trim().to_string()
    }
}

failure!(ErrorSyncBlocked);

#[renderer(buffer)]
pub fn render_error_sync_blocked(error: ErrorSyncBlocked, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("sync.err_blocked_help").trim()));
    ec.exit_code = EC_ERR_SYNC;
}

/// Error: the exchange could not be made.
#[derive(Grouped)]
pub struct ErrorSyncFailed {
    /// What the exchange failed with.
    pub cause: String,
}

impl Failure for ErrorSyncFailed {
    fn name(&self) -> &'static str {
        "error_sync_failed"
    }

    fn reason(&self) -> String {
        t!("sync.err_failed", cause = self.cause).trim().to_string()
    }
}

failure!(ErrorSyncFailed);

#[renderer(buffer)]
pub fn render_error_sync_failed(error: ErrorSyncFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("sync.err_failed_help").trim()));
    ec.exit_code = EC_ERR_SYNC;
}

/// Error: `rola sync` was given arguments it cannot use.
#[derive(Grouped)]
pub struct ErrorSyncArgument;

impl Failure for ErrorSyncArgument {
    fn name(&self) -> &'static str {
        "error_sync_argument"
    }

    fn reason(&self) -> String {
        t!("sync.err_argument").trim().to_string()
    }
}

failure!(ErrorSyncArgument);

#[renderer(buffer)]
pub fn render_error_sync_argument(error: ErrorSyncArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("sync.err_argument_help").trim()));
    ec.exit_code = EC_ERR_SYNC_ARGUMENT;
}

/// Error: a direction was named beside `--no-layout`, which has none to narrow.
///
/// It is refused rather than ignored: a run that reads no Layout exchanges the index both ways and
/// uploads the stored content whichever direction was asked for, so a `--down-only` that was quietly
/// dropped would do the opposite of what it said.
#[derive(Grouped)]
pub struct ErrorSyncNoLayoutDirection;

impl Failure for ErrorSyncNoLayoutDirection {
    fn name(&self) -> &'static str {
        "error_sync_no_layout_direction"
    }

    fn reason(&self) -> String {
        t!("sync.err_no_layout_direction").trim().to_string()
    }
}

failure!(ErrorSyncNoLayoutDirection);

#[renderer(buffer)]
pub fn render_error_sync_no_layout_direction(
    error: ErrorSyncNoLayoutDirection,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("sync.err_no_layout_direction_help").trim())
    );
    ec.exit_code = EC_ERR_SYNC_ARGUMENT;
}

/// Result: a plan was made.
// What a plan says of each part is a flag, and `--json` writes the four by name: grouping them
// would change what a program reads for no gain a reader of the plan would see.
#[allow(clippy::struct_excessive_bools)]
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultSyncPlan {
    /// The Layout the plan is about.
    layout: String,
    /// The Vault it tracks.
    vault: String,
    /// Whether the run was told to change history rather than refuse.
    forced: bool,
    /// What is to move, in path order.
    entries: Vec<Entry>,
    /// How many entries both sides already hold at the same version.
    up_to_date: usize,
    /// Whether the Vault's Layout was read at all.
    ///
    /// A run that leaves the Layout behind has no plan, and is not the same answer as a plan with
    /// nothing in it: the entries and the count above say what a plan came to, and this says whether
    /// there is one. `--json` carries it so a program can tell the two apart; a reader that only has
    /// the counts cannot.
    #[serde(default)]
    layout_read: bool,
    /// Whether the index is to move, which is what `--no-index` leaves behind.
    #[serde(default)]
    index: bool,
    /// Whether the store is to move, which is what `--no-storage` leaves behind.
    #[serde(default)]
    storage: bool,
}

#[renderer(buffer)]
pub fn render_result_sync_plan(result: ResultSyncPlan) {
    for entry in &result.entries {
        let path = entry
            .local_path
            .as_deref()
            .or(entry.remote_path.as_deref())
            .unwrap_or("-");
        let remote = entry
            .remote_path
            .as_deref()
            .or(entry.planned_path.as_deref())
            .unwrap_or("-");

        r_println!(
            "{:<11} {}  ->  {}  {}",
            kind_token(entry.kind),
            path,
            remote,
            entry.uuid
        );
    }

    // A plan of nothing is not what a run that read no Layout would do, it moves the index and the
    // store whichever count is printed, so that run is told what it is about to move instead.
    if result.layout_read {
        r_println!(
            "{}",
            t!(
                "sync.result_summary",
                planned = result.entries.len(),
                up_to_date = result.up_to_date
            )
            .trim()
        );
    } else {
        r_println!(
            "{}",
            plan_without_layout(result.index, result.storage).trim()
        );
    }

    if result
        .entries
        .iter()
        .any(|entry| entry.kind == Kind::Blocked)
    {
        r_eprintln!("{}", err_line!(t!("sync.warn_blocked").trim()));
        r_eprintln!("{}", help_line!(t!("sync.warn_blocked_help").trim()));
    }
}

/// What a run that read no Layout would move, in words.
///
/// What such a run moves is fixed by `--no-layout` rather than by what a plan would name, and the two
/// parts it leaves alone can take either of the two away: the sentence says what is left, so that a
/// run told to leave everything behind is not described as though it would move something.
fn plan_without_layout(index: bool, storage: bool) -> String {
    match (index, storage) {
        (true, true) => t!("sync.plan_no_layout"),
        (true, false) => t!("sync.plan_no_layout_index"),
        (false, true) => t!("sync.plan_no_layout_storage"),
        (false, false) => t!("sync.plan_no_layout_nothing"),
    }
    .trim()
    .to_owned()
}

/// Result: a run was carried out.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultSyncDone {
    /// The Layout the run was about.
    layout: String,
    /// The Vault it tracks.
    vault: String,
    /// How many entries were written to the Vault.
    applied: usize,
    /// How many versions were brought down from the Vault.
    received: usize,
    /// How many entries both sides already held at the same version.
    up_to_date: usize,
    /// What was not done, one line each.
    failed: Vec<String>,
    /// Whether the Vault's Layout was read at all.
    ///
    /// A run that leaves the Layout behind has no plan, so the three counts above are all zero
    /// however much moved: what it did is in the two below, and which of the two is printed is what
    /// this says.
    #[serde(default)]
    layout_read: bool,
    /// How many stored objects the index named, which is what a run with no plan asked for.
    #[serde(default)]
    index_objects: usize,
    /// How many units of stored content the run sent.
    #[serde(default)]
    storage_objects: usize,
}

#[renderer(buffer)]
pub fn render_result_sync_done(result: ResultSyncDone, ec: &mut ResExitCode) {
    for cause in &result.failed {
        r_eprintln!("{}", err_line!(cause));
    }

    if result.layout_read {
        r_println!(
            "{}",
            t!(
                "sync.done_summary",
                applied = result.applied,
                received = result.received,
                up_to_date = result.up_to_date,
                failed = result.failed.len()
            )
            .trim()
        );
    } else {
        r_println!(
            "{}",
            t!(
                "sync.done_no_layout",
                index = result.index_objects,
                storage = result.storage_objects,
                failed = result.failed.len()
            )
            .trim()
        );
    }

    if !result.failed.is_empty() {
        ec.exit_code = EC_ERR_SYNC;
    }
}

/// What a plan's kind is written as in the human listing.
///
/// It is the same word `--json` writes the kind by, so a reader and a program read one name.
fn kind_token(kind: Kind) -> &'static str {
    match kind {
        Kind::Create => "create",
        Kind::Send => "send",
        Kind::Receive => "receive",
        Kind::Diverged => "diverged",
        Kind::Refused => "refused",
        Kind::Blocked => "blocked",
        Kind::RemoteOnly => "remote-only",
        Kind::UpToDate => "up-to-date",
        Kind::Mismatched => "kind-mismatch",
    }
}

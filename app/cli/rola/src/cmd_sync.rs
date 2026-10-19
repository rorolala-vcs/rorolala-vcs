//! The `rola sync` command: moving a Layout's work to and from the Vault it tracks.
//!
//! A Layout tracks one Vault — the name in its `TRACK` file — and this is how the two are brought
//! together. What the Vault holds now is fetched first, since the plan is made from it, and the
//! plan says what is to move and which way. `--dry-run` stops there: it prints the plan and does
//! nothing, which is what the run that wants to know before it is changed asks for.
//!
//! What goes up is the plan's creations and versions: a `Uuid` only this Layout holds is made an
//! entry of the Vault's Layout under `@/new/`, and a version the Vault is behind on is set. What
//! comes down is a version the Vault is ahead on: what it stored is written into the tree, and the
//! version into the Layout. Content is moved around the Layout either way — the index and the
//! store, unless `--no-index` or `--no-storage` says otherwise — and `--no-layout` leaves the
//! Layout out of it entirely: no plan is made, the index is exchanged both ways, and the stored
//! content is uploaded. A direction is a narrowing of the plan, so `--up-only` and `--down-only`
//! are refused beside it.
//!
//! A file changed here that the Vault is ahead on is refused before anything is written, so a pull
//! never overwrites what the run does not know about; merging is not here yet.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates,
// so a lint allowed on the handler below is reported as defined twice. The allow lives here,
// where it covers the one signature that needs it.
#![allow(clippy::trivially_copy_pass_by_ref)]

use std::str::FromStr as _;

use librorolala::daemon::{
    EntryWrite, RemoteWrites, action_fetch_layout, action_sync_index_all_async,
    action_write_remote_entries,
};
use librorolala::layout::{Batch, Layout, LayoutPath, MutableData};
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
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
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::{Direction as MoveDirection, Progress};
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::checkout::Checkout;
use crate::complete::{strip_written, typing_flag};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_SYNC, EC_ERR_SYNC_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutNoCurrent, ErrorLayoutNotCached,
    add_cached_entry, cached_copy, failed, readonly_layout_dir, set_cached_version,
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
/// says what is to move: a `Uuid` only this Layout holds goes up under the Vault's `@/new/`, a
/// version the Vault is behind on goes up when this account holds it, one this Layout is behind on
/// comes down, and a `Uuid` only the Vault holds is named rather than brought — `checkin` is what
/// brings one.
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

    let (up_only, down_only, both, dry_run, no_index, no_storage, no_layout) = args
        .pick(&ARG_UP_ONLY)
        .pick(&ARG_DOWN_ONLY)
        .pick(&ARG_BOTH)
        .pick(&ARG_DRY_RUN)
        .pick(&ARG_NO_INDEX)
        .pick(&ARG_NO_STORAGE)
        .pick(&ARG_NO_LAYOUT)
        .unwrap();

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

    // A run that reads no Layout has no plan for a direction to narrow: what it does is fixed —
    // the index both ways and the store up — so a direction is refused rather than dropped.
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

    // The Layout being worked in is the one the plan is about; a Layout that is not checked out is
    // not the one a run is working in.
    let layouts = held.layouts();
    let name = match layouts.current() {
        Ok(Some(name)) => name,
        Ok(None) => return ErrorLayoutNoCurrent.into(),
        Err(error) => return failed(&error),
    };
    let layout = match layouts.get(&name) {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutMissing.into(),
        Err(error) => return failed(&error),
    };

    let Some(track) = (match layouts.track(&name) {
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
    // first — the same fetch `rola layout fetch` makes. A run that is not about the Layout at all,
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
        let next = planned(plan.as_ref(), state, name, vault_name);

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
        Ok(carried) => done(plan.as_ref(), name, vault_name, carried),
        Err(error) => error.into(),
    }
}

/// What a run that was carried out answers with.
///
/// What a run with no plan did is not in the three plan counts — there was no plan — so the counts
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

    let dir = readonly_layout_dir(carry.workspace, vault_name, VAULT_LAYOUT_NAME);
    if !dir.is_dir() {
        return Err(PlanRefusal::NotCached);
    }
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
        ))
        .map_err(PlanRefusal::Failed)?;
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

/// Writes what a plan names, and moves the content that gives it meaning.
///
/// Content is moved before the Layout is touched either way: a version is only a meaning once what
/// it stored is here, and what goes up names versions the Vault has to be able to read.
///
/// The two exchanges say their own progress — they are actions, and an action reports itself — and
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
    // A plan is what names the content to move, and `--no-layout` is the run that has none. Reading
    // no Layout must not leave the content behind, though: `--no-storage` is the only thing that
    // suppresses moving content. So with no plan what moves is everything the index holds — the
    // stored object every variant points at, the new ones included — and it is read here, before the
    // index is synced, so what crosses is this side's own work rather than everything the Vault
    // holds as well.
    //
    // The list is read even when `--no-storage` leaves the store behind, because it is also the list
    // a run with no plan reports having asked for.
    let indexed = plan.is_none().then(|| indexed(carry));

    let entries = plan_entries(plan);

    let sources = sources_of(state, carry, vault_name, progress)?;

    // What moves the content, which is the plan's entries or — with no plan — everything the index
    // names. Both the orders below ask for it, so what it does is stated once.
    let carry_content = || -> Result<usize, ErrorSyncFailed> {
        let Some(sources) = &sources else {
            return Ok(0);
        };

        if let Some(keys) = &indexed {
            return sources
                .send(carry.store, keys)
                .map_err(|cause| ErrorSyncFailed { cause });
        }

        let (receive, send) = content_of(&entries, state, carry);

        if let Some(store) = carry.store {
            sources.bring(store, &receive);
        }

        sources
            .send(carry.store, &send)
            .map_err(|cause| ErrorSyncFailed { cause })
    };

    // Which of the two exchanges goes first is decided by what names what. With no plan the Vault's
    // index is about to name variants this run sends, so the content goes first: a failed upload
    // then leaves content nobody names — harmless, since a key is its content — rather than a name
    // pointing at nothing until a later run repairs it. With a plan it is the other way round: a
    // version to bring down is resolved through index objects only the Vault holds, so the index has
    // to cross before its content can be found.
    let storage_objects;
    if plan.is_none() {
        storage_objects = carry_content()?;
        sync_index(carry, state, progress)?;
    } else {
        sync_index(carry, state, progress)?;
        storage_objects = carry_content()?;
    }

    // What a run with no plan asked the Vault for is what the index named: the content every variant
    // points at. The index objects themselves cross in an exchange of their own, which answers with
    // no count — the action is already exported to C with a `()` output — so what is reported is the
    // list this run made, not what the Vault answered with.
    let index_objects = if state.skip.index {
        0
    } else {
        indexed.as_ref().map_or(0, Vec::len)
    };

    let up_total = count(&entries, false);
    let down_total = count(&entries, true);

    let mut moves = progress.begin("", Some(up_total + down_total));
    let mut up = (up_total > 0).then(|| moves.spawn(MoveDirection::Up, "", Some(up_total)));
    let mut down = (down_total > 0).then(|| moves.spawn(MoveDirection::Down, "", Some(down_total)));

    let mut applied = 0;
    let mut received = 0;
    let mut failed = Vec::new();

    // Every path a pull writes is remembered in the one reading, which is written once at the end
    // rather than read back and rewritten for each file brought down; the version each pulled
    // entry names is written to the Layout in one batch for the same reason.
    let mut checkout = Checkout::open(carry.layout, carry.workspace.get_root());
    let mut batch = carry.layout.batch();

    // What this side is ahead on is gathered and written in one exchange at the end, rather than
    // one connection and handshake for each entry.
    let mut ups: Vec<&Entry> = Vec::new();

    for entry in entries.iter().copied() {
        if entry.kind == Kind::Receive {
            if let Some(task) = &mut down {
                let _named = task.doing(name_of(entry));
            }

            received += usize::from(receive(
                entry,
                carry,
                &mut checkout,
                &mut batch,
                &mut failed,
            )?);

            if let Some(task) = &mut down {
                task.advance_by(1);
            }

            moves.advance_by(1);
        } else {
            ups.push(entry);
        }
    }

    apply_entries(
        &ups,
        state.forced,
        carry,
        vault_name,
        account_name,
        &mut applied,
        &mut failed,
    )?;

    // What is written up is one exchange of many entries rather than a step each, so its share of
    // the bars is taken once it is done: the whole bar must not read as finished while the exchange
    // — the step that reaches the network — is still in front of the reader.
    if let Some(task) = &mut up {
        task.advance_by(up_total);
    }
    moves.advance_by(up_total);

    if let Err(cause) = checkout.finish() {
        failed.push(cause);
    }

    if let Err(error) = batch.commit() {
        failed.push(error.to_string());
    }

    if let Some(sources) = &sources {
        sources.report();
    }

    // A run that read no Layout left this side's copy of what the Vault holds exactly as it found
    // it, which is what the reading commands go on answering from until something fetches. Saying so
    // is what keeps a reader from taking a version graph for the Vault's current state.
    if plan.is_none() {
        eprintln!("{}", warn_line!(t!("sync.warn_layout_not_fetched").trim()));
    }

    Ok(Carried {
        applied,
        received,
        failed,
        index_objects,
        storage_objects,
    })
}

/// The entries a plan says are to move.
///
/// The content is asked for one object at a time, by what the plan names: an object a run needs is
/// fetched, and one it does not is never reached for. An entry both sides already hold at the same
/// version, and one only the Vault holds, names nothing this run moves.
fn plan_entries(plan: Option<&sync::Plan>) -> Vec<&Entry> {
    plan.map_or_else(Vec::new, |plan| {
        plan.entries
            .iter()
            .filter(|entry| !matches!(entry.kind, Kind::UpToDate | Kind::RemoteOnly))
            .collect()
    })
}

/// The Vaults a run may send content to, or `None` when `--no-storage` leaves the store alone.
///
/// # Errors
///
/// Returns [`ErrorSyncFailed`] when the runtime the exchanges are waited on could not be made.
fn sources_of<'a>(
    state: StateSync,
    carry: &Carry<'a>,
    vault_name: &str,
    progress: &Progress,
) -> Result<Option<fetch::Sources<'a>>, ErrorSyncFailed> {
    if state.skip.storage {
        return Ok(None);
    }

    let primary = fetch::primary(carry.remote, Some(vault_name));
    // Offline is not passed through: `rola sync` is refused before it gets here, since the exchange
    // with the Vault is the whole of what it does.
    let sources = fetch::Sources::new(carry.workspace, carry.account, carry.remote, primary, false)
        .map_err(|cause| ErrorSyncFailed { cause })?
        .reporting(progress.clone())
        .with_concurrency(state.concurrency);

    Ok(Some(sources))
}

/// What carrying a plan out came to.
pub struct Carried {
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

/// Makes the two indexes hold the same objects, unless the run leaves the index alone.
///
/// # Errors
///
/// Returns [`ErrorSyncFailed`] when the exchange could not be made.
fn sync_index(
    carry: &Carry<'_>,
    state: StateSync,
    progress: &Progress,
) -> Result<(), ErrorSyncFailed> {
    if state.skip.index {
        return Ok(());
    }

    carry
        .runtime
        .block_on(action_sync_index_all_async(
            carry.workspace,
            carry.account,
            carry.target.to_string(),
            // The exchange has one answer whichever side asks: a full sync is everything either side
            // holds, and an input names nothing — see `ActionSyncIndexAll`.
            String::new(),
            progress.clone(),
        ))
        .map_err(|error| ErrorSyncFailed {
            cause: error.to_string(),
        })
}

/// Every stored object the index names, for a run with no plan to name content by.
///
/// A variant is meaningful only once what it stored is beside it, and a run that read no Layout has
/// no list of versions to work from: what the index holds is the list. Every variant's content is
/// named, so the new variants a run just made have theirs carried the same way the old ones do.
///
/// The scan is the index's own rather than a listing of every object read whole: only the Variants
/// are decoded, since only they name content, and the other kinds are skipped by the kind their
/// bytes lead with.
fn indexed(carry: &Carry<'_>) -> Vec<Key> {
    let Ok(hashes) = carry.runtime.block_on(carry.index.variant_storage_hashes()) else {
        return Vec::new();
    };

    let mut keys: Vec<Key> = hashes.into_iter().map(Key::new).collect();
    keys.sort_unstable();
    keys.dedup();

    keys
}

/// The content a plan needs moved: the objects a pull writes here, and the objects a push puts in
/// the Vault's store.
///
/// A version is only a meaning once what it stored is in the store that names it, so each entry
/// names one object on the side it is about — a pull the Vault's version, a push this side's. An
/// entry that is neither — a refusal, a block, or a `Diverged` a run is not forcing — names nothing
/// to move, and its content is never asked for.
fn content_of(entries: &[&Entry], state: StateSync, carry: &Carry<'_>) -> (Vec<Key>, Vec<Key>) {
    let mut receive = Vec::new();
    let mut send = Vec::new();

    for entry in entries {
        let pull = entry.kind == Kind::Receive;
        let push = match entry.kind {
            Kind::Create | Kind::Send => true,
            Kind::Diverged => state.forced,
            _ => false,
        };

        if !pull && !push {
            continue;
        }

        let named = if pull {
            entry.remote_version.as_deref()
        } else {
            entry.local_version.as_deref()
        };
        let Some(version) = named.and_then(|text| Key::from_str(text).ok()) else {
            continue;
        };
        let Ok(stored) = carry
            .runtime
            .block_on(sync::store_of(carry.index, version.digest()))
        else {
            continue;
        };
        let Some(stored) = stored else {
            continue;
        };

        if pull {
            receive.push(stored);
        } else {
            send.push(stored);
        }
    }

    (receive, send)
}

/// How many of `entries` move down, or up when `down` is false.
fn count(entries: &[&Entry], down: bool) -> u64 {
    let moved = entries
        .iter()
        .filter(|entry| (entry.kind == Kind::Receive) == down)
        .count();

    u64::try_from(moved).unwrap_or(u64::MAX)
}

/// The name an entry is shown by while it is being moved.
fn name_of(entry: &Entry) -> &str {
    entry
        .local_path
        .as_deref()
        .or(entry.remote_path.as_deref())
        .or(entry.planned_path.as_deref())
        .unwrap_or_default()
}

/// Why a run could not make a plan.
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
fn receive(
    entry: &Entry,
    carry: &Carry<'_>,
    checkout: &mut Checkout,
    batch: &mut Batch<'_>,
    failed: &mut Vec<String>,
) -> Result<bool, ErrorSyncFailed> {
    let (Some(text), Ok(id)) = (entry.remote_version.as_deref(), Uuid::from_str(&entry.uuid))
    else {
        failed.push(format!(
            "{}: {}",
            entry.uuid,
            t!("sync.write_malformed").trim()
        ));

        return Ok(false);
    };

    let Ok(version) = Key::from_str(text) else {
        failed.push(format!(
            "{}: {}",
            entry.uuid,
            t!("sync.write_malformed").trim()
        ));

        return Ok(false);
    };

    // What the version names has to be here before the tree can be given it.
    let stored = match carry
        .runtime
        .block_on(sync::store_of(carry.index, version.digest()))
    {
        Ok(stored) => stored,
        Err(cause) => return Err(ErrorSyncFailed { cause }),
    };

    if let Some(path) = entry.local_path.as_deref() {
        let (Some(stored), Some(store)) = (stored, carry.store) else {
            failed.push(format!(
                "{}: {}",
                entry.uuid,
                t!("sync.write_no_content").trim()
            ));

            return Ok(false);
        };

        let target = carry.workspace.get_root().join(path);

        if let Some(parent) = target.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            failed.push(format!("{}: {error}", entry.uuid));

            return Ok(false);
        }

        if let Err(error) = carry.runtime.block_on(store.extract_file(&stored, &target)) {
            failed.push(format!("{}: {error}", entry.uuid));

            return Ok(false);
        }

        // What a pull wrote is what the Layout now names, so the reading has to be told; a path
        // left marked as disagreed would be reported as changed by the very next run.
        if let Err(cause) = checkout.remember(path) {
            failed.push(format!("{}: {cause}", entry.uuid));

            return Ok(false);
        }
    }

    let Some(data) = carry.layout.entry(id) else {
        failed.push(format!(
            "{}: {}",
            entry.uuid,
            t!("sync.write_missing").trim()
        ));

        return Ok(false);
    };

    if let Err(error) = batch.update_entry(
        id,
        MutableData::new(
            entry.owner.clone(),
            *version.digest(),
            data.description().to_owned(),
        ),
    ) {
        failed.push(format!("{}: {error}", entry.uuid));

        return Ok(false);
    }

    Ok(true)
}

/// Writes what the Vault is behind on, in one exchange, counting each done or naming why it was not.
///
/// A run ahead on many entries asks for them all at once: each would otherwise be its own connection
/// and handshake, and the writes have nothing to say to one another. What the Vault takes is written
/// into the fetched copy as well, so that a reading command sees the Vault as this run left it rather
/// than as it found it. That copy is what `--layout truth@<VAULT>` reads, and one that lagged behind
/// would make a run look as though it had not happened.
///
/// # Errors
///
/// Returns the exchange's own failure, or the outcomes that will not read.
fn apply_entries(
    ups: &[&Entry],
    forced: bool,
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
    applied: &mut usize,
    failed: &mut Vec<String>,
) -> Result<(), ErrorSyncFailed> {
    // What is asked for, paired with the entry it is asked for, so an answer can be put back to it.
    let mut creates: Vec<(String, String, String)> = Vec::new();
    let mut create_entries: Vec<&Entry> = Vec::new();
    let mut sets: Vec<(String, String)> = Vec::new();
    let mut setting: Vec<&Entry> = Vec::new();

    for entry in ups {
        match entry.kind {
            Kind::Create => {
                let (Some(path), Some(version)) = (
                    entry.planned_path.as_deref(),
                    entry.local_version.as_deref(),
                ) else {
                    continue;
                };

                creates.push((path.to_owned(), entry.uuid.clone(), version.to_owned()));
                create_entries.push(entry);
            }
            Kind::Send | Kind::Diverged => {
                if entry.kind == Kind::Diverged && !forced {
                    failed.push(format!("{}: {}", entry.uuid, t!("sync.write_force").trim()));

                    continue;
                }

                let Some(version) = entry.local_version.as_deref() else {
                    continue;
                };

                sets.push((entry.uuid.clone(), version.to_owned()));
                setting.push(entry);
            }
            Kind::Refused => {
                failed.push(format!(
                    "{}: {}",
                    entry.uuid,
                    t!("sync.write_refused").trim()
                ));
            }
            _ => {}
        }
    }

    if creates.is_empty() && sets.is_empty() {
        return Ok(());
    }

    let writes = serde_json::to_string(&RemoteWrites { creates, sets }).map_err(|error| {
        ErrorSyncFailed {
            cause: error.to_string(),
        }
    })?;

    let answer = action_write_remote_entries(
        carry.workspace,
        carry.account,
        carry.target.to_string(),
        writes,
    )
    .map_err(|error| ErrorSyncFailed {
        cause: error.to_string(),
    })?;

    let outcomes: Vec<EntryWrite> =
        serde_json::from_str(&answer).map_err(|error| ErrorSyncFailed {
            cause: error.to_string(),
        })?;

    // The outcomes come back in the order asked for: the creates first, then the sets.
    let mut outcomes = outcomes.into_iter();
    let mut written: Vec<&Entry> = Vec::new();
    for &entry in &create_entries {
        let Some(outcome) = outcomes.next() else {
            break;
        };

        record(entry, outcome, &mut written, applied, failed);
    }
    for &entry in &setting {
        let Some(outcome) = outcomes.next() else {
            break;
        };

        record(entry, outcome, &mut written, applied, failed);
    }

    write_fetched_copy(&written, carry, vault_name, account_name, failed);

    Ok(())
}

/// Puts one answer back to the entry it belongs to.
fn record<'a>(
    entry: &'a Entry,
    outcome: EntryWrite,
    written: &mut Vec<&'a Entry>,
    applied: &mut usize,
    failed: &mut Vec<String>,
) {
    match outcome {
        EntryWrite::Written => {
            *applied += 1;
            written.push(entry);
        }
        other => failed.push(format!("{}: {}", entry.uuid, write_reason(other))),
    }
}

/// Writes down in the fetched copy what the Vault took, in one go.
///
/// The copy is opened once and every change is gathered into one batch, so a run that writes many
/// entries reads and writes the copy once rather than once per entry.
fn write_fetched_copy(
    entries: &[&Entry],
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
    failed: &mut Vec<String>,
) {
    if entries.is_empty() {
        return;
    }

    let copy = match cached_copy(carry.workspace, vault_name) {
        Ok(Some(copy)) => copy,
        Ok(None) => return,
        Err(error) => {
            failed.push(error.to_string());

            return;
        }
    };

    let mut batch = copy.batch();
    for entry in entries {
        if let Err(cause) = remember_written(&mut batch, entry, account_name) {
            failed.push(format!("{}: {cause}", entry.uuid));
        }
    }

    if let Err(error) = batch.commit() {
        failed.push(error.to_string());
    }
}

/// Writes down in the fetched copy what the Vault just took.
///
/// A new `Uuid` is added at the name it was made under, held by this account; one the Vault was
/// behind on has its version moved. Nothing else about the copy changes, since nothing else about
/// the Vault did.
///
/// # Errors
///
/// Returns a message when the entry does not read or the change cannot be made in `copy`.
fn remember_written(copy: &mut Batch<'_>, entry: &Entry, account_name: &str) -> Result<(), String> {
    let Ok(id) = Uuid::from_str(&entry.uuid) else {
        return Err(t!("sync.write_malformed").trim().to_owned());
    };
    let Some(version) = entry.local_version.as_deref() else {
        return Ok(());
    };
    let Ok(version) = Key::from_str(version) else {
        return Err(t!("sync.write_malformed").trim().to_owned());
    };

    match entry.kind {
        Kind::Create => {
            let Some(path) = entry.planned_path.as_deref() else {
                return Ok(());
            };
            let Ok(path) = LayoutPath::new(path) else {
                return Err(t!("sync.write_malformed").trim().to_owned());
            };

            add_cached_entry(
                copy,
                id,
                &path,
                *version.digest(),
                Some(account_name.to_owned()),
            )
            .map_err(|error| error.to_string())
        }
        Kind::Send | Kind::Diverged => {
            set_cached_version(copy, id, *version.digest()).map_err(|error| error.to_string())
        }
        _ => Ok(()),
    }
}

/// What did not happen to one entry, in words.
fn write_reason(outcome: EntryWrite) -> String {
    match outcome {
        EntryWrite::Written => t!("sync.write_written").trim().to_string(),
        EntryWrite::Exists => t!("sync.write_exists").trim().to_string(),
        EntryWrite::Taken => t!("sync.write_taken").trim().to_string(),
        EntryWrite::Missing => t!("sync.write_missing").trim().to_string(),
        EntryWrite::Refused => t!("sync.write_refused").trim().to_string(),
        EntryWrite::Malformed => t!("sync.write_malformed").trim().to_string(),
    }
}

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

    // A plan of nothing is not what a run that read no Layout would do — it moves the index and the
    // store whichever count is printed — so that run is told what it is about to move instead.
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
    }
}

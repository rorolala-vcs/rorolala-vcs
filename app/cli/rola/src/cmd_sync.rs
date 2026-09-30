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
//! Layout out of it, moving only data and index.
//!
//! A file changed here that the Vault is ahead on is refused before anything is written, so a pull
//! never overwrites what the run does not know about; merging is not here yet.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates,
// so a lint allowed on the handler below is reported as defined twice. The allow lives here,
// where it covers the one signature that needs it.
#![allow(clippy::trivially_copy_pass_by_ref)]

use std::str::FromStr as _;

use librorolala::daemon::{
    EntryWrite, action_create_remote_entry, action_fetch_layout, action_set_remote_version,
    action_sync_all_async, action_sync_index_all_async,
};
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResCurrentRemoteVault, ResProgressSetting, ResRorolalaStorage, ResVCSIndex, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::{Direction as MoveDirection, Progress};
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::checkout::remember;
use crate::exit_codes::{EC_ERR_SYNC, EC_ERR_SYNC_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutNotCached,
    add_cached_entry, failed, readonly_layout_dir, set_cached_version,
};
use crate::progress::Reporting;
use crate::sync::{self, Direction, Entry, Kind};
use crate::vcs_index::{ErrorVcsIndexNoIndex, runtime as index_runtime};

/// The flags `rola sync` takes.
#[derive(Pickable)]
struct SyncFlags {
    /// Only what this Layout holds goes up.
    #[arg(long)]
    up_only: Flag,
    /// Only what the Vault holds comes down.
    #[arg(long)]
    down_only: Flag,
    /// Both directions; the one taken when neither is named.
    #[arg(long)]
    both: Flag,
    /// Change history rather than refuse when the versions have moved apart.
    #[arg(long)]
    force: Flag,
    /// Show the plan and change nothing.
    #[arg(long)]
    dry_run: Flag,
    /// Leave the index alone; only the Layout and the store move.
    #[arg(long)]
    no_index: Flag,
    /// Leave the store alone; only the Layout and the index move.
    #[arg(long)]
    no_storage: Flag,
    /// Leave the Vault's Layout alone; only data and index move.
    #[arg(long)]
    no_layout: Flag,
}

#[help(buffer)]
pub fn help_sync(_: EntrySync, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("sync.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntrySync)]
pub fn desc_sync() -> Description {
    t!("sync.description").to_string().into()
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
/// moved: the index and the store, unless `--no-index` or `--no-storage` leaves one behind, and
/// `--no-layout` leaves the Layout itself behind, so only data and index move.
///
/// # Errors
///
/// Renders [`ErrorSyncNoTrack`] when the Layout tracks no Vault, [`ErrorSyncBlocked`] when a file
/// changed here is behind the Vault, [`ErrorSyncFailed`] when the exchange could not be made, and
/// the exchange's own failures otherwise.
#[command(node = "sync", entry = EntrySync)]
pub fn sync(args: EntrySync) -> Next {
    let flags = args.pick(&arg![SyncFlags]).unwrap();

    let up = matches!(flags.up_only, Flag::Active);
    let down = matches!(flags.down_only, Flag::Active);
    let both = matches!(flags.both, Flag::Active);

    // At most one direction may be named; naming none is both.
    if [up, down, both].into_iter().filter(|named| *named).count() > 1 {
        return ErrorSyncArgument.into();
    }

    let direction = if up {
        Direction::Up
    } else if down {
        Direction::Down
    } else {
        Direction::Both
    };

    StateSync {
        direction,
        forced: matches!(flags.force, Flag::Active),
        dry_run: matches!(flags.dry_run, Flag::Active),
        skip: Skip {
            index: matches!(flags.no_index, Flag::Active),
            storage: matches!(flags.no_storage, Flag::Active),
            layout: matches!(flags.no_layout, Flag::Active),
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
    /// Whether the Vault's Layout is left alone, leaving only data and index.
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
        Ok(None) => return ErrorLayoutArgument.into(),
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
        runtime: &runtime,
    };

    // What the Vault holds now is what the plan is made from, so its copy is brought up to date
    // first — the same fetch `rola layout fetch` makes. A run that is not about the Layout at all,
    // which `--no-layout` says, makes no plan and reads no copy.
    //
    // One watching covers the whole run: the two steps that make the plan and every step that
    // carries it out are one thing to a reader, and a bar that waited on each part on its own would
    // never have been let out at all.
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
        Ok((applied, received, failed)) => ResultSyncDone {
            layout: name,
            vault: vault_name,
            applied,
            received,
            up_to_date: plan.as_ref().map_or(0, |plan| plan.up_to_date),
            failed,
        }
        .into(),
        Err(error) => error.into(),
    }
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
    let remote_layout =
        Layout::open(&dir).map_err(|error| PlanRefusal::Failed(error.to_string()))?;

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
) -> Result<(usize, usize, Vec<String>), ErrorSyncFailed> {
    if !state.skip.index
        && let Err(error) = carry.runtime.block_on(action_sync_index_all_async(
            carry.workspace,
            carry.account,
            carry.target.to_string(),
            String::new(),
            progress.clone(),
        ))
    {
        return Err(ErrorSyncFailed {
            cause: error.to_string(),
        });
    }

    if !state.skip.storage
        && let Err(error) = carry.runtime.block_on(action_sync_all_async(
            carry.workspace,
            carry.account,
            carry.target.to_string(),
            String::new(),
            progress.clone(),
        ))
    {
        return Err(ErrorSyncFailed {
            cause: error.to_string(),
        });
    }

    let entries: Vec<&Entry> = plan.map_or_else(Vec::new, |plan| {
        plan.entries
            .iter()
            .filter(|entry| !matches!(entry.kind, Kind::UpToDate | Kind::RemoteOnly))
            .collect()
    });

    let up_total = count(&entries, false);
    let down_total = count(&entries, true);

    let mut moves = progress.begin("", Some(up_total + down_total));
    let mut up = (up_total > 0).then(|| moves.spawn(MoveDirection::Up, "", Some(up_total)));
    let mut down = (down_total > 0).then(|| moves.spawn(MoveDirection::Down, "", Some(down_total)));

    let mut applied = 0;
    let mut received = 0;
    let mut failed = Vec::new();

    for entry in entries {
        if entry.kind == Kind::Receive {
            if let Some(task) = &mut down {
                let _named = task.doing(name_of(entry));
            }

            received += usize::from(receive(entry, carry, &mut failed)?);

            if let Some(task) = &mut down {
                task.advance_by(1);
            }
        } else {
            if let Some(task) = &mut up {
                let _named = task.doing(name_of(entry));
            }

            apply_entry(
                entry,
                state.forced,
                carry,
                vault_name,
                account_name,
                &mut applied,
                &mut failed,
            )?;

            if let Some(task) = &mut up {
                task.advance_by(1);
            }
        }

        moves.advance_by(1);
    }

    Ok((applied, received, failed))
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
        if let Err(cause) = remember(carry.layout, carry.workspace.get_root(), path) {
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

    if let Err(error) = carry.layout.update_entry(
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

/// Writes what one planned entry says, counting it done or naming why it was not.
///
/// What the Vault takes is written into the fetched copy as well, so that a reading command sees
/// the Vault as this run left it rather than as it found it. That copy is what `--layout
/// truth@<VAULT>` reads, and one that lagged behind would make a run look as though it had not
/// happened.
///
/// # Errors
///
/// Returns the exchange's own failure, or the outcome that will not read.
fn apply_entry(
    entry: &Entry,
    forced: bool,
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
    applied: &mut usize,
    failed: &mut Vec<String>,
) -> Result<(), ErrorSyncFailed> {
    let answer = match entry.kind {
        Kind::Create => {
            let (Some(path), Some(version)) = (
                entry.planned_path.as_deref(),
                entry.local_version.as_deref(),
            ) else {
                return Ok(());
            };

            action_create_remote_entry(
                carry.workspace,
                carry.account,
                carry.target.to_string(),
                path.to_owned(),
                entry.uuid.clone(),
                version.to_owned(),
            )
            .map_err(|error| ErrorSyncFailed {
                cause: error.to_string(),
            })?
        }
        Kind::Send | Kind::Diverged => {
            if entry.kind == Kind::Diverged && !forced {
                failed.push(format!("{}: {}", entry.uuid, t!("sync.write_force").trim()));

                return Ok(());
            }

            let Some(version) = entry.local_version.as_deref() else {
                return Ok(());
            };

            action_set_remote_version(
                carry.workspace,
                carry.account,
                carry.target.to_string(),
                entry.uuid.clone(),
                version.to_owned(),
            )
            .map_err(|error| ErrorSyncFailed {
                cause: error.to_string(),
            })?
        }
        Kind::Refused => {
            failed.push(format!(
                "{}: {}",
                entry.uuid,
                t!("sync.write_refused").trim()
            ));

            return Ok(());
        }
        _ => return Ok(()),
    };

    match serde_json::from_str::<EntryWrite>(&answer) {
        Ok(EntryWrite::Written) => {
            *applied += 1;

            if let Err(cause) = remember_written(entry, carry, vault_name, account_name) {
                failed.push(format!("{}: {cause}", entry.uuid));
            }
        }
        Ok(other) => failed.push(format!("{}: {}", entry.uuid, write_reason(other))),
        Err(error) => {
            return Err(ErrorSyncFailed {
                cause: error.to_string(),
            });
        }
    }

    Ok(())
}

/// Writes down in the fetched copy what the Vault just took.
///
/// A new `Uuid` is added at the name it was made under, held by this account; one the Vault was
/// behind on has its version moved. Nothing else about the copy changes, since nothing else about
/// the Vault did.
///
/// # Errors
///
/// Returns a message when the entry does not read or the copy could not be written.
fn remember_written(
    entry: &Entry,
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
) -> Result<(), String> {
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
                carry.workspace,
                vault_name,
                id,
                &path,
                *version.digest(),
                Some(account_name.to_owned()),
            )
            .map_err(|error| error.to_string())
        }
        Kind::Send | Kind::Diverged => {
            set_cached_version(carry.workspace, vault_name, id, *version.digest())
                .map_err(|error| error.to_string())
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

/// Result: a plan was made.
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

    r_println!(
        "{}",
        t!(
            "sync.result_summary",
            planned = result.entries.len(),
            up_to_date = result.up_to_date
        )
        .trim()
    );

    if result
        .entries
        .iter()
        .any(|entry| entry.kind == Kind::Blocked)
    {
        r_eprintln!("{}", err_line!(t!("sync.warn_blocked").trim()));
        r_eprintln!("{}", help_line!(t!("sync.warn_blocked_help").trim()));
    }
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
}

#[renderer(buffer)]
pub fn render_result_sync_done(result: ResultSyncDone, ec: &mut ResExitCode) {
    for cause in &result.failed {
        r_eprintln!("{}", err_line!(cause));
    }

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

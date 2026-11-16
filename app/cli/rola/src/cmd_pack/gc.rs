//! The collecting half of `rola pack`: what `--garbage-collect` does, and nothing else.
//!
//! The roots are the Layout the run stands in: a Vault keeps its truth Layout, a Workspace the one
//! Layout it works in, and a run in neither has no roots at all, which is refused rather than
//! guessed at, since a collecting without roots takes everything away. What the roots keep is
//! worked out first, shown for the confirmation, and only then taken away, index first and store
//! after, so a collecting that stops halfway leaves content behind that nothing names rather than
//! a reference with nothing under it.

use librorolala::storage::{CollectPlan as StoragePlan, Lockable as _};
use librorolala::vcs::CollectPlan as IndexPlan;
use librorolala::workspace::Merging;
use mingling::{
    Grouped,
    confirm::YesConfirm,
    macros::{buffer, r_eprintln, r_println, renderer},
    res::{ResConfirm, ResExitCode},
};
use rorolala_cli_setups::{
    ResProgressSetting, ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_CANCELLED, EC_ERR_PACK_GC_FAILED, EC_ERR_PACK_GC_MERGING, EC_ERR_PACK_GC_NO_LAYOUT,
};
use crate::failure::failure;
use crate::progress::Reporting;

/// Collects what the Layout does not reference, from the index first and the store after.
///
/// This is the whole of `--garbage-collect`: the roots are read, the closure is worked out, the
/// plan is shown and asked about, and what was agreed to is taken away. Which sides are swept is
/// what the state says; both stores are read either way, since the closure needs the index to know
/// what content is wanted and the store to know what a tree's structure names.
pub fn collect(
    state: super::StatePack,
    storage: &ResRorolalaStorage,
    index: &ResVCSIndex,
    vault: &ResVault,
    workspace: &ResWorkspace,
    force: bool,
    confirm: ResConfirm,
    progress_setting: ResProgressSetting,
) -> Next {
    // The roots are the Layout the run stands in, and a run in neither a Vault nor a Workspace has
    // none, which is a refusal rather than an empty answer, since collecting against no roots would
    // take everything away.
    let layout = match roots_of(vault, workspace) {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorGcNoLayout.into(),
        Err(cause) => return ErrorGcFailed { cause }.into(),
    };

    // A Workspace with merges waiting keeps variants no chain names, which is the whole of what a
    // collecting would take away from it. The records are this run's to respect, so they are asked
    // before anything is enumerated.
    if let Some(held) = workspace.as_ref()
        && vault.as_ref().is_none()
        && Merging::read(held.get_root()).iter().next().is_some()
    {
        return ErrorGcMerging.into();
    }

    // Both stores are read even when only one is swept: the closure needs the index to know what
    // content is wanted, and the store to know what a tree's structure names.
    let (Some(store), Some(vcs)) = (storage.as_ref(), index.as_ref()) else {
        return super::ErrorPackNoStorage.into();
    };

    let reporting = Reporting::start(progress_setting);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            reporting.finish();

            return ErrorGcFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    // What the roots keep, worked out before anything is asked: a run that cannot see the whole of
    // what is wanted takes nothing away, so a failure here is the end of the collecting. The
    // `Progress` handed in is a fresh clone per call, dropped when the call returns: a clone held
    // past `finish` would keep the channel open and the reader waiting on it forever.
    let live = match runtime.block_on(librorolala::gc::live_of(
        &layout,
        vcs,
        store,
        &reporting.progress(),
    )) {
        Ok(live) => live,
        Err(failure) => {
            reporting.finish();

            return ErrorGcFailed {
                cause: failure.to_string(),
            }
            .into();
        }
    };

    // The plans are read-only, so they are made outside the locks and shown for the confirmation.
    let plans = match plans_of(&runtime, vcs, store, &live, state) {
        Ok(plans) => plans,
        Err(cause) => {
            reporting.finish();

            return ErrorGcFailed { cause }.into();
        }
    };

    // Both stores are changed by this, so it is asked for rather than assumed, and the numbers the
    // plans made are what the caller is asked about. `--force` is the answer given in advance.
    if !force && !confirm.ask::<YesConfirm>(&confirm_of(&plans)) {
        reporting.finish();

        return ResultGcDeclined.into();
    }

    // The index goes first: a collecting that stops after it leaves content behind that nothing
    // names, which the next collecting takes, where the other order would leave references with
    // nothing under them.
    let taken = match sweep(&runtime, vcs, store, &plans, &reporting.progress()) {
        Ok(taken) => taken,
        Err(SweepFailure::Locked(cause)) => {
            reporting.finish();

            return super::ErrorPackLocked { cause }.into();
        }
        Err(SweepFailure::Failed(cause)) => {
            reporting.finish();

            return ErrorGcFailed { cause }.into();
        }
    };

    reporting.finish();

    ResultGc {
        index: taken.index.map(|report| {
            (
                report.removed(),
                report.removed_len(),
                report.kept_by_grace(),
                report.changed(),
            )
        }),
        storage: taken.storage.map(|report| {
            (
                report.removed(),
                report.removed_len(),
                report.kept_by_grace(),
                report.changed(),
            )
        }),
    }
    .into()
}

/// The Layout the run stands in, as the roots of the collecting.
///
/// A Vault's truth Layout comes first, the Workspace's one Layout second, and a run in neither has
/// none. A Layout that is there but does not read is a failure rather than no roots at all: what it
/// says is what would have been kept, and guessing past it would take what it names away. One that
/// is not there yet is made, the way every command that works in a Workspace makes it: an empty
/// Layout keeps nothing, so the collecting is one that takes what the confirmation agrees to, and
/// the grace holds what was written without a reference yet.
fn roots_of(
    vault: &ResVault,
    workspace: &ResWorkspace,
) -> Result<Option<librorolala::layout::Layout>, String> {
    if let Some(held) = vault.as_ref() {
        return held.layout().map(Some).map_err(|error| error.to_string());
    }

    if let Some(held) = workspace.as_ref() {
        return held.layout().map(Some).map_err(|error| error.to_string());
    }

    Ok(None)
}

/// The plans both sides would carry out, read-only, made outside the locks.
///
/// A plan is carried out against the stores as they stand when the sweep runs, so what arrives
/// between this and the sweep is kept and what is gone is skipped.
fn plans_of(
    runtime: &tokio::runtime::Runtime,
    vcs: &librorolala::vcs::VCSIndex,
    store: &librorolala::storage::RorolalaStorage,
    live: &librorolala::gc::Live,
    state: super::StatePack,
) -> Result<Plans, String> {
    let index = if state.index {
        Some(
            runtime
                .block_on(vcs.collect_plan(live.index()))
                .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };

    let storage = if state.storage {
        Some(
            runtime
                .block_on(store.collect_plan(live.objects(), live.manifests()))
                .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };

    Ok(Plans { index, storage })
}

/// The numbers the confirmation is asked about, taken from whichever plans were made.
fn confirm_of(plans: &Plans) -> String {
    let (index_entries, index_bytes) = plans
        .index
        .as_ref()
        .map_or((0, 0), |plan| (plan.removed(), plan.removed_len()));
    let (storage_entries, storage_bytes) = plans
        .storage
        .as_ref()
        .map_or((0, 0), |plan| (plan.removed(), plan.removed_len()));

    t!(
        "pack.gc_confirm",
        index_entries = index_entries,
        index_bytes = index_bytes,
        storage_entries = storage_entries,
        storage_bytes = storage_bytes,
    )
    .trim()
    .to_owned()
}

/// The plans a collecting carries out, one per side it sweeps.
struct Plans {
    /// What the index sweep would take away, when the index is swept.
    index: Option<IndexPlan>,
    /// What the store sweep would take away, when the store is swept.
    storage: Option<StoragePlan>,
}

/// Why a sweep failed, and what it failed with.
enum SweepFailure {
    /// One of the stores was locked by another run.
    Locked(String),
    /// The sweep itself could not be carried out.
    Failed(String),
}

/// What a sweep took away, by the side it swept.
struct Taken {
    /// The index sweep's report, when the index was swept.
    index: Option<librorolala::vcs::CollectReport>,
    /// The store sweep's report, when the store was swept.
    storage: Option<librorolala::storage::CollectReport>,
}

/// Carries the plans out, under each side's lock, the index first.
fn sweep(
    runtime: &tokio::runtime::Runtime,
    vcs: &librorolala::vcs::VCSIndex,
    store: &librorolala::storage::RorolalaStorage,
    plans: &Plans,
    progress: &Progress,
) -> Result<Taken, SweepFailure> {
    let mut taken = Taken {
        index: None,
        storage: None,
    };

    if let Some(plan) = &plans.index {
        let guard = runtime
            .block_on(vcs.lock())
            .map_err(|error| SweepFailure::Locked(error.to_string()))?;

        let report = runtime
            .block_on(guard.collect(plan, progress))
            .map_err(|error| SweepFailure::Failed(error.to_string()))?;
        taken.index = Some(report);
    }

    if let Some(plan) = &plans.storage {
        let guard = runtime
            .block_on(store.lock())
            .map_err(|error| SweepFailure::Locked(error.to_string()))?;

        let report = runtime
            .block_on(guard.collect(plan, progress))
            .map_err(|error| SweepFailure::Failed(error.to_string()))?;
        taken.storage = Some(report);
    }

    Ok(taken)
}

/// What a report says, as the plain numbers a result carries.
///
/// Both sides answer the same shape, so one tuple is what a result keeps either way.
type Report = (usize, u64, usize, bool);

/// Result: what the collecting took away, by the side it swept.
#[derive(Grouped)]
pub struct ResultGc {
    /// The index side: entries and bytes taken, entries kept by the grace, and whether anything
    /// changed, when the index was swept.
    index: Option<Report>,
    /// The store side: the same, when the store was swept.
    storage: Option<Report>,
}

#[renderer(buffer)]
pub fn render_result_gc(result: ResultGc) {
    let changed = result.index.is_some_and(|(_, _, _, changed)| changed)
        || result.storage.is_some_and(|(_, _, _, changed)| changed);

    if changed {
        let (index_entries, index_bytes) = result
            .index
            .map_or((0, 0), |(removed, len, _, _)| (removed, len));
        let (storage_entries, storage_bytes) = result
            .storage
            .map_or((0, 0), |(removed, len, _, _)| (removed, len));

        r_println!(
            "{}",
            t!(
                "pack.result_gc",
                index_entries = index_entries,
                index_bytes = index_bytes,
                storage_entries = storage_entries,
                storage_bytes = storage_bytes,
            )
            .trim()
        );
    } else {
        r_println!("{}", t!("pack.result_gc_nothing").trim());
    }

    // What the grace kept is said, since it is the part a caller may have expected to go and did
    // not: it comes back to the next collecting, after its reference has landed.
    let grace = result
        .index
        .into_iter()
        .chain(result.storage)
        .map(|(_, _, kept, _)| kept)
        .sum::<usize>();
    if grace > 0 {
        r_eprintln!("{}", t!("pack.result_gc_grace", kept = grace).trim());
    }
}

/// Result: the run was asked to confirm itself and was not confirmed.
#[derive(Grouped)]
pub struct ResultGcDeclined;

#[renderer(buffer)]
pub fn render_result_gc_declined(_: ResultGcDeclined, ec: &mut ResExitCode) {
    r_eprintln!("{}", help_line!(t!("pack.result_declined").trim()));
    ec.exit_code = EC_CANCELLED;
}

/// Error: the collecting could not be carried out.
///
/// A collecting that fails has taken nothing away, so this is always said before anything is gone
/// and a retry is safe.
#[derive(Grouped)]
pub struct ErrorGcFailed {
    /// Why the collecting could not be carried out.
    cause: String,
}

impl Failure for ErrorGcFailed {
    fn name(&self) -> &'static str {
        "error_pack_gc_failed"
    }

    fn reason(&self) -> String {
        t!("pack.err_gc_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorGcFailed);

#[renderer(buffer)]
pub fn render_error_gc_failed(error: ErrorGcFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_gc_failed_help").trim()));
    ec.exit_code = EC_ERR_PACK_GC_FAILED;
}

/// Error: the run is nowhere a Layout is, so there are no roots to collect from.
#[derive(Grouped)]
pub struct ErrorGcNoLayout;

impl Failure for ErrorGcNoLayout {
    fn name(&self) -> &'static str {
        "error_pack_gc_no_layout"
    }

    fn reason(&self) -> String {
        t!("pack.err_gc_no_layout").trim().to_string()
    }
}

failure!(ErrorGcNoLayout);

#[renderer(buffer)]
pub fn render_error_gc_no_layout(error: ErrorGcNoLayout, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_gc_no_layout_help").trim()));
    ec.exit_code = EC_ERR_PACK_GC_NO_LAYOUT;
}

/// Error: the Workspace holds merges waiting, and a collecting would take what they keep.
#[derive(Grouped)]
pub struct ErrorGcMerging;

impl Failure for ErrorGcMerging {
    fn name(&self) -> &'static str {
        "error_pack_gc_merging"
    }

    fn reason(&self) -> String {
        t!("pack.err_gc_merging").trim().to_string()
    }
}

failure!(ErrorGcMerging);

#[renderer(buffer)]
pub fn render_error_gc_merging(error: ErrorGcMerging, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_gc_merging_help").trim()));
    ec.exit_code = EC_ERR_PACK_GC_MERGING;
}

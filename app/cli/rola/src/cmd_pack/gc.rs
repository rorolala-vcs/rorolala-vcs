//! The collecting half of `rola pack`: what `--garbage-collect` does, and nothing else.
//!
//! The roots are the Layout the run stands in: a Vault keeps its truth Layout, a Workspace the one
//! Layout it works in, and a run in neither has no roots at all, which is refused rather than
//! guessed at, since a collecting without roots takes everything away. What the roots keep is
//! worked out first, shown for the confirmation, and only then taken away, index first and store
//! after, so a collecting that stops halfway leaves content behind that nothing names rather than
//! a reference with nothing under it.
//!
//! Every place the store records is a root too: a store shared between places is collected once,
//! and a run that read only the place it stands in would take the other places' content away.

// The collecting run is one ordered pass over the roots, the plans and the sweep, and the order is
// what makes it safe; splitting it into helpers would pass the same half dozen values around.
#![allow(
    clippy::too_many_lines,
    reason = "the order of the roots, the plans and the sweep is the safety property"
)]

use librorolala::gc::{DEFAULT_GRACE, Live};
use librorolala::storage::RorolalaStorage;
use librorolala::vault::Vault;
use librorolala::vcs::VCSIndex;
use librorolala::workspace::{Merging, Workspace};
use librorolala::write::collect::{
    Plans, SweepError as SweepFailure, plans as collect_plans, sweep as collect_sweep,
};
use mingling::{
    Grouped,
    confirm::YesConfirm,
    macros::{buffer, r_eprintln, r_println, renderer},
    res::{ResConfirm, ResExitCode},
};
use rorolala_cli_setups::{
    ResProgressSetting, ResStorageSet, ResVCSIndex, ResVault, ResVaultConfig, ResWorkspace,
    ResWorkspaceConfig,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rust_i18n::t;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::Next;
use crate::exit_codes::{
    EC_CANCELLED, EC_ERR_PACK_GC_FAILED, EC_ERR_PACK_GC_MERGING, EC_ERR_PACK_GC_NO_LAYOUT,
    EC_ERR_STORAGE_BIND,
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
    storage: &ResStorageSet,
    index: &ResVCSIndex,
    vault: &ResVault,
    workspace: &ResWorkspace,
    vault_config: &ResVaultConfig,
    workspace_config: &ResWorkspaceConfig,
    force: bool,
    confirm: ResConfirm,
    progress_setting: ResProgressSetting,
) -> Next {
    // The roots are the Layout the run stands in, and the places the store records. A run in
    // neither a Vault nor a Workspace has no Layout of its own, which is allowed when the store
    // records its users: collecting against no root at all would take everything away.
    let own_layout = match roots_of(vault, workspace) {
        Ok(layout) => layout,
        Err(cause) => return ErrorGcFailed { cause }.into(),
    };

    // A Workspace with merges waiting keeps variants no chain names, which is the whole of what a
    // collecting would take away from it. The records are this run's to respect, so they are asked
    // before anything is enumerated.
    if let Some(held) = workspace.as_ref()
        && vault.as_ref().is_none()
        && Merging::read(held.get_root()).iter().next().is_some()
    {
        return ErrorGcMerging {
            path: held.get_root().display().to_string(),
        }
        .into();
    }

    // How long the collecting holds what nothing references: what the run said with `--grace`
    // comes first, the configuration of the Vault or Workspace the roots came from comes second,
    // and the collecting run's own default comes last. One value holds for both stores, since a
    // run that protected them differently would protect the write half way.
    let grace = state.grace.unwrap_or_else(|| {
        config_grace(vault_config, workspace_config).map_or(DEFAULT_GRACE, Duration::from_secs)
    });

    // The store is the target: the first binding that is there. The index is optional, since a run
    // in a bare store collects only content and has no index of its own.
    let Some(set) = storage.storage() else {
        // A run in nothing at all has no roots and no store; the old answer, "no Layout", is the
        // one a caller reads.
        return if own_layout.is_none() {
            ErrorGcNoLayout.into()
        } else {
            super::ErrorPackNoStorage.into()
        };
    };
    let store = set.write_target();

    if !set.write_target_writable() {
        return ErrorGcReadOnly {
            path: store.get_root().display().to_string(),
        }
        .into();
    }

    let vcs = index.as_ref();

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
    let live = match &own_layout {
        Some(layout) => match vcs {
            Some(index) => match runtime.block_on(librorolala::gc::live_of(
                layout,
                index,
                set,
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
            },
            None if layout.entries().is_empty() => Live::default(),
            None => {
                reporting.finish();

                return ErrorGcFailed {
                    cause: "the Layout has entries and no index to read them with".to_owned(),
                }
                .into();
            }
        },
        None => Live::default(),
    };

    // What every place the store records keeps is kept too: a collecting run that only read the
    // place it stands in would take another place's content away.
    let own_root = vault
        .as_ref()
        .and_then(rorolala_vault::Vault::get_current_rola_storage)
        .or_else(|| {
            workspace
                .as_ref()
                .and_then(rorolala_workspace::Workspace::get_current_rola_storage)
        })
        .map(|held| held.get_root().to_path_buf());
    let live = match connected_live(store, own_root.as_deref(), &runtime, &reporting.progress()) {
        Ok((more, found)) => {
            // A store outside the place standing here is collected only when it says who writes to
            // it: its own `.` says nothing about the places that share it, and collecting against
            // no root at all would take every one of their content away. A run with no place of
            // its own is in the store itself, so there is no other place to ask about.
            let external = own_root
                .as_deref()
                .is_some_and(|own| own != store.get_root());
            if external && !found {
                reporting.finish();

                return ErrorGcNoLayout.into();
            }

            let mut live = live;
            live.absorb(more);
            live
        }
        Err(ConnectedFailure::Merging(path)) => {
            reporting.finish();

            return ErrorGcMerging { path }.into();
        }
        Err(ConnectedFailure::Failed(cause)) => {
            reporting.finish();

            return ErrorGcFailed { cause }.into();
        }
    };

    // A closure with nothing in it over a store that holds something is the shape of a root set
    // that names no content at all, and taking everything away is what the confirmation would be
    // agreeing to without seeing it. `--force` is the caller saying it means it.
    if live.objects().is_empty() && live.manifests().is_empty() {
        let held = runtime.block_on(set.list_exist_keys()).unwrap_or_default();

        if !held.is_empty() && !force {
            reporting.finish();

            return ErrorGcNoRoots.into();
        }
    }

    // The plans are read-only, so they are made outside the locks and shown for the confirmation.
    // A bare store has no index of its own, so only the store side is swept: there is nothing an
    // index side could be.
    let sweep_index = state.index && vcs.is_some();
    let plans = match collect_plans(
        &runtime,
        vcs,
        store,
        &live,
        sweep_index,
        state.storage,
        grace,
    ) {
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
    let taken = match collect_sweep(&runtime, vcs, store, &plans, &reporting.progress()) {
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

/// Why the places a store records could not all be read.
enum ConnectedFailure {
    /// A Workspace has a merge waiting, which keeps variants no chain names.
    Merging(String),
    /// A place could not be read at all.
    Failed(String),
}

/// What every place the store records keeps.
///
/// A store records the Workspaces and Vaults that write to it, and a `git-rola` project is one of
/// the Workspaces it may record. Each is read the way the run's own place is: its Layout and index
/// for a place, its lock for a project. The store being collected is added as a last, read-only
/// fallback, so a place that does not bind it still has its content read.
///
/// A place that is gone is said aloud and passed over; a place that is there and does not read is
/// a failure, since guessing past it would take what it names away.
fn connected_live(
    store: &RorolalaStorage,
    own_root: Option<&Path>,
    runtime: &tokio::runtime::Runtime,
    progress: &Progress,
) -> Result<(Live, bool), ConnectedFailure> {
    let connected = store
        .connected()
        .map_err(|error| ConnectedFailure::Failed(error.to_string()))?;
    let mut live = Live::default();
    let mut found_any = false;

    for (entries, workspace_side) in [(&connected.workspaces, true), (&connected.vaults, false)] {
        for written in entries {
            let root = PathBuf::from(written);

            // The run's own store is the place standing here, and it is already read.
            if own_root.is_some_and(|own| own == root.as_path()) {
                continue;
            }

            let mut found = false;

            if root.join(".rola").join("workspace.toml").is_file() {
                found = true;
                let workspace = Workspace::locate(&root).ok_or_else(|| {
                    ConnectedFailure::Failed(format!("`{written}` is not a Workspace any more"))
                })?;

                if Merging::read(workspace.get_root()).iter().next().is_some() {
                    return Err(ConnectedFailure::Merging(written.clone()));
                }

                if let Some(layout) = workspace
                    .layout_if_there()
                    .map_err(|error| ConnectedFailure::Failed(format!("`{written}`: {error}")))?
                {
                    if let Some(index) = VCSIndex::locate(workspace.get_root()) {
                        let set = workspace
                            .storage_set()
                            .map_err(|error| ConnectedFailure::Failed(error.to_string()))?
                            .with_read_fallback(store.clone());
                        let more = runtime
                            .block_on(librorolala::gc::live_of(&layout, &index, &set, progress))
                            .map_err(|error| ConnectedFailure::Failed(error.to_string()))?;

                        live.absorb(more);
                    } else if !layout.entries().is_empty() {
                        return Err(ConnectedFailure::Failed(format!(
                            "`{written}` has a Layout and no index"
                        )));
                    }
                }
            }

            if root.join("rola.toml").is_file() {
                found = true;
                let set = librorolala::storage::StorageSet::sole(store.clone());
                let more = runtime
                    .block_on(librorolala::gc::live_of_project(&root, &set, progress))
                    .map_err(|error| ConnectedFailure::Failed(error.to_string()))?;

                live.absorb(more);
            }

            if root.join("vault.toml").is_file() {
                found = true;
                let vault = Vault::locate(&root).ok_or_else(|| {
                    ConnectedFailure::Failed(format!("`{written}` is not a Vault any more"))
                })?;

                if let Some(layout) = vault
                    .layout_if_there()
                    .map_err(|error| ConnectedFailure::Failed(format!("`{written}`: {error}")))?
                {
                    if let Some(index) = VCSIndex::locate(vault.get_root()) {
                        let set = vault
                            .storage_set()
                            .map_err(|error| ConnectedFailure::Failed(error.to_string()))?
                            .with_read_fallback(store.clone());
                        let more = runtime
                            .block_on(librorolala::gc::live_of(&layout, &index, &set, progress))
                            .map_err(|error| ConnectedFailure::Failed(error.to_string()))?;

                        live.absorb(more);
                    } else if !layout.entries().is_empty() {
                        return Err(ConnectedFailure::Failed(format!(
                            "`{written}` has a Layout and no index"
                        )));
                    }
                }
            }

            if found {
                found_any = true;
            } else {
                eprintln!("{}", t!("pack.gc_lost_binding", path = written.as_str()));
                forget_binding(store, &root, workspace_side);
            }
        }
    }

    Ok((live, found_any))
}

/// Removes a binding the collection found nothing at, when the store may be written.
///
/// A read-only store keeps the record: the run cannot change it, and the warning already said the
/// binding was passed over.
fn forget_binding(store: &RorolalaStorage, root: &Path, workspace_side: bool) {
    if !store.is_writable() {
        eprintln!(
            "{}",
            t!(
                "pack.gc_binding_readonly",
                path = root.display().to_string()
            )
        );
        return;
    }

    let removed = if workspace_side {
        store.disconnect_workspace(root)
    } else {
        store.disconnect_vault(root)
    };

    if let Err(error) = removed {
        eprintln!(
            "{}",
            t!(
                "pack.gc_binding_not_removed",
                path = root.display().to_string(),
                error = error.to_string()
            )
        );
    }
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

/// How long the configuration says an entry nothing references is kept, when it says.
///
/// The Vault the roots came from is asked first, the Workspace second, and a configuration that
/// says nothing is answered with nothing, which is the run's own default to fill.
fn config_grace(
    vault_config: &ResVaultConfig,
    workspace_config: &ResWorkspaceConfig,
) -> Option<u64> {
    if let Some(config) = vault_config.config() {
        return config
            .gc_config()
            .and_then(librorolala::vault::GcConfig::grace);
    }

    if let Some(config) = workspace_config.config() {
        return config
            .gc_config()
            .and_then(librorolala::workspace::GcConfig::grace);
    }

    None
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

/// Error: a Workspace holds merges waiting, and a collecting would take what they keep.
#[derive(Grouped)]
pub struct ErrorGcMerging {
    /// The Workspace whose merge is waiting.
    path: String,
}

impl Failure for ErrorGcMerging {
    fn name(&self) -> &'static str {
        "error_pack_gc_merging"
    }

    fn reason(&self) -> String {
        t!("pack.err_gc_merging", path = self.path.as_str())
            .trim()
            .to_string()
    }
}

failure!(ErrorGcMerging);

#[renderer(buffer)]
pub fn render_error_gc_merging(error: ErrorGcMerging, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_gc_merging_help").trim()));
    ec.exit_code = EC_ERR_PACK_GC_MERGING;
}

/// Error: the roots reach no content while the store holds some.
#[derive(Grouped)]
pub struct ErrorGcNoRoots;

impl Failure for ErrorGcNoRoots {
    fn name(&self) -> &'static str {
        "error_pack_gc_no_roots"
    }

    fn reason(&self) -> String {
        t!("pack.err_gc_no_roots").trim().to_string()
    }
}

failure!(ErrorGcNoRoots);

#[renderer(buffer)]
pub fn render_error_gc_no_roots(error: ErrorGcNoRoots, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_gc_no_roots_help").trim()));
    ec.exit_code = EC_ERR_PACK_GC_NO_LAYOUT;
}

/// Error: the store a collecting run targets is read-only.
#[derive(Grouped)]
pub struct ErrorGcReadOnly {
    /// The store that cannot be written.
    path: String,
}

impl Failure for ErrorGcReadOnly {
    fn name(&self) -> &'static str {
        "error_pack_gc_read_only"
    }

    fn reason(&self) -> String {
        t!("pack.err_gc_read_only", path = self.path.as_str())
            .trim()
            .to_string()
    }
}

failure!(ErrorGcReadOnly);

#[renderer(buffer)]
pub fn render_error_gc_read_only(error: ErrorGcReadOnly, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    ec.exit_code = EC_ERR_STORAGE_BIND;
}

//! What a collecting run would take away, and the sweep that takes it.
//!
//! The plans are read before either side is locked, so what the confirmation names is what the run
//! means to do. The sweep then takes each side under its own lock, the index first and the store
//! after, so a sweep that stops halfway leaves content behind that nothing names rather than a
//! reference with nothing under it.

use std::path::Path;
use std::time::Duration;

use rorolala_storage::CollectPlan as StoragePlan;
use rorolala_storage::CollectReport as StorageReport;
use rorolala_storage::Lockable as _;
use rorolala_storage::RorolalaStorage;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rorolala_vault::Vault;
use rorolala_vcs::{CollectPlan as IndexPlan, CollectReport as IndexReport, VCSIndex};
use rorolala_workspace::Workspace;
use serde::Serialize;

use crate::gc::Live;
use crate::write::WriteError;

/// What the two sweeps would take away.
#[derive(Clone, Default)]
pub struct Plans {
    /// What the index sweep would take away, when the index is swept.
    pub index: Option<IndexPlan>,
    /// What the store sweep would take away, when the store is swept.
    pub storage: Option<StoragePlan>,
}

/// Why a sweep failed.
#[derive(Debug, Clone)]
pub enum SweepError {
    /// One of the stores was locked by another run.
    Locked(String),
    /// The sweep itself could not be carried out.
    Failed(String),
}

/// What a sweep took away.
#[derive(Clone, Default)]
pub struct Taken {
    /// The index sweep's report, when the index was swept.
    pub index: Option<IndexReport>,
    /// The store sweep's report, when the store was swept.
    pub storage: Option<StorageReport>,
}

/// Reads the plans the two sides would carry out, without locking either.
///
/// A plan is carried out against the stores as they stand when the sweep runs, so what arrives
/// between this and the sweep is kept and what is gone is skipped.
///
/// # Errors
///
/// Returns a message when a plan cannot be read.
pub fn plans(
    runtime: &tokio::runtime::Runtime,
    index: &VCSIndex,
    store: &RorolalaStorage,
    live: &Live,
    sweep_index: bool,
    sweep_storage: bool,
    grace: Duration,
) -> Result<Plans, String> {
    let index = if sweep_index {
        Some(
            runtime
                .block_on(index.collect_plan(live.index(), grace))
                .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };

    let storage = if sweep_storage {
        Some(
            runtime
                .block_on(store.collect_plan(live.objects(), live.manifests(), grace))
                .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };

    Ok(Plans { index, storage })
}

/// Carries the plans out, under each side's lock, the index first.
///
/// # Errors
///
/// Returns [`SweepError::Locked`] when a side is locked by another run, and
/// [`SweepError::Failed`] when a sweep cannot be carried out.
pub fn sweep(
    runtime: &tokio::runtime::Runtime,
    index: &VCSIndex,
    store: &RorolalaStorage,
    plans: &Plans,
    progress: &Progress,
) -> Result<Taken, SweepError> {
    let mut taken = Taken::default();

    if let Some(plan) = &plans.index {
        let guard = runtime
            .block_on(index.lock())
            .map_err(|error| SweepError::Locked(error.to_string()))?;

        taken.index = Some(
            runtime
                .block_on(guard.collect(plan, progress))
                .map_err(|error| SweepError::Failed(error.to_string()))?,
        );
    }

    if let Some(plan) = &plans.storage {
        let guard = runtime
            .block_on(store.lock())
            .map_err(|error| SweepError::Locked(error.to_string()))?;

        taken.storage = Some(
            runtime
                .block_on(guard.collect(plan, progress))
                .map_err(|error| SweepError::Failed(error.to_string()))?,
        );
    }

    Ok(taken)
}

/// One side's numbers, as a plan or a sweep answers them.
#[derive(Serialize)]
struct Side {
    /// How many entries the side would take or took away.
    removed: usize,
    /// How many bytes those entries occupy.
    bytes: u64,
    /// How many entries the grace keeps.
    kept: usize,
    /// Whether the side changed, which a plan never says.
    changed: bool,
}

/// Both sides, as a plan or a sweep answers them.
#[derive(Serialize)]
struct Summary {
    /// The index side, when it is swept.
    index: Option<Side>,
    /// The store side, when it is swept.
    storage: Option<Side>,
}

/// The Layout a collecting run stands in: a Vault's truth Layout first, a Workspace's after.
fn roots(directory: &Path) -> Result<(crate::layout::Layout, RorolalaStorage), WriteError> {
    if let Some(vault) = Vault::locate(directory) {
        let layout = vault.layout().map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })?;
        let store = vault
            .get_current_rola_storage()
            .ok_or(WriteError::NoStore)?;
        return Ok((layout, store));
    }
    if let Some(workspace) = Workspace::locate(directory) {
        let layout = workspace.layout().map_err(|error| WriteError::Layout {
            reason: error.to_string(),
        })?;
        let store = workspace
            .get_current_rola_storage()
            .ok_or(WriteError::NoStore)?;
        return Ok((layout, store));
    }

    Err(WriteError::NoLayout)
}

/// The grace a run keeps entries by, the default when it names none.
const fn grace_of(seconds: u64) -> Duration {
    if seconds == 0 {
        crate::gc::DEFAULT_GRACE
    } else {
        Duration::from_secs(seconds)
    }
}

/// Reads what a collecting run would take away under a directory.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, the plan's numbers, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. A grace of zero means the
/// default.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`], [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the
/// parts are missing, and [`WriteError::Internal`] when the closure cannot be worked out.
#[lazyffi(export = rola_write_collect_plan)]
pub fn plan_at(
    directory: &Path,
    storage: bool,
    index: bool,
    grace: u64,
) -> Result<String, WriteError> {
    let (layout, store) = roots(directory)?;
    let vcs = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let runtime = runtime()?;
    let progress = Progress::silent();
    let live = runtime
        .block_on(crate::gc::live_of(&layout, &vcs, &store, &progress))
        .map_err(|error| WriteError::Internal {
            reason: format!("{error:?}"),
        })?;
    let plans = self::plans(
        &runtime,
        &vcs,
        &store,
        &live,
        index,
        storage,
        grace_of(grace),
    )
    .map_err(|reason| WriteError::Internal { reason })?;

    let summary = Summary {
        index: plans.index.as_ref().map(|plan| Side {
            removed: plan.removed(),
            bytes: plan.removed_len(),
            kept: plan.kept_by_grace(),
            changed: false,
        }),
        storage: plans.storage.as_ref().map(|plan| Side {
            removed: plan.removed(),
            bytes: plan.removed_len(),
            kept: plan.kept_by_grace(),
            changed: false,
        }),
    };
    json(&summary)
}

/// Carries out a collecting run under a directory, reading its plan afresh.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, the sweep's numbers, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. A grace of zero means the
/// default.
///
/// # Errors
///
/// Returns [`WriteError::NoLayout`], [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the
/// parts are missing, and [`WriteError::Internal`] when the closure or the sweep fails.
#[lazyffi(export = rola_write_collect_apply)]
pub fn collect_at(
    directory: &Path,
    storage: bool,
    index: bool,
    grace: u64,
) -> Result<String, WriteError> {
    let (layout, store) = roots(directory)?;
    let vcs = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let runtime = runtime()?;
    let progress = Progress::silent();
    let live = runtime
        .block_on(crate::gc::live_of(&layout, &vcs, &store, &progress))
        .map_err(|error| WriteError::Internal {
            reason: format!("{error:?}"),
        })?;
    let plans = self::plans(
        &runtime,
        &vcs,
        &store,
        &live,
        index,
        storage,
        grace_of(grace),
    )
    .map_err(|reason| WriteError::Internal { reason })?;
    let taken =
        sweep(&runtime, &vcs, &store, &plans, &progress).map_err(|error| WriteError::Internal {
            reason: match error {
                SweepError::Locked(reason) | SweepError::Failed(reason) => reason,
            },
        })?;

    let summary = Summary {
        index: taken.index.map(|report| Side {
            removed: report.removed(),
            bytes: report.removed_len(),
            kept: report.kept_by_grace(),
            changed: report.changed(),
        }),
        storage: taken.storage.map(|report| Side {
            removed: report.removed(),
            bytes: report.removed_len(),
            kept: report.kept_by_grace(),
            changed: report.changed(),
        }),
    };
    json(&summary)
}

/// The runtime the blocking exports wait on.
fn runtime() -> Result<tokio::runtime::Runtime, WriteError> {
    tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })
}

/// Serializes an answer.
fn json(summary: &Summary) -> Result<String, WriteError> {
    serde_json::to_string(summary).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;

    use super::{collect_at, plan_at};
    use crate::write::track::{TrackOptions, track_file, track_vault_file};

    #[test]
    fn an_empty_workspace_plans_and_collects_nothing() {
        let dir = std::env::temp_dir().join(format!("rola-write-collect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        let plan = plan_at(&dir, true, true, 0).unwrap();
        assert!(plan.contains("\"index\""), "{plan}");
        assert!(plan.contains("\"storage\""), "{plan}");

        let neither = plan_at(&dir, false, false, 0).unwrap();
        assert_eq!(neither, r#"{"index":null,"storage":null}"#);

        let report = collect_at(&dir, true, true, 0).unwrap();
        assert!(report.contains("\"removed\":0"), "{report}");

        let neither = collect_at(&dir, false, false, 0).unwrap();
        assert_eq!(neither, r#"{"index":null,"storage":null}"#);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_plans_and_collects_nothing_it_keeps() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-collect-vault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Vault::create(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        let plan = plan_at(&dir, true, true, 0).unwrap();
        assert!(plan.contains("\"index\""), "{plan}");
        assert!(plan.contains("\"storage\""), "{plan}");

        let report = collect_at(&dir, true, true, 0).unwrap();
        assert!(report.contains("\"index\""), "{report}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_collecting_run_outside_a_scope_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-collect-none-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let error = plan_at(&dir, true, true, 0).unwrap_err();
        assert!(matches!(error, crate::write::WriteError::NoLayout));

        let _ = fs::remove_dir_all(&dir);
    }
}

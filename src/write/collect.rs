//! What a collecting run would take away, and the sweep that takes it.
//!
//! The plans are read before either side is locked, so what the confirmation names is what the run
//! means to do. The sweep then takes each side under its own lock, the index first and the store
//! after, so a sweep that stops halfway leaves content behind that nothing names rather than a
//! reference with nothing under it.

use std::time::Duration;

use rorolala_storage::CollectPlan as StoragePlan;
use rorolala_storage::CollectReport as StorageReport;
use rorolala_storage::Lockable as _;
use rorolala_storage::RorolalaStorage;
use rorolala_utils_progress::Progress;
use rorolala_vcs::{CollectPlan as IndexPlan, CollectReport as IndexReport, VCSIndex};

use crate::gc::Live;

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

//! `write-remote-entries`: writing many entries of the Vault's Layout in one exchange.
//!
//! A sync names every entry it is ahead on, and each would otherwise be its own exchange, a
//! connection and a handshake per file. The writes are made here in one go: the entries to create,
//! and the versions to set, are carried across together and answered together, one outcome per
//! write in the order they were asked for.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

use super::layout_remote::{EntryWrite, batched, create_remote, remote_transact, set_remote};

/// One entry a sync asks the Vault to create.
///
/// # FFI
///
/// A `RolaDaemonRemoteCreate *` array the caller fills. Every `const char *` is borrowed for the
/// call and released by the caller; `kind` is `tree` for a tree and anything else for a file.
#[lazyffi(export = RolaDaemonRemoteCreate, input)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteCreate {
    /// The path to make the entry at.
    pub path: String,
    /// The `Uuid` the entry is known by.
    pub uuid: String,
    /// The version to hold it at.
    pub version: String,
    /// What it is, `tree` or anything else for a file.
    pub kind: String,
}

/// One version a sync asks the Vault to set.
///
/// # FFI
///
/// A `RolaDaemonRemoteSet *` array the caller fills. Every `const char *` is borrowed for the call
/// and released by the caller.
#[lazyffi(export = RolaDaemonRemoteSet, input)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteSet {
    /// The `Uuid` of the entry to set.
    pub uuid: String,
    /// The version to set it at.
    pub version: String,
}

/// The writes a sync asks the Vault to make, as a C caller fills them.
///
/// `creates` and `sets` cross as slices: C passes the first record and a count, and each record is
/// read through its own input conversion.
pub type WriteRemoteEntriesRequest = (Vec<RemoteCreate>, Vec<RemoteSet>);

/// An Action that writes many entries of the Vault's Layout in one exchange.
///
/// # Input
///
/// The writes: the entries to create (each a path, a `Uuid`, a version and what it is) and the
/// versions to set, each a `Uuid` and a version.
///
/// # Output
///
/// What became of each: one outcome per write, in the order they were asked for, the creates first,
/// then the sets. Each outcome is what `create-remote-entry` or `set-remote-version` answers on its
/// own.
pub struct ActionWriteRemoteEntries;

impl Action for ActionWriteRemoteEntries {
    /// Writing many entries is id 16.
    const ID: u32 = 16;

    /// The entries to create, and the versions to set.
    ///
    /// - creates: `Vec<RemoteCreate>`
    /// - sets: `Vec<RemoteSet>`
    type Input = (Vec<RemoteCreate>, Vec<RemoteSet>);
    type Output = EntryWrites;

    /// Asks the Vault to make the writes, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if an input or an
    /// outcome will not cross, [`ActionError::Store`] if the Vault's Layout could not be written, and
    /// whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcomes = remote_transact(&mut ctx, input, |layout, (creates, sets), me, admin| {
            // The writes are one batch: what a sync is ahead on is written with one flush per shard
            // rather than one per entry, and each write is checked against the writes before it, so
            // the answers are the ones one exchange per entry would give.
            batched(layout, |batch| {
                let mut outcomes = Vec::with_capacity(creates.len() + sets.len());
                for create in &creates {
                    outcomes.push(create_remote(
                        batch,
                        &create.path,
                        &create.uuid,
                        &create.version,
                        &create.kind,
                        me,
                    )?);
                }
                for set in &sets {
                    outcomes.push(set_remote(batch, &set.uuid, &set.version, me, admin)?);
                }

                Ok(outcomes)
            })
        })
        .await?;

        Ok(EntryWrites::from(outcomes))
    }
}

/// What became of a batch of writes, as a C caller reads them.
///
/// # FFI
///
/// A `RolaDaemonEntryWrites *` the caller releases with `free_rola_daemon_entry_writes`. Its
/// `_count` and `_at` answer with the outcomes, one per write, in the order they were asked for.
/// Each is a `RolaDaemonEntryWriteOutcome *` the caller releases with `free_rola_daemon_entry_write_outcome`.
#[lazyffi(export = RolaDaemonEntryWrites)]
#[derive(Debug, Clone, Default, Serialize)]
pub struct EntryWrites {
    /// Each outcome, in the order the writes were asked for.
    outcomes: Vec<EntryWriteOutcome>,
}

#[lazyffi(export = rola_daemon_)]
impl EntryWrites {
    /// How many outcomes the batch answered.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.outcomes.len()
    }

    /// The outcome at `index`, or null past the end.
    #[must_use]
    pub fn at(&self, index: usize) -> Option<EntryWriteOutcome> {
        self.outcomes.get(index).cloned()
    }
}

impl From<Vec<EntryWrite>> for EntryWrites {
    fn from(outcomes: Vec<EntryWrite>) -> Self {
        Self {
            outcomes: outcomes.into_iter().map(EntryWriteOutcome::from).collect(),
        }
    }
}

/// One outcome of a write, as a C caller reads it.
///
/// # FFI
///
/// A `RolaDaemonEntryWriteOutcome *` the caller releases with `free_rola_daemon_entry_write_outcome`. `kind` says
/// which outcome it was, and it carries nothing else.
#[lazyffi(export = RolaDaemonEntryWriteOutcome)]
#[derive(Debug, Clone, Serialize)]
pub struct EntryWriteOutcome {
    /// Which outcome the write reached.
    kind: EntryWriteKind,
}

/// Which outcome a write reached.
#[lazyffi(export = RolaDaemonEntryWriteKind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EntryWriteKind {
    /// The Vault's Layout now holds what was asked for.
    Written,
    /// The Vault already holds an entry by that `Uuid`.
    Exists,
    /// The Vault already names something at that path.
    Taken,
    /// The Vault holds no entry by that `Uuid`.
    Missing,
    /// The account does not hold the entry and does not administrate the Vault.
    Refused,
    /// A path, a `Uuid`, or a version is not one this can read.
    Malformed,
}

#[lazyffi(export = rola_daemon_)]
impl EntryWriteOutcome {
    /// Which outcome the write reached.
    #[must_use]
    pub const fn kind(&self) -> EntryWriteKind {
        self.kind
    }
}

impl From<EntryWrite> for EntryWriteOutcome {
    fn from(outcome: EntryWrite) -> Self {
        let kind = match outcome {
            EntryWrite::Written => EntryWriteKind::Written,
            EntryWrite::Exists => EntryWriteKind::Exists,
            EntryWrite::Taken => EntryWriteKind::Taken,
            EntryWrite::Missing => EntryWriteKind::Missing,
            EntryWrite::Refused => EntryWriteKind::Refused,
            EntryWrite::Malformed => EntryWriteKind::Malformed,
        };

        Self { kind }
    }
}

impl EntryWrites {
    /// The outcomes as the action made them, for a caller inside Rust.
    #[must_use]
    pub fn outcomes(&self) -> Vec<EntryWrite> {
        self.outcomes
            .iter()
            .map(EntryWriteOutcome::outcome)
            .collect()
    }
}

impl EntryWriteOutcome {
    /// The outcome as the action made it, for a caller inside Rust.
    #[must_use]
    pub const fn outcome(&self) -> EntryWrite {
        match self.kind {
            EntryWriteKind::Written => EntryWrite::Written,
            EntryWriteKind::Exists => EntryWrite::Exists,
            EntryWriteKind::Taken => EntryWrite::Taken,
            EntryWriteKind::Missing => EntryWrite::Missing,
            EntryWriteKind::Refused => EntryWrite::Refused,
            EntryWriteKind::Malformed => EntryWrite::Malformed,
        }
    }
}

//! `write-remote-entries`: writing many entries of the Vault's Layout in one exchange.
//!
//! A sync names every entry it is ahead on, and each would otherwise be its own exchange — a
//! connection and a handshake per file. The writes are made here in one go: the entries to create
//! under `@/new/`, and the versions to set, are carried across together and answered together, one
//! outcome per write in the order they were asked for.

use rorolala_protocol::{Action, ActionContext, ActionError, OnlyWorkspace};
use serde::{Deserialize, Serialize};

use super::layout_remote::{batched, create_remote, remote_transact, set_remote};

/// The writes a sync asks the Vault to make, as they cross.
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteWrites {
    /// Each entry to create: its path, its `Uuid`, and the version to hold it at.
    pub creates: Vec<(String, String, String)>,

    /// Each version to move: the `Uuid` and the version.
    pub sets: Vec<(String, String)>,
}

/// An Action that writes many entries of the Vault's Layout in one exchange.
///
/// # Input
///
/// The writes, as JSON: the entries to create — each a path, a `Uuid`, and a version — and the
/// versions to set, each a `Uuid` and a version.
///
/// # Output
///
/// What became of each, as JSON: an array of outcomes, one per write, in the order they were asked
/// for — the creates first, then the sets. Each outcome is what `create-remote-entry` or
/// `set-remote-version` answers on its own.
pub struct ActionWriteRemoteEntries;

impl Action for ActionWriteRemoteEntries {
    /// Writing many entries is id 16.
    const ID: u32 = 16;

    type Input = String;
    type Output = String;

    /// Asks the Vault to make the writes, and answers with what it said.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, or the Vault holds no member to act as, [`ActionError::Codec`] if an input or an
    /// outcome will not cross, [`ActionError::Json`] if the writes will not read or the outcomes
    /// will not encode, [`ActionError::Store`] if the Vault's Layout could not be written, and
    /// whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let outcomes = remote_transact(&mut ctx, input, |layout, writes, me, admin| {
            let writes: RemoteWrites =
                serde_json::from_str(&writes).map_err(|error| ActionError::Json(error.into()))?;

            // The writes are one batch: what a sync is ahead on is written with one flush per shard
            // rather than one per entry, and each write is checked against the writes before it, so
            // the answers are the ones one exchange per entry would give.
            batched(layout, |batch| {
                let mut outcomes = Vec::with_capacity(writes.creates.len() + writes.sets.len());
                for (path, uuid, version) in &writes.creates {
                    outcomes.push(create_remote(batch, path, uuid, version, me)?);
                }
                for (uuid, version) in &writes.sets {
                    outcomes.push(set_remote(batch, uuid, version, me, admin)?);
                }

                Ok(outcomes)
            })
        })
        .await?;

        serde_json::to_string(&outcomes).map_err(|error| ActionError::Json(error.into()))
    }
}

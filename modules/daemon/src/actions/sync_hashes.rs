//! `sync-hashes`: making the two stores hold the same named keys.
//!
//! Where [`sync_all`](crate::sync_all) works out what to sync — everything either side holds — this
//! is told: the keys are the caller's, and the carrying both sides do afterwards is the very
//! primitive a full sync uses. So a full sync is this with everything named, and this is a full sync
//! narrowed to a list.

use std::str::FromStr as _;

use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::Key;

use super::sync_all::local_store;
use super::sync_storage::{Side, carry_blob, sync_storage};

/// An Action that makes the two stores hold the keys it is given.
///
/// # Input
///
/// The hashes, separated by whitespace — one a line, as a listing prints them. A hash is read the
/// way a key is anywhere else: as hex, or named with `blake3:` or `manifest:` in front of it.
///
/// # Output
///
/// Nothing: what a sync comes to is the same on both sides, and a caller that wants to say what
/// happened says it in its own words.
pub struct ActionSyncHashes;

impl Action for ActionSyncHashes {
    /// The named sync is id 7.
    const ID: u32 = 7;

    type Input = String;
    type Output = ();

    /// Has the two stores carry the named keys.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, and whatever the exchange fails with — see
    /// [`sync_storage`](crate::sync_storage).
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        let local = local_store(&ctx)?;

        // The list is the caller's, so it belongs to the Workspace and crosses to the Vault first:
        // both sides must name the same keys, or the carrying that follows would say two different
        // things on the two ends.
        let here = input.into_inner();
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let text = arrived
            .or(to_vault)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        let required: Vec<Key> = text
            .split_whitespace()
            .filter_map(|token| Key::from_str(token).ok())
            .collect();

        // Each side names the store it has; the other wrapper is empty and the other side fills it,
        // exactly as a full sync hands them in.
        let workspace = OnlyWorkspace::new(&ctx, || local.clone());
        let vault = OnlyVault::new(&ctx, || local.clone());

        sync_storage(&mut ctx, workspace, vault, Both::new(required)).await
    }
}

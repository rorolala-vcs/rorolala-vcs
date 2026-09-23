//! `sync-all`: making the store on each side hold the same keys as the other.
//!
//! Where [`sync_storage`](crate::sync_storage) is the carrying, this is the asking: it works out
//! what a full sync is — everything either side holds — and hands that to the primitive, so that
//! what one side has and the other does not is on both by the time it returns.

use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::{Blake3Hash, Key, RorolalaStorage, StorageBackend as _};

use super::sync_storage::{Side, carry_blob, sync_storage};

/// An Action that makes the Workspace's store and the Vault's hold the same keys.
///
/// The Workspace and the Vault each keep a store of their own, and this asks for a full sync
/// between them: each side says what it holds, and the keys asked for are everything the two of
/// them hold between them, so what is only on one side ends up on both. Nothing is uploaded or
/// downloaded — a key moves in whichever direction it is only on one side of.
///
/// A side that has no store is given one, since a store that is not there holds nothing and a full
/// sync of two stores is what was asked for.
///
/// # Input
///
/// The trait names an input and this action reads none: what to sync is not the caller's to say,
/// since a full sync is everything either side holds. What crosses is the empty string, and nothing
/// of it is read.
///
/// # Output
///
/// Nothing: what a full sync comes to is the same on both sides, and a caller that wants to say
/// what happened says it in its own words.
pub struct ActionSyncAll;

impl Action for ActionSyncAll {
    /// The sync is id 1: what a peer asks for once it has spoken.
    const ID: u32 = 1;

    type Input = String;
    type Output = ();

    /// Works out what a full sync is, and has the two stores carry it out.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault to work on, and whatever the exchange fails with — see
    /// [`sync_storage`](crate::sync_storage).
    async fn process(
        _: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // The store this side has: the Workspace's on the Workspace, the Vault's on the Vault.
        let local = local_store(&ctx)?;

        // What this side holds, as one run of digests: what crosses is that run, said to the other
        // side and read back, so that both ends know what the two of them hold between them. The run
        // goes in pieces, like everything else that crosses, so a store of any size can say what it
        // holds without any one value being too long to frame.
        let mine: Vec<u8> = local
            .list_exist_keys()
            .await
            .map_err(|error| ActionError::Store(error.to_string()))?
            .iter()
            .flat_map(|key| *key.digest())
            .collect();

        let to_vault = ctx.is_workspace().then(|| mine.clone());
        let held_by_workspace = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let to_workspace = ctx.is_vault().then(|| mine.clone());
        let held_by_vault = carry_blob(&mut ctx, Side::Workspace, to_workspace.clone()).await?;

        // Each side sent one of those and was given the other, whichever side it is.
        let held_by_workspace = held_by_workspace.or(to_vault).unwrap_or_default();
        let held_by_vault = held_by_vault.or(to_workspace).unwrap_or_default();

        // Everything either side holds, in one order both ends reach: the keys are taken as their
        // digests, which are what can cross, and put back into keys in the order digests sort in.
        let mut required: Vec<Key> = held_by_workspace
            .chunks_exact(size_of::<Blake3Hash>())
            .chain(held_by_vault.chunks_exact(size_of::<Blake3Hash>()))
            .map(|digest| {
                Key::new(
                    digest
                        .try_into()
                        .expect("a run of digests is a whole number of them"),
                )
            })
            .collect();
        required.sort_unstable();
        required.dedup();

        // Each side names the store it has; the other wrapper is empty and the other side fills it.
        let workspace = OnlyWorkspace::new(&ctx, || local.clone());
        let vault = OnlyVault::new(&ctx, || local.clone());

        sync_storage(&mut ctx, workspace, vault, Both::new(required)).await
    }
}

/// The store this side works on, made if it is not there yet.
///
/// Which store that is follows from where this runs: an action taken from a Workspace works on the
/// Workspace's store, and one served by a Vault works on the Vault's. A side that has neither is
/// not a side an action can run on.
fn local_store(ctx: &ActionContext<'_>) -> Result<RorolalaStorage, ActionError> {
    if let Some(workspace) = ctx.current_workspace().into_inner() {
        return Ok(workspace.get_or_create_rola_storage());
    }

    if let Some(vault) = ctx.current_vault().into_inner() {
        return Ok(vault.get_or_create_rola_storage());
    }

    Err(ActionError::MissingValue)
}

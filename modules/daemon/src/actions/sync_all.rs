//! `sync-all`: making the store on each side hold the same keys as the other.
//!
//! Where [`sync_storage`](crate::sync_storage) is the carrying, this is the asking: it works out
//! what a full sync is, everything either side holds, and hands that to the primitive, so that
//! what one side has and the other does not is on both by the time it returns.

use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::{Blake3Hash, Key, StorageSet};

use super::sync_storage::{DEFAULT_WINDOW, Side, carry_blob, grant, split_ask, sync_storage};

/// An Action that makes the Workspace's store and the Vault's hold the same keys.
///
/// The Workspace and the Vault each keep a store of their own, and this asks for a full sync
/// between them: each side says what it holds, and the keys asked for are everything the two of
/// them hold between them, so what is only on one side ends up on both. Nothing is uploaded or
/// downloaded, a key moves in whichever direction it is only on one side of.
///
/// A side that has no store is given one, since a store that is not there holds nothing and a full
/// sync of two stores is what was asked for.
///
/// # Input
///
/// The trait names an input and what to sync is not the caller's to say, since a full sync is
/// everything either side holds. What it carries is the ask for how much the sending side reads at
/// once, `c:<n>`, the same token [`sync-hashes`](crate::ActionSyncHashes) reads: the ask belongs to
/// the Workspace, so it crosses to the Vault first, and both ends work the window out from it.
/// Nothing else in it is read.
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
    /// Vault to work on, and whatever the exchange fails with, see
    /// [`sync_storage`](crate::sync_storage).
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // The store this side has: the Workspace's on the Workspace, the Vault's on the Vault.
        let local = local_stores(&ctx)?;

        // How much the caller wants read at once is its own word, so it crosses to the Vault first:
        // the window is the sending side's number, and each side sends one direction of a full sync.
        let here = input.into_inner();
        let to_vault = here.clone().map(String::into_bytes);
        let arrived = carry_blob(&mut ctx, Side::Vault, to_vault.clone()).await?;
        let text = arrived
            .or(to_vault)
            .map_or_default(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        let (asked, _) = split_ask(&text);
        let granted = grant(&mut ctx, asked).await?;
        let window = usize::try_from(granted).unwrap_or(DEFAULT_WINDOW);

        // What this side holds, as one run of digests: what crosses is that run, said to the other
        // side and read back, so that both ends know what the two of them hold between them. The run
        // goes in pieces, like everything else that crosses, so a store of any size can say what it
        // holds without any one value being too long to frame.
        //
        // It is every key the store holds rather than only those it can produce: producing one means
        // reading it back, chunk by chunk, which is a full read of the store before a byte moves, and
        // the presence the two answers feed is what decides each key anyway. What that costs is the
        // one thing this run is for: a key held but not producible, a manifest whose chunks are gone
        //, is now asked for by both sides, and neither holds it, so the run says so rather than
        // leaving it out of the sync in silence.
        let mine: Vec<u8> = local
            .list_all_keys()
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
            .as_chunks::<{ size_of::<Blake3Hash>() }>()
            .0
            .iter()
            .chain(
                held_by_vault
                    .as_chunks::<{ size_of::<Blake3Hash>() }>()
                    .0
                    .iter(),
            )
            .map(|digest| Key::new(*digest))
            .collect();
        required.sort_unstable();
        required.dedup();

        // Each side names the store it has; the other wrapper is empty and the other side fills it.
        let workspace = OnlyWorkspace::new(&ctx, || local.clone());
        let vault = OnlyVault::new(&ctx, || local.clone());

        sync_storage(&mut ctx, workspace, vault, Both::new(required), window).await
    }
}

/// The stores this side works on.
///
/// A caller that handed its stores in has them used as they are; that is how a `git-rola` project
/// works through a borrowed Workspace whose own configuration is not the project's. Otherwise the
/// stores follow from where this runs: an action taken from a Workspace works on the stores the
/// Workspace binds, and one served by a Vault on the Vault's. A side that has neither is not a side
/// an action can run on.
pub(crate) fn local_stores(ctx: &ActionContext<'_>) -> Result<StorageSet, ActionError> {
    if let Some(stores) = ctx.local_stores() {
        return Ok(stores.clone());
    }

    if let Some(workspace) = ctx.current_workspace().into_inner() {
        return workspace
            .storage_set()
            .map_err(|error| ActionError::Store(error.to_string()));
    }

    if let Some(vault) = ctx.current_vault().into_inner() {
        return vault
            .storage_set()
            .map_err(|error| ActionError::Store(error.to_string()));
    }

    Err(ActionError::MissingValue)
}

//! `sync-all`: making the store on each side hold the same keys as the other.
//!
//! Where [`sync_storage`](crate::sync_storage) is the carrying, this is the asking: it works out
//! what a full sync is, everything either side holds, and hands that to the primitive, so that
//! what one side has and the other does not is on both by the time it returns.

use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::{Blake3Hash, Key, StorageSet};
use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

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

/// The ask a full sync is handed.
///
/// # FFI
///
/// A `RolaDaemonSyncAllRequest` the caller fills. `ask` is borrowed for the call and released by the
/// caller; an empty `ask` leaves both ends on the default window.
#[lazyffi(export = RolaDaemonSyncAllRequest, input)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncAllRequest {
    /// How much the sending side reads at once, `c:<n>`, or empty for the default.
    pub ask: String,
}

impl Action for ActionSyncAll {
    /// The sync is id 1: what a peer asks for once it has spoken.
    const ID: u32 = 1;

    /// The ask for how much is read at once, and nothing else.
    type Input = SyncAllRequest;
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
        let here = input.into_inner().map(|request| request.ask);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_auth::Account;
    use rorolala_protocol::{Action as _, ActionContext, ActionError, OnlyWorkspace};
    use rorolala_storage::{RorolalaStorage, StorageSet};
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;

    use super::{ActionSyncAll, SyncAllRequest, local_stores};

    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-sync-all-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_side_with_no_store_says_so() {
        let ctx = ActionContext::new_workspace_ctx(Account::default());

        assert!(matches!(local_stores(&ctx), Err(ActionError::MissingValue)));
    }

    #[test]
    fn a_side_with_handed_stores_uses_them() {
        let dir = scratch("handed");
        let store = RorolalaStorage::create(dir.join("store"));
        let set = StorageSet::sole(store);
        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_local_stores(&set);

        assert!(local_stores(&ctx).is_ok());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_workspace_side_uses_the_workspaces_store() {
        let dir = scratch("workspace");
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let ctx =
            ActionContext::new_workspace_ctx(Account::default()).with_current_workspace(&workspace);

        assert!(local_stores(&ctx).is_ok());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_vault_side_uses_the_vaults_store() {
        let dir = scratch("vault");
        Vault::create(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_current_vault(&vault);

        assert!(local_stores(&ctx).is_ok());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_sync_all_with_no_channel_is_refused() {
        let dir = scratch("process");
        let store = RorolalaStorage::create(dir.join("store"));
        let set = StorageSet::sole(store);
        let ctx = ActionContext::new_workspace_ctx(Account::default()).with_local_stores(&set);
        let input = OnlyWorkspace::from(Some(SyncAllRequest::default()));

        assert!(ActionSyncAll::process(input, ctx).await.is_err());

        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod exchange_tests {
    use rorolala_auth::{Account, Member};
    use rorolala_protocol::{Action as _, ActionContext, OnlyWorkspace};
    use rorolala_storage::{RorolalaStorage, StorageSet, store_file};
    use rorolala_utils_location::Locate as _;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;

    use crate::test_support::{run, scratch, session};

    use super::{ActionSyncAll, SyncAllRequest};

    /// What only the Workspace holds is on the Vault by the time both sides return.
    #[test]
    fn a_full_sync_carries_what_only_one_side_holds() {
        let dir = scratch("sync-all");
        let workspace_root = dir.join("workspace");
        Workspace::create(&workspace_root).unwrap();
        let workspace = Workspace::locate(&workspace_root).unwrap();
        let vault_root = dir.join("vault");
        Vault::create(&vault_root).unwrap();
        let vault = Vault::locate(&vault_root).unwrap();

        let workspace_store = RorolalaStorage::create(dir.join("workspace-store"));
        let workspace_set = StorageSet::sole(workspace_store.clone());
        let vault_store = RorolalaStorage::create(dir.join("vault-store"));
        let vault_set = StorageSet::sole(vault_store);

        let file = dir.join("content.bin");
        std::fs::write(&file, b"a content only the workspace holds").unwrap();
        let key = run(store_file(&workspace_store, &file)).unwrap();

        let (client, server) = session();
        let workspace_ctx = ActionContext::new_workspace_ctx(Account::default())
            .with_current_workspace(&workspace)
            .with_local_stores(&workspace_set)
            .with_channel(client);
        let vault_ctx = ActionContext::new_vault_ctx(Member::default())
            .with_current_vault(&vault)
            .with_local_stores(&vault_set)
            .with_channel(server);

        let (here, there) = run(async {
            tokio::join!(
                ActionSyncAll::process(
                    OnlyWorkspace::from(Some(SyncAllRequest::default())),
                    workspace_ctx
                ),
                ActionSyncAll::process(OnlyWorkspace::<SyncAllRequest>::from(None), vault_ctx),
            )
        });

        here.unwrap();
        there.unwrap();

        let held = run(vault_set.contains_keys(&[key])).unwrap();
        assert!(held.held(0), "the Vault took what the Workspace held");

        let _ = std::fs::remove_dir_all(dir);
    }
}

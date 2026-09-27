//! `sync-index-all`: making the index on each side hold the same objects as the other.
//!
//! The index is synced the way a store is, and for the same reason: each side says what it holds,
//! and what only one of them holds crosses, so one run leaves both holding everything either had.
//! What differs is what a key is made of. An index object is one object and is never cut, so there
//! is none of the manifest-and-chunks a store's content may need — a key crosses as its bytes, and
//! the other side accepts them under the key they hash to.

use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::{Blake3Hash, Key, Presence, StorageBackend as _, TransferableBackend as _};
use rorolala_utils_constants::{VAULT_INDEX_DIR, WORKSPACE_INDEX_DIR};
use rorolala_utils_location::Locate;
use rorolala_utils_progress::Direction;
use rorolala_vcs::{VCSIndex, VCSIndexError};

use super::sync_storage::{Side, carry_blob, reach_the_other_end};

/// An Action that makes the Workspace's index and the Vault's hold the same objects.
///
/// The two ends keep an index of their own, and this asks for a full sync between them: each side
/// says what it holds, and what crosses is everything the two of them hold between them. Nothing is
/// uploaded or downloaded — an object moves in whichever direction it is only on one side of.
///
/// A side that has no index is given one, since an index that is not there holds nothing and a full
/// sync of two indexes is what was asked for.
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
pub struct ActionSyncIndexAll;

impl Action for ActionSyncIndexAll {
    /// The index sync is id 2: what a peer asks for after the store's.
    const ID: u32 = 2;

    type Input = String;
    type Output = ();

    /// Works out what a full sync of the indexes is, and has the two carry it out.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault to work on, and whatever the exchange fails with — see [`sync_index`].
    async fn process(
        _: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // The index this side has: the Workspace's on the Workspace, the Vault's on the Vault.
        let local = local_index(&ctx)?;

        // What this side holds, as one run of digests, exchanged so that both ends know what the two
        // of them hold between them.
        let mine: Vec<u8> = local
            .list_exist_keys()
            .await
            .map_err(index_failed)?
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

        // Each side names the index it has; the other wrapper is empty and the other side fills it.
        let workspace = OnlyWorkspace::new(&ctx, || local.clone());
        let vault = OnlyVault::new(&ctx, || local.clone());

        sync_index(&mut ctx, workspace, vault, Both::new(required)).await
    }
}

/// The index this side works on, made if it is not there yet.
///
/// Which index that is follows from where this runs: an action taken from a Workspace works on the
/// Workspace's index, and one served by a Vault works on the Vault's — each is kept beside the store
/// of the same root, at the path the layout names. A side that has neither is not a side an action
/// can run on.
pub(crate) fn local_index(ctx: &ActionContext<'_>) -> Result<VCSIndex, ActionError> {
    if let Some(workspace) = ctx.current_workspace().into_inner() {
        return Ok(VCSIndex::create(
            workspace.get_root().join(WORKSPACE_INDEX_DIR),
        ));
    }

    if let Some(vault) = ctx.current_vault().into_inner() {
        return Ok(VCSIndex::create(vault.get_root().join(VAULT_INDEX_DIR)));
    }

    Err(ActionError::MissingValue)
}

/// Makes two indexes hold the same keys.
///
/// It is the same exchange [`sync_storage`](super::sync_storage) runs — each side says which of the
/// keys it holds, the answers cross, and then every key one side holds and the other does not is
/// carried over, the Workspace's first — with the carrying reduced to what an index object needs:
/// one object, read on the side that has it and written on the side that has not.
///
/// # Errors
///
/// Returns [`ActionError::NoChannel`] if there is nothing to exchange over,
/// [`ActionError::MissingValue`] if the context did not say which index this side has or the answers
/// did not come back, [`ActionError::MissingObject`] if a required key is held by neither side, and
/// [`ActionError::Store`] or [`ActionError::Io`] if the index could not be read or written.
async fn sync_index(
    ctx: &mut ActionContext<'_>,
    workspace: OnlyWorkspace<VCSIndex>,
    vault: OnlyVault<VCSIndex>,
    required_keys: Both<Vec<Key>>,
) -> Result<(), ActionError> {
    // Whichever side this is, it named the index it has; the other wrapper is empty.
    let local = workspace
        .into_inner()
        .or_else(|| vault.into_inner())
        .ok_or(ActionError::MissingValue)?;
    let keys = required_keys.into_inner();

    // What this side holds of what was asked for, and then what the other side holds: the two are
    // exchanged before anything moves, so both ends agree on which keys cross and which way.
    let mine = local.contains_keys(&keys).await.map_err(index_failed)?;

    let handed: OnlyWorkspace<Vec<u8>> = ctx.only_workspace(|| mine.clone().into());
    let from_vault = ctx.transfer(handed).await?;
    let answered: OnlyVault<Vec<u8>> = ctx.only_vault(|| mine.clone().into());
    let from_workspace = ctx.transfer(answered).await?;

    let theirs = Presence::from(
        from_vault
            .into_inner()
            .or_else(|| from_workspace.into_inner())
            .ok_or(ActionError::MissingValue)?,
    );

    // Whose answer is whose, from this side: the same two answers read the same way on both ends,
    // which is what lets each side work out the same list of keys to move without being told.
    let (workspace_holds, vault_holds) = if ctx.is_workspace() {
        (mine, theirs)
    } else {
        (theirs, mine)
    };

    for at in 0..keys.len() {
        if !workspace_holds.held(at) && !vault_holds.held(at) {
            return Err(ActionError::MissingObject);
        }
    }

    let progress = ctx.progress();

    let up: Vec<&Key> = keys
        .iter()
        .enumerate()
        .filter(|(at, _)| workspace_holds.held(*at) && !vault_holds.held(*at))
        .map(|(_, key)| key)
        .collect();
    let down: Vec<&Key> = keys
        .iter()
        .enumerate()
        .filter(|(at, _)| vault_holds.held(*at) && !workspace_holds.held(*at))
        .map(|(_, key)| key)
        .collect();

    let mut everything = progress.begin(
        "",
        Some(u64::try_from(up.len() + down.len()).unwrap_or(u64::MAX)),
    );

    // The Workspace's keys go first, then the Vault's. The order is the same on both ends, and it is
    // what keeps the two in step: a key crosses in one direction only, and both know which.
    if !up.is_empty() {
        let mut task = everything.spawn(
            Direction::Up,
            "",
            Some(u64::try_from(up.len()).unwrap_or(u64::MAX)),
        );

        for key in &up {
            let working = task.doing(key.hex());
            carry_index(ctx, &local, key, Side::Vault).await?;
            drop(working);

            task.advance_by(1);
            everything.advance_by(1);
        }
    }

    if !down.is_empty() {
        let mut task = everything.spawn(
            Direction::Down,
            "",
            Some(u64::try_from(down.len()).unwrap_or(u64::MAX)),
        );

        for key in &down {
            let working = task.doing(key.hex());
            carry_index(ctx, &local, key, Side::Workspace).await?;
            drop(working);

            task.advance_by(1);
            everything.advance_by(1);
        }
    }

    // As a store sync does: the Workspace waits for the Vault to say it got here, so a run that
    // returns has the Vault's side of the carrying done rather than only sent.
    reach_the_other_end(ctx).await?;

    Ok(())
}

/// Carries the object under `key` to `to`, where it is written down.
///
/// `local` is the index on this side: what it holds is read here when this is the side sending, and
/// what arrives is written into it when this is the side taking. Which of the two it is is `to`'s to
/// say — the side a value goes to is the side that does not have it. An index object is one object,
/// so what crosses for a key is its bytes and nothing else.
async fn carry_index(
    ctx: &mut ActionContext<'_>,
    local: &VCSIndex,
    key: &Key,
    to: Side,
) -> Result<(), ActionError> {
    // The side a key is going to is the side that does not have it, so the side sending is the
    // other one.
    let sending = match to {
        Side::Vault => ctx.is_workspace(),
        Side::Workspace => ctx.is_vault(),
    };

    let content = if sending {
        let Some(bytes) = local.content(key).await.map_err(index_failed)? else {
            return Err(ActionError::MissingObject);
        };

        Some(bytes)
    } else {
        None
    };

    if let Some(bytes) = carry_blob(ctx, to, content).await? {
        local.accept(key, bytes).await.map_err(index_failed)?;
    }

    Ok(())
}

/// What an index failure means to an action.
fn index_failed(error: VCSIndexError) -> ActionError {
    match error {
        VCSIndexError::Io(source) => ActionError::Io(source.into()),
        // What this side held a moment ago is not there now, which is the same answer as the check
        // having found neither side holding it.
        VCSIndexError::NotFound(_) => ActionError::MissingObject,
        error => ActionError::Store(error.to_string()),
    }
}

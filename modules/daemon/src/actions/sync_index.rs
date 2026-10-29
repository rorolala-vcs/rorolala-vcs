//! `sync-index-all`: making the index on each side hold the same objects as the other.
//!
//! The index is synced the way a store is, and for the same reason: each side says what it holds,
//! and what only one of them holds crosses, so one run leaves both holding everything either had.
//! What differs is what a key is made of. An index object is one object and is never cut, so there
//! is none of the manifest-and-chunks a store's content may need — a key crosses as its bytes, and
//! the other side accepts them under the key they hash to.

use std::collections::BTreeSet;

use rorolala_errors::BincodeError;
use rorolala_protocol::{Action, ActionContext, ActionError, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::{Blake3Hash, Key, Presence, StorageBackend as _, TransferableBackend as _};
use rorolala_utils_constants::{VAULT_INDEX_DIR, WORKSPACE_INDEX_DIR};
use rorolala_utils_location::Locate;
use rorolala_utils_progress::Task;
use rorolala_vcs::{VCSIndex, VCSIndexError};

use super::sync_storage::{
    Outgoing, Packet, Side, Taken, carry_blob, carry_blob_moving, carry_frame, direction, flush,
    opposite, packet_unread, push, reach_the_other_end, read_taken, say_taken, sending,
};

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
    /// Vault to work on, and whatever the exchange fails with — see `sync_index`.
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
/// carried over, the Workspace's first — with the carrying shaped for what an index object is: an
/// object is one object and is never cut, so a whole packet of them crosses in one value, and what
/// the taker receives it writes as one pack rather than a file apiece.
///
/// # Errors
///
/// Returns [`ActionError::NoChannel`] if there is nothing to exchange over,
/// [`ActionError::MissingValue`] if the context did not say which index this side has or the answers
/// did not come back, [`ActionError::MissingObject`] if a required key is held by neither side,
/// [`ActionError::Codec`] if a packet from the peer does not read, [`ActionError::NotConfirmed`] if
/// the side taking a direction could not keep what it took, [`ActionError::LocalFile`] if the index
/// could not be read or written, and [`ActionError::Store`] for what an index failed with otherwise.
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

    // One byte per key is already a run of bytes, so it crosses as one rather than through serde.
    let on_workspace = ctx.is_workspace();
    let to_vault = carry_frame(ctx, Side::Vault, on_workspace.then_some(&mine[..])).await?;
    let to_workspace =
        carry_frame(ctx, Side::Workspace, (!on_workspace).then_some(&mine[..])).await?;

    let theirs = Presence::from(to_vault.or(to_workspace).ok_or(ActionError::MissingValue)?);

    // Whose answer is whose, from this side: the same two answers read the same way on both ends,
    // which is what lets each side work out the same list of keys to move without being told.
    let (workspace_holds, vault_holds) = if on_workspace {
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
        carry_keys(ctx, &local, &up, Side::Vault, &mut everything, 0).await?;
    }

    if !down.is_empty() {
        let done = u64::try_from(up.len()).unwrap_or(u64::MAX);
        carry_keys(ctx, &local, &down, Side::Workspace, &mut everything, done).await?;
    }

    // As a store sync does: the Workspace waits for the Vault to say it got here, so a run that
    // returns has the Vault's side of the carrying done rather than only sent.
    reach_the_other_end(ctx).await?;

    Ok(())
}

/// Carries one direction of keys, a packet of objects at a time, and hears what became of it.
///
/// Both ends reach what the side taking it says, whatever came of the carrying: a side that returned
/// early would leave the other waiting for a value that is never coming, which is a hang rather than
/// a failure anybody is told about.
async fn carry_keys(
    ctx: &mut ActionContext<'_>,
    local: &VCSIndex,
    keys: &[&Key],
    to: Side,
    everything: &mut Task,
    base: u64,
) -> Result<(), ActionError> {
    let sender = sending(ctx, to);

    // Both ends reach the status, whatever came of the carrying: a side that returned early would
    // leave the other waiting for a value that is never coming.
    let (carried, taken) = if sender {
        (
            send_keys(ctx, local, keys, to, everything, base).await,
            None,
        )
    } else {
        match take_keys(ctx, local, keys, to, everything, base).await {
            Ok(taken) => (Ok(()), Some(taken)),
            Err(error) => (Err(error), Some(Taken::Failed)),
        }
    };

    // Said before anything is returned: the other side is waiting for it.
    let confirmed = match taken {
        Some(taken) => say_taken(ctx, opposite(to), taken).await,
        None => read_taken(ctx, opposite(to)).await,
    };

    carried?;
    confirmed
}

/// Sends `keys` to `to`, a packet of objects at a time.
///
/// An object is read as its packet is built, so a sync of many never holds all of them at once, and
/// a packet is sent as soon as it is full rather than after the whole direction has been read. What
/// one object is bigger than a packet can hold is carried as a blob after the packet that names it,
/// the same way a store's content is.
async fn send_keys(
    ctx: &mut ActionContext<'_>,
    local: &VCSIndex,
    keys: &[&Key],
    to: Side,
    everything: &mut Task,
    base: u64,
) -> Result<(), ActionError> {
    let mut task = everything.spawn(direction(to), "", None);
    let mut outgoing = Outgoing::default();

    for (at, key) in keys.iter().enumerate() {
        outgoing.completed = u32::try_from(at).unwrap_or(u32::MAX);

        // What is being carried is named as it is carried, so a reader watching one object move
        // knows which one is in hand.
        let working = task.doing(key.hex());
        let content = local.content(key).await.map_err(index_failed)?;
        drop(working);

        // What this side held a moment ago is not there now, which is the same answer as the check
        // having found neither side holding it.
        let Some(content) = content else {
            return Err(ActionError::MissingObject);
        };

        push(ctx, &mut outgoing, &mut task, to, key, false, content).await?;

        outgoing.completed = u32::try_from(at.saturating_add(1)).unwrap_or(u32::MAX);
        everything.advance(base.saturating_add(u64::from(outgoing.completed)));
    }

    if !outgoing.carried.is_empty() {
        flush(ctx, &mut outgoing, &mut task).await?;
    }

    // A direction ends where its sender says it does. An empty packet is the one thing the two ends
    // cannot mistake for entries, so it is what says there is no more.
    let last = Outgoing::last_header(outgoing.completed);

    ctx.write_frame(&[last.as_slice()]).await
}

/// Takes the packets of one direction and writes what they hold into `local` as one pack.
///
/// Nothing is written loose: an index object is small and the run of them is many, so the entries of
/// a whole direction become one pack with one index rather than a file apiece. That is also what
/// makes a direction one commit — a reader finds the objects of a sync when the pack naming them is
/// there, and not one at a time as they arrive.
///
/// What it answers is what the sender is told: which of the keys it now holds, read from what the
/// batch accepted rather than by asking the index again.
async fn take_keys(
    ctx: &mut ActionContext<'_>,
    local: &VCSIndex,
    keys: &[&Key],
    to: Side,
    everything: &mut Task,
    base: u64,
) -> Result<Taken, ActionError> {
    let mut task = everything.spawn(direction(to), "", None);
    let mut batch = local.batch();
    // What this side wrote, which is what it answers for: a batch writes what it is handed, so an
    // object it accepted is an object the index holds once the batch is published.
    let mut accepted: BTreeSet<Key> = BTreeSet::new();
    let mut failure = None;

    loop {
        // What this side holds is never looked at: the side taking a packet runs nothing of the
        // other side's, so it passes no bytes of its own.
        let bytes = carry_frame(ctx, to, None)
            .await?
            .ok_or(ActionError::MissingValue)?;
        let Some(packet) = Packet::decode(&bytes) else {
            // A packet that does not read is not the end of the direction: the loop goes on to the
            // one that is, so the side sending them is never left with nobody reading.
            failure.get_or_insert_with(packet_unread);

            continue;
        };
        let last = packet.carried.is_empty();

        // One name for a whole packet: what a reader is shown is the work in hand, and a packet is
        // what is in hand — naming every object of it would be the chatter this shape is rid of.
        let named = packet
            .carried
            .first()
            .map(|entry| entry.key.hex())
            .unwrap_or_default();
        let working = task.doing(named);

        for entry in &packet.carried {
            // An entry too big to be in the packet has its bytes after it, as the blob it has
            // always been, so it is read either way: a side that stopped reading here would leave
            // the other writing into a channel nobody drains.
            let followed;
            let bytes: &[u8] = if entry.lead.large() {
                followed = carry_blob_moving(ctx, to, None, Some(&mut task))
                    .await?
                    .ok_or(ActionError::MissingValue)?;

                &followed
            } else {
                entry.bytes
            };

            if failure.is_some() {
                continue;
            }

            // An index object is one object and is never cut, so a packet that says what it carries
            // is a manifest is not one this sync sent.
            if entry.lead.manifest() {
                failure = Some(ActionError::Codec(BincodeError::new(
                    "an index sync carries objects, not manifests".to_owned(),
                )));

                continue;
            }

            match batch.accept(&entry.key, bytes) {
                Ok(()) => {
                    accepted.insert(entry.key);
                }
                Err(error) => failure = Some(index_failed(error)),
            }
        }

        drop(working);

        task.advance_by(packet.bytes());
        everything.advance(base.saturating_add(u64::from(packet.finished)));

        if last {
            break;
        }
    }

    // The batch becomes an index's here, and a direction is not done until it has: a reader that
    // arrives before this finds the index it found before the sync started rather than one whose
    // objects are part of the way through being written. What failed is not published.
    if failure.is_none()
        && let Err(error) = batch.publish().await
    {
        failure = Some(index_failed(error));
    }

    if let Some(error) = failure {
        return Err(error);
    }

    // What this side holds is read from what it has just written rather than by asking the index
    // again: an object the batch accepted is an object the pack now names.
    let missing: Vec<Key> = keys
        .iter()
        .map(|key| **key)
        .filter(|key| !accepted.contains(key))
        .collect();

    Ok(Taken::Held(missing))
}

/// What an index failure means to an action.
fn index_failed(error: VCSIndexError) -> ActionError {
    match error {
        // An index that could not be read or written is a file, not a channel: what a caller does
        // about it is give access to a place, and the two are told apart because they are acted on
        // differently.
        VCSIndexError::Io(source) => ActionError::LocalFile(source.into()),
        // What this side held a moment ago is not there now, which is the same answer as the check
        // having found neither side holding it.
        VCSIndexError::NotFound(_) => ActionError::MissingObject,
        error => ActionError::Store(error.to_string()),
    }
}

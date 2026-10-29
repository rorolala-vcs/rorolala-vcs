//! Syncing two stores, so that each ends up holding the keys that were asked for.
//!
//! This is a primitive rather than an [`Action`](rorolala_protocol::Action): what an action
//! decides is what to ask for, and this is the asking itself, made of the protocol's own
//! functions — a value is built on the side that holds it and carried with
//! [`transfer`](ActionContext::transfer) — so an action can ask for a set of keys without
//! knowing how a key moves.
//!
//! Nothing here is upload or download. Each side says which of the keys it holds, the two
//! answers are exchanged, and then every key one side holds and the other does not is carried
//! over — in the Workspace's direction first, then in the Vault's — so one pass leaves both
//! sides holding all of them, whichever way each one happened to travel.
//!
//! A key is more than one object: a content kept as chunks is a manifest that names them, and
//! the chunks themselves, and what is carried for such a key is exactly that — the manifest and
//! then each chunk it names — rather than the content, so the other store keeps the chunks it
//! was given instead of cutting the whole thing again. What a store already holds is not written
//! twice: see [`RorolalaStorage`]'s write, which passes over what is there.
//!
//! # Why one value carries many entries
//!
//! A store of small chunks is what this has to serve: a `.pptx` cut at its member boundaries, or
//! text cut at its lines, arrives as thousands of entries of a few kilobytes. Carried one entry per
//! value, each of them paid the whole of what crossing costs — a length value, a payload value, the
//! records each of those becomes on an encrypted stream, a progress name, and a write of its own on
//! the receiving side. None of that is about the bytes: it is about the number of entries, and it
//! does not shrink when the entries do.
//!
//! So a value carries a *packet*: as many entries as fit, cut by the side that has them. Both ends
//! do not have to agree on the cut — the side taking them learns what a packet holds from the packet
//! itself, and knows a direction is over when a packet arrives holding nothing. What they do agree
//! on, without being told, is which entries cross and in which direction: both read that from the
//! two presence answers. The entries of a packet are written into the receiving store as one batch,
//! which lays them down in packs rather than one file each — see
//! [`begin_batch`](RorolalaStorage::begin_batch).
//!
//! A direction is not given a total in bytes: what it is about to carry is only known by reading
//! what it holds, and that read is the carrying itself — it was over a third of a sync of small
//! entries, spent on one number before a byte moved. So the bar a direction draws is drawn from what
//! crosses — a packet at a time, by the bytes it holds — while the run's own bar is counted in units,
//! which both ends know before anything moves.

use std::collections::{BTreeMap, BTreeSet};

use rorolala_auth::MAX_PLAINTEXT;
use rorolala_errors::BincodeError;
use rorolala_protocol::{ActionContext, ActionError, Both, MAX_FRAME, OnlyVault, OnlyWorkspace};
use rorolala_storage::{
    BLAKE3_HASH_LEN, Blake3Hash, Chunk, Error as StoreError, Key, Manifest, ObjectBatch, Presence,
    RorolalaStorage, StorageBackend as _,
};
use rorolala_utils_progress::{Direction, Task};

/// Which side a key is being carried to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    /// To the Vault, which is where a Workspace's values go.
    Vault,
    /// To the Workspace.
    Workspace,
}

/// How many bytes of a blob one value carries.
///
/// A value crosses as one frame, and a frame carries at most [`MAX_FRAME`] bytes — so a content
/// bigger than that crosses as several values, and this is how much of it each of them takes. What
/// is held back is the room the framing itself takes, so that a piece of this much always fits the
/// frame it is put in, whatever the value is wrapped in on the way out.
const PIECE: usize = MAX_FRAME - FRAMING;

/// How much room the framing around one value is left.
const FRAMING: usize = 1024;

/// How many bytes of entries one packet holds.
///
/// A packet and the framing around it fit one record of the session: a record holds a plaintext of so
/// many bytes, and the room a frame's length takes and the packet's own header are taken out of that
/// first. One packet is then one record — one sealing, one write — rather than being cut across two.
const PACKET_BYTES: usize = MAX_PLAINTEXT - size_of::<u32>() - PACKET_HEADER;

/// How many entries one packet holds, whatever their size.
///
/// A packet of many empty entries would still cost a record each to carry, and the count is what
/// bounds the work of writing one packet out: a content of ten thousand empty chunks is ten
/// thousand entries, and this is what keeps them from all being one value.
///
/// The byte bound is what keeps a packet's memory in hand; this one only has to keep a packet of
/// empty entries from being one enormous value. It is set well above what a packet of small chunks
/// holds, so what stops a packet at four kilobytes an entry is its bytes and not its count.
const PACKET_ENTRIES: usize = 1024;

/// How much of one entry a packet does not spend on its bytes: its kind, its key and its length.
const ENTRY_HEADER: usize = size_of::<u8>() + BLAKE3_HASH_LEN + size_of::<u32>();

/// How much a packet spends before its entries: which unit the direction has reached, and how many
/// entries follow.
const PACKET_HEADER: usize = 2 * size_of::<u32>();

// A packet has to be able to cross as one record of the session: its bytes, the frame's length and
// its own header. The two constants are derived from each other, and this says so where a change to
// either would break it.
const _: () = assert!(PACKET_BYTES + size_of::<u32>() + PACKET_HEADER <= MAX_PLAINTEXT);

/// How many reads of a content's chunks are kept in flight at once when a caller names no number.
///
/// A store read waits on a disk through a thread that can block, so the work of one read is mostly
/// the waiting; a few at a time is what turns that waiting into overlap. The number bounds what is
/// held in memory rather than what is asked for: a content of thousands of chunks would otherwise be
/// read into as many buffers at once as it has chunks. A caller that names its own — a run with
/// `--concurrency`, or the window a Vault grants — replaces it.
pub(crate) const DEFAULT_WINDOW: usize = 8;

/// How much of what a key names one unit carries.
///
/// A content kept as chunks is a manifest that names them: a caller that has the manifest can carry
/// the manifest and each chunk as units of their own, so the chunks of one content travel over as
/// many lanes as there are, rather than one behind another. A caller that does not have it asks for
/// the whole, and the two ends expand it here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitKind {
    /// All of what the key names: the manifest and the chunks it names, or the object.
    Whole,
    /// Only the manifest the key names, which the taker records as a manifest.
    Manifest,
    /// Only the object the key names, which the taker writes as an object.
    Object,
    /// The manifest if the key has one and the object otherwise, which the taker asks for when it
    /// does not know how the content is kept: it learns from what arrives, and a content kept as
    /// chunks is then asked for a chunk at a time.
    Discover,
}

/// One thing to carry: a key, and how much of it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unit {
    /// The key being carried.
    pub(crate) key: Key,
    /// How much of what it names is carried.
    pub(crate) kind: UnitKind,
}

impl Unit {
    /// The whole of what `key` names, left for the two ends to expand.
    pub(crate) const fn whole(key: Key) -> Self {
        Self {
            key,
            kind: UnitKind::Whole,
        }
    }
}

/// Makes two stores hold the same keys.
///
/// `workspace` and `vault` are the stores each side works on: on the Workspace the first holds
/// the store and the second is empty, and on the Vault the other way round — each side names the
/// store it has, and the empty one is not mistaken for a store that holds nothing. `required_keys`
/// are the keys both sides must end up holding, whatever they hold to begin with.
///
/// What is asked for is a key, and a key is the whole of what it names: a content kept as chunks
/// is its manifest and the chunks the manifest names, and both cross. So a store that is given a
/// key ends up holding it the way the other store had it, rather than cutting it over again.
///
/// The exchange is one round of asking and one of carrying:
///
/// 1. each side says which of `required_keys` it holds, and the two answers cross, so both sides
///    know both;
/// 2. a key neither side holds is reported as [`ActionError::MissingObject`] — there is nowhere
///    for it to come from, and moving the others would only hide that;
/// 3. what the Workspace holds and the Vault does not is carried to the Vault, and then what the
///    Vault holds and the Workspace does not is carried to the Workspace.
///
/// Both sides run this, and run it in step: which keys cross, and which way, is read from the two
/// answers, so the two ends reach the same list in the same order without either being told.
///
/// # Errors
///
/// Returns [`ActionError::NoChannel`] if there is nothing to exchange over,
/// [`ActionError::MissingValue`] if the context did not say which store this side has or the
/// answers did not come back, [`ActionError::MissingObject`] if a required key is held by neither
/// side, [`ActionError::Codec`] if a manifest or a packet from the peer does not read, and
/// [`ActionError::LocalFile`] if a store could not be read or written, and
/// [`ActionError::Store`] for what a store failed with otherwise.
pub async fn sync_storage(
    ctx: &mut ActionContext<'_>,
    workspace: OnlyWorkspace<RorolalaStorage>,
    vault: OnlyVault<RorolalaStorage>,
    required_keys: Both<Vec<Key>>,
) -> Result<(), ActionError> {
    let units = required_keys
        .into_inner()
        .into_iter()
        .map(Unit::whole)
        .collect();

    sync_units(ctx, workspace, vault, Both::new(units), DEFAULT_WINDOW).await
}

/// Makes two stores hold the same units.
///
/// This is the whole of [`sync_storage`] with how much is read at once left to the caller: a
/// required key is a unit naming the whole of what it holds, while a manifest and its chunks are
/// units of their own, which is what lets the chunks of one content be read as many at a time as the
/// caller arranges. The presence each side reports is per unit, so a unit already held is passed over
/// whichever way the content under its key is kept.
///
/// `window` is how many reads of the sending side are kept in flight at once — see
/// [`DEFAULT_WINDOW`]. It is the side that sends a direction that reads, so it is that side's
/// number: a caller asking a Vault to send it content asks with the same one, and the Vault holds it
/// to what it serves.
///
/// # Errors
///
/// Returns what [`sync_storage`] does.
pub(crate) async fn sync_units(
    ctx: &mut ActionContext<'_>,
    workspace: OnlyWorkspace<RorolalaStorage>,
    vault: OnlyVault<RorolalaStorage>,
    required: Both<Vec<Unit>>,
    window: usize,
) -> Result<(), ActionError> {
    // Whichever side this is, it named the store it has; the other wrapper is empty.
    let local = workspace
        .into_inner()
        .or_else(|| vault.into_inner())
        .ok_or(ActionError::MissingValue)?;
    let units = required.into_inner();

    // What this side holds of what was asked for, and then what the other side holds: the two are
    // exchanged before anything moves, so both ends agree on which units cross and which way.
    let mine = presence(&local, &units).await?;

    // One byte per unit is already a run of bytes, so it crosses as one: what an answer says is a
    // position per key, and putting that through serde would be a visit per position for a shape that
    // is a `Vec<u8>` and nothing else. Each side sends the answer to the side that is not its own.
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

    for at in 0..units.len() {
        if !workspace_holds.held(at) && !vault_holds.held(at) {
            return Err(ActionError::MissingObject);
        }
    }

    // What has to move is read from the two answers, so a run says how much of it there is
    // before it starts rather than as it finds out. Nothing has moved yet, so this is the first
    // thing said, and a reader watching from the start sees a bar that is empty and honest
    // rather than one that fills up to a length it did not know it had.
    let progress = ctx.progress();

    let up: Vec<&Unit> = units
        .iter()
        .enumerate()
        .filter(|(at, _)| workspace_holds.held(*at) && !vault_holds.held(*at))
        .map(|(_, unit)| unit)
        .collect();
    let down: Vec<&Unit> = units
        .iter()
        .enumerate()
        .filter(|(at, _)| vault_holds.held(*at) && !workspace_holds.held(*at))
        .map(|(_, unit)| unit)
        .collect();

    let mut everything = progress.begin(
        "",
        Some(u64::try_from(up.len() + down.len()).unwrap_or(u64::MAX)),
    );

    // The Workspace's keys go first, then the Vault's. The order is the same on both ends, and it is
    // what keeps the two in step: a key crosses in one direction only, and both know which.
    if !up.is_empty() {
        carry_direction(ctx, &local, &up, Side::Vault, &mut everything, 0, window).await?;
    }

    if !down.is_empty() {
        let done = u64::try_from(up.len()).unwrap_or(u64::MAX);
        carry_direction(
            ctx,
            &local,
            &down,
            Side::Workspace,
            &mut everything,
            done,
            window,
        )
        .await?;
    }

    // Both ends have carried everything they had to, and this is the point they reach once they
    // have. It is what a run waits on: the entries of a direction are written into the receiving
    // store's packs, and a pack is not a store's until its index is published — which the side taking
    // them does before it answers here. Waiting for the Vault to say it got here leaves the sync only
    // once both ends have nothing left to do.
    reach_the_other_end(ctx).await?;

    Ok(())
}

/// Which of `units` this side holds.
///
/// Every unit is asked about at once where one call can ask: [`contains_keys`] answers a whole batch
/// from a run of lookups kept going a few at a time, and asking it one key per call is asking it to
/// start that run over for each — which for a content of thousands of chunks is what made a large
/// sync slow before it moved a byte. A manifest unit asks a different question — whether a *manifest*
/// is there, which is not the same as whether the content is — so those are asked one at a time,
/// and there are as many of them as the caller named manifest tokens.
///
/// [`contains_keys`]: StorageBackend::contains_keys
async fn presence(local: &RorolalaStorage, units: &[Unit]) -> Result<Presence, ActionError> {
    let mut presence = Presence::all_missing(units.len());

    let content: Vec<usize> = units
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.kind != UnitKind::Manifest)
        .map(|(at, _)| at)
        .collect();
    let keys: Vec<Key> = content.iter().map(|at| units[*at].key).collect();
    let held = local.contains_keys(&keys).await.map_err(store_failed)?;

    for (at, index) in content.iter().enumerate() {
        if held.held(at) {
            presence.set_held(*index, true);
        }
    }

    for (at, unit) in units.iter().enumerate() {
        if unit.kind == UnitKind::Manifest
            && local
                .holds_manifest(&unit.key)
                .await
                .map_err(store_failed)?
        {
            presence.set_held(at, true);
        }
    }

    Ok(presence)
}

/// Carries one direction: the side that has the units sends them, and the side taking them writes
/// them down.
///
/// `base` is how many units of the whole run this direction comes after, so that a bar drawn in
/// units moves by the same amount on both ends: two directions under one task are one bar, and the
/// second starts where the first left off.
///
/// # Errors
///
/// Returns what a store read, a store write and the exchange fail with — see [`ActionError`].
async fn carry_direction(
    ctx: &mut ActionContext<'_>,
    local: &RorolalaStorage,
    units: &[&Unit],
    to: Side,
    everything: &mut Task,
    base: u64,
    window: usize,
) -> Result<(), ActionError> {
    let sender = sending(ctx, to);

    // Both ends reach the status, whatever came of the carrying: a side that returned early would
    // leave the other waiting for a value that is never coming, which is a hang rather than a
    // failure anybody is told about. What the side taking it says is a status byte and then, when it
    // could not produce what it took, the digests of the units it could not.
    let (carried, taken) = if sender {
        (
            send_direction(ctx, local, units, to, everything, base, window).await,
            None,
        )
    } else {
        match take_direction(ctx, local, units, to, everything, base).await {
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

/// What the side taking a direction came to.
///
/// A direction is carried without being acknowledged, so what the taker says at the end of one is
/// the whole of what its sender knows: it is a status, and then the units the taker cannot produce,
/// which is what a sender acts on rather than "it failed" alone.
pub(crate) enum Taken {
    /// It holds every unit of what was carried, or the units it cannot produce.
    Held(Vec<Key>),
    /// It could not carry out the taking at all, and says so rather than naming units.
    Failed,
}

impl Taken {
    /// What this is said as: a status byte, and then the digests of the units it cannot produce.
    fn said(&self) -> Vec<u8> {
        match self {
            Self::Held(missing) if missing.is_empty() => vec![0_u8],
            Self::Held(missing) => {
                let mut said = vec![2_u8];
                said.extend(missing.iter().flat_map(|key| *key.digest()));

                said
            }
            Self::Failed => vec![1_u8],
        }
    }
}

/// Says what the side taking a direction came to.
///
/// It is said before anything is returned: the other side is waiting for it, and a side that
/// returned early would leave it waiting for a value that is never coming.
///
/// A status byte and the digests it names are one run of bytes, so the whole of it is one frame.
///
/// # Errors
///
/// Returns whatever the exchange fails with.
pub(crate) async fn say_taken(
    ctx: &mut ActionContext<'_>,
    to: Side,
    taken: Taken,
) -> Result<(), ActionError> {
    let said = taken.said();

    carry_frame(ctx, to, Some(said.as_slice())).await.map(drop)
}

/// Reads what the side that took a direction said it came to.
///
/// A unit it cannot produce is [`ActionError::NotConfirmed`] naming the first of them, since what a
/// caller does about it is look at that content; a taking that failed outright is the same error
/// without a unit to name.
///
/// # Errors
///
/// Returns [`ActionError::NotConfirmed`] when the side taking it could not, and whatever the
/// exchange fails with.
pub(crate) async fn read_taken(ctx: &mut ActionContext<'_>, to: Side) -> Result<(), ActionError> {
    let said = carry_frame(ctx, to, None)
        .await?
        .ok_or(ActionError::MissingValue)?;

    match said.split_first() {
        // The side taking it could not carry what it took, so neither side may say it did.
        Some((1, _)) => Err(ActionError::NotConfirmed(
            "the side taking it failed".to_owned(),
        )),
        // These are the units it took and cannot produce again, and one is enough to fail the run.
        Some((2, rest)) => {
            first_of(rest).map_or(Ok(()), |key| Err(ActionError::NotConfirmed(key.hex())))
        }
        _ => Ok(()),
    }
}

/// The side a direction does not name.
///
/// A value is sent by the side that owns it, so what a side taking a direction says is carried the
/// other way: it is the Workspace's wrapper the Vault sends, and the Vault's the Workspace sends,
/// whichever way the units themselves went.
pub(crate) const fn opposite(to: Side) -> Side {
    match to {
        Side::Vault => Side::Workspace,
        Side::Workspace => Side::Vault,
    }
}

/// The first key a confirmation named, when it named one.
pub(crate) fn first_of(said: &[u8]) -> Option<Key> {
    let first = said.as_chunks::<{ size_of::<Blake3Hash>() }>().0.first()?;

    Some(Key::new(*first))
}

/// Whether this side is the one that has the units, which is the side `to` does not name.
///
/// The side a unit goes to is the side that does not have it, so the side sending is the other one.
pub(crate) const fn sending(ctx: &ActionContext<'_>, to: Side) -> bool {
    match to {
        Side::Vault => ctx.is_workspace(),
        Side::Workspace => ctx.is_vault(),
    }
}

/// The direction a side's work moves, as a bar draws it.
pub(crate) const fn direction(to: Side) -> Direction {
    match to {
        Side::Vault => Direction::Up,
        Side::Workspace => Direction::Down,
    }
}

/// Sends `units` to `to`, a packet at a time.
///
/// What is read is read here and now: a unit is expanded as its packet is built, so a content of
/// many chunks costs one window of them in hand rather than all of it, and a packet is sent as soon
/// as it is full rather than after the whole direction has been read.
async fn send_direction(
    ctx: &mut ActionContext<'_>,
    local: &RorolalaStorage,
    units: &[&Unit],
    to: Side,
    everything: &mut Task,
    base: u64,
    window: usize,
) -> Result<(), ActionError> {
    let mut task = everything.spawn(direction(to), "", None);
    let mut outgoing = Outgoing::default();
    let mut failure = None;

    for (at, unit) in units.iter().enumerate() {
        outgoing.completed = u32::try_from(at).unwrap_or(u32::MAX);

        // What is being carried is named as it is carried: a unit of a cut content is a chunk, and a
        // reader watching one content move wants to see which one is in hand.
        let working = task.doing(unit.key.hex());
        let added = add_unit(ctx, local, unit, to, &mut outgoing, &mut task, window).await;
        drop(working);

        if let Err(error) = added {
            failure = Some(error);

            break;
        }

        // Said once the unit's entries are all in hand, so a packet flushed by the next unit's first
        // entry carries the count that includes this one.
        outgoing.completed = u32::try_from(at.saturating_add(1)).unwrap_or(u32::MAX);
        everything.advance(base.saturating_add(u64::from(outgoing.completed)));
    }

    if failure.is_none()
        && !outgoing.carried.is_empty()
        && let Err(error) = flush(ctx, &mut outgoing, &mut task).await
    {
        failure = Some(error);
    }

    // A direction ends where its sender says it does. An empty packet is the one thing the two ends
    // cannot mistake for entries — a packet holding none is one nothing was added to — so it is what
    // says there is no more, whatever the two of them happen to have counted.
    //
    // It is sent even when the carrying failed: the side taking it is looking for the end of the
    // direction, and a side that stops without saying it is one the other waits on for good.
    let last = Outgoing::last_header(outgoing.completed);
    let ended = ctx.write_frame(&[last.as_slice()]).await;

    failure.or_else(|| ended.err()).map_or(Ok(()), Err)
}

/// Takes the packets of one direction and writes what they hold into `local`.
///
/// Nothing is written loose: the entries of a packet go into a
/// [batch](RorolalaStorage::begin_batch), which lays them down in packs and gives those packs their
/// indexes once the direction is over. A store taking a thousand small chunks that way writes two
/// files rather than a thousand, at the cost of what it holds not being findable until the direction
/// ends — which is exactly when the side that sent them is told it may return.
///
/// What it answers is what the sender is told: which units it now holds, read from what it wrote
/// rather than from a second reading of the store — see [`holds`].
async fn take_direction(
    ctx: &mut ActionContext<'_>,
    local: &RorolalaStorage,
    units: &[&Unit],
    to: Side,
    everything: &mut Task,
    base: u64,
) -> Result<Taken, ActionError> {
    // The bar this direction draws is drawn from what crosses: nothing said how long it would be, so
    // each packet moves it by the bytes it holds and the run's own bar is counted in units.
    let mut task = everything.spawn(direction(to), "", None);

    let mut batch = local.begin_batch().await;
    // What this side wrote, which is what it will answer for. Objects and manifests are kept apart
    // because a key can be either: a content kept as chunks is a manifest under its own key and its
    // chunks are objects of their own, and "the object arrived" and "the manifest arrived" are two
    // different answers about one key.
    let mut objects: BTreeSet<Key> = BTreeSet::new();
    let mut manifests: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
    let mut failure = None;

    loop {
        // What this side holds is never looked at: the side taking a packet runs nothing of the other
        // side's, so it passes no bytes of its own.
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
        // what is in hand — naming every entry of it would be a name per chunk, which is the chatter
        // this shape exists to be rid of.
        let named = packet
            .carried
            .first()
            .map(|entry| entry.key.hex())
            .unwrap_or_default();
        let working = task.doing(named);

        for entry in &packet.carried {
            // An entry too big to be in the packet has its bytes after it, as the blob it has always
            // been — a length and then pieces — so it is read either way: a side that stopped
            // reading here would leave the other writing into a channel nobody drains.
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

            if entry.lead.manifest() {
                match Manifest::decode(bytes) {
                    Ok(manifest) => match batch.add_manifest(&entry.key, &manifest).await {
                        Ok(()) => {
                            manifests.insert(
                                entry.key,
                                manifest
                                    .chunks()
                                    .iter()
                                    .map(rorolala_storage::Chunk::key)
                                    .collect(),
                            );
                        }
                        Err(error) => failure = Some(store_failed(error)),
                    },
                    Err(error) => {
                        failure = Some(ActionError::Codec(BincodeError::new(error.to_string())));
                    }
                }

                continue;
            }

            // The key is the object's own hash, so bytes that do not hash to it are not the object
            // that was asked for, whatever they say for themselves. It is checked here, once, as the
            // bytes arrive: what is written into the store is then one entry the side taking it need
            // not read back to believe.
            if Key::new(*blake3::hash(bytes).as_bytes()) != entry.key {
                failure = Some(ActionError::Store(format!(
                    "what arrived under `{}` is not that object",
                    entry.key.hex()
                )));

                continue;
            }

            match batch.add(&entry.key, bytes).await {
                Ok(()) => {
                    objects.insert(entry.key);
                }
                Err(error) => failure = Some(store_failed(error)),
            }
        }

        drop(working);

        task.advance_by(packet.bytes());
        everything.advance(base.saturating_add(u64::from(packet.finished)));

        if last {
            break;
        }
    }

    // The packs become the store's here, and a direction is not done until they have: a reader that
    // arrives before this finds the store it found before the sync started, rather than a store whose
    // manifests name chunks that cannot be read yet. What failed is not published: half of a content
    // is not a content, and what it leaves is the garbage a write cut short leaves.
    if failure.is_none()
        && let Err(error) = batch.publish().await
    {
        failure = Some(store_failed(error));
    }

    if let Some(error) = failure {
        return Err(error);
    }

    // What this side holds is read from what it has just written rather than from a second reading of
    // the store: every entry it accepted is an entry it wrote, and publishing made them its own.
    let missing: Vec<Key> = units
        .iter()
        .filter(|unit| !holds(unit, &objects, &manifests))
        .map(|unit| unit.key)
        .collect();

    Ok(Taken::Held(missing))
}

/// Whether this side holds everything `unit` names, read from what it has just written.
///
/// The presence a direction opens with says which side has a unit, and the side that has it sends the
/// whole of it — the manifest and every chunk it names, or the object. So "every entry of this unit
/// was written" is the same question as "this side holds it", and it is answered from the packets
/// rather than by a second reading of the store.
///
/// What this rests on is that a batch writes what it is handed, even what the store already holds: a
/// key a batch accepted is a key in the store. Should a batch ever start passing over what is already
/// there, this has to count "was already there" as held as well, or it would report a unit missing
/// that the store has.
fn holds(unit: &Unit, objects: &BTreeSet<Key>, manifests: &BTreeMap<Key, Vec<Key>>) -> bool {
    match unit.kind {
        UnitKind::Manifest => manifests.contains_key(&unit.key),
        UnitKind::Object => objects.contains(&unit.key),
        // The whole of what the key names: the object itself, or the manifest and every chunk the
        // manifest names. A caller that does not know which it is — `Discover` — asked for the same
        // whole, so it is answered the same way.
        UnitKind::Whole | UnitKind::Discover => {
            objects.contains(&unit.key)
                || manifests
                    .get(&unit.key)
                    .is_some_and(|chunks| chunks.iter().all(|chunk| objects.contains(chunk)))
        }
    }
}

/// What a packet that does not read is reported as.
pub(crate) fn packet_unread() -> ActionError {
    ActionError::Codec(BincodeError::new("a packet does not read".to_owned()))
}

/// Reads a content's chunks with its window always full.
///
/// A packet is sent when it is full, which is a wait nothing about the disk asks for: the reads for
/// the packet after it can already be in flight. A window filled once per content and taken in order
/// does exactly that — one read is taken as each one is handed over, so the number in flight is the
/// window rather than the window on average.
/// One chunk being read: the key it is for, and the read itself.
type Reading = (Key, tokio::task::JoinHandle<Result<Vec<u8>, StoreError>>);

/// Reads a content's chunks with its window always full, out of one opened batch.
///
/// A packet is sent when it is full, which is a wait nothing about the disk asks for: the reads for
/// the packet after it can already be in flight. A window filled as each read is handed over does
/// exactly that, so the number of reads in flight is the window rather than the window on average.
///
/// The batch is what makes each of those reads cheap: the packs are opened once for the whole
/// content, rather than opened, seeked into and read once per chunk — see
/// [`RorolalaStorage::open_objects`].
struct Prefetch {
    /// The content's chunks, in the order the manifest names them, already planned and opened.
    batch: std::sync::Arc<ObjectBatch>,
    /// The keys in the order the manifest names them.
    keys: Vec<Key>,
    /// How many of them have been begun.
    begun: usize,
    /// The reads in flight, in the order they were begun.
    reads: std::collections::VecDeque<Reading>,
    /// How many reads are kept in flight at once.
    window: usize,
}

impl Prefetch {
    /// Opens a batch of `keys`, with `window` reads in flight.
    async fn open(
        store: RorolalaStorage,
        keys: Vec<Key>,
        window: usize,
    ) -> Result<Self, ActionError> {
        let batch = store.open_objects(&keys).await.map_err(store_failed)?;

        Ok(Self {
            batch: std::sync::Arc::new(batch),
            keys,
            begun: 0,
            reads: std::collections::VecDeque::new(),
            window: window.max(1),
        })
    }

    /// Keeps the window full.
    fn fill(&mut self) {
        while self.reads.len() < self.window {
            let Some(key) = self.keys.get(self.begun).copied() else {
                return;
            };
            self.begun += 1;

            let batch = std::sync::Arc::clone(&self.batch);

            // The side taking what this sends checks every entry against the key it arrived under,
            // so hashing it here as well is a second pass over the content for a check the other
            // side is about to make.
            self.reads.push_back((
                key,
                tokio::spawn(async move { batch.read_unchecked(&key).await }),
            ));
        }
    }

    /// The next chunk that has been read, in the order the manifest names them.
    async fn next(&mut self) -> Result<Option<(Key, Vec<u8>)>, ActionError> {
        self.fill();

        let Some((key, read)) = self.reads.pop_front() else {
            return Ok(None);
        };
        let bytes = read
            .await
            .map_err(|error| ActionError::Store(error.to_string()))?
            .map_err(store_failed)?;

        // Filled before this one is handed over, so the wait it takes is a wait the next read is
        // already spending on the disk.
        self.fill();

        Ok(Some((key, bytes)))
    }
}

/// Adds the entries of one unit to the packet being built, sending it when it is full.
///
/// `window` is how many of a cut content's chunks are read at once — see [`DEFAULT_WINDOW`].
async fn add_unit(
    ctx: &mut ActionContext<'_>,
    local: &RorolalaStorage,
    unit: &Unit,
    to: Side,
    outgoing: &mut Outgoing,
    task: &mut Task,
    window: usize,
) -> Result<(), ActionError> {
    // What a unit is expanded into is decided by what this side holds: a content kept as chunks is
    // its manifest and the chunks it names, and one kept whole is the object. Which of the two it is
    // is said by the entry that carries it, so the side taking it never has to ask.
    let manifest = local.manifest_of(&unit.key).await.map_err(store_failed)?;

    match unit.kind {
        UnitKind::Manifest => {
            let Some(manifest) = manifest else {
                // What this side held a moment ago is not there now, which is the same answer as the
                // check having found neither side holding it.
                return Err(ActionError::MissingObject);
            };

            push(ctx, outgoing, task, to, &unit.key, true, manifest.encode()).await
        }
        // A whole unit is the manifest and then every chunk it names, and an object unit is the
        // object whatever the store keeps beside it: the caller asked for one thing, so one thing
        // crosses.
        UnitKind::Whole => match manifest {
            None => {
                let bytes = local
                    .read_object_unchecked(&unit.key)
                    .await
                    .map_err(store_failed)?;

                push(ctx, outgoing, task, to, &unit.key, false, bytes).await
            }
            Some(manifest) => {
                push(ctx, outgoing, task, to, &unit.key, true, manifest.encode()).await?;

                // The chunks are read with the window always full, and each read outlives the one
                // in hand: a store read is a hand-over to a thread that blocks, and waiting for the
                // whole of one window before beginning the next is a sync that spends the sending of
                // a packet with the disk idle. What is sent is still in the manifest's order — the
                // reads are taken in the order they were begun, not the order they finish.
                let keys: Vec<Key> = manifest.chunks().iter().map(Chunk::key).collect();
                let mut prefetch = Prefetch::open(local.clone(), keys, window).await?;

                while let Some((key, bytes)) = prefetch.next().await? {
                    push(ctx, outgoing, task, to, &key, false, bytes).await?;
                }

                Ok(())
            }
        },
        UnitKind::Object => {
            let bytes = local
                .read_object_unchecked(&unit.key)
                .await
                .map_err(store_failed)?;

            push(ctx, outgoing, task, to, &unit.key, false, bytes).await
        }
        // What is sent is decided here rather than announced: the taker learns from the entry that
        // carries it whether what arrived is a manifest or an object, so there is no lead value and
        // no round of asking.
        UnitKind::Discover => {
            if let Some(manifest) = manifest {
                push(ctx, outgoing, task, to, &unit.key, true, manifest.encode()).await
            } else {
                let bytes = local
                    .read_object_unchecked(&unit.key)
                    .await
                    .map_err(store_failed)?;

                push(ctx, outgoing, task, to, &unit.key, false, bytes).await
            }
        }
    }
}

/// Adds one entry to the packet being built, sending the packet first when it has no room left.
///
/// An entry bigger than one value can hold does not go into a packet at all: it crosses as the blob
/// it has always been — a length and then pieces of at most [`PIECE`] — with the packet naming it by
/// key and saying nothing else about it. A content kept whole can be any size at all, so one that is
/// bigger than a frame is the case that piece-splitting exists for and this shape would otherwise
/// lose.
pub(crate) async fn push(
    ctx: &mut ActionContext<'_>,
    outgoing: &mut Outgoing,
    task: &mut Task,
    to: Side,
    key: &Key,
    manifest: bool,
    bytes: Vec<u8>,
) -> Result<(), ActionError> {
    if bytes.len() > PIECE {
        return push_large(ctx, outgoing, task, to, key, manifest, bytes).await;
    }

    let over = !outgoing.carried.is_empty() && !outgoing.room_for(bytes.len());
    if over {
        flush(ctx, outgoing, task).await?;
    }

    outgoing.add(Carried::new(*key, Lead::of(manifest, false), bytes));

    Ok(())
}

/// Adds one entry whose bytes are too many for one value, and carries them as a blob.
///
/// The packet that names it goes first and on its own: the side taking these runs the same calls in
/// the same order, so a blob that followed a packet still being built would be read as its entries
/// rather than as the blob they name.
async fn push_large(
    ctx: &mut ActionContext<'_>,
    outgoing: &mut Outgoing,
    task: &mut Task,
    to: Side,
    key: &Key,
    manifest: bool,
    bytes: Vec<u8>,
) -> Result<(), ActionError> {
    if !outgoing.carried.is_empty() {
        flush(ctx, outgoing, task).await?;
    }

    outgoing.add(Carried::new(*key, Lead::of(manifest, true), Vec::new()));
    flush(ctx, outgoing, task).await?;

    carry_blob_moving(ctx, to, Some(bytes), Some(task)).await?;

    Ok(())
}

/// Sends what the packet being built holds, and starts another.
///
/// The packet goes out as one frame made of the parts it already holds — its header, then each
/// entry's header and bytes — so what crosses is not a buffer built to carry it. Only the side
/// sending a direction builds packets, which is why this writes a frame where a value would be
/// carried: there is no direction for the other end to read here.
pub(crate) async fn flush(
    ctx: &mut ActionContext<'_>,
    outgoing: &mut Outgoing,
    task: &mut Task,
) -> Result<(), ActionError> {
    let carried = u64::try_from(outgoing.bytes).unwrap_or(u64::MAX);
    let (header, entries) = outgoing.parts();

    let mut parts = Vec::with_capacity(entries.len() + 1);
    parts.push(header.as_slice());
    parts.extend_from_slice(&entries);

    ctx.write_frame(&parts).await?;
    task.advance_by(carried);
    outgoing.clear();

    Ok(())
}

/// What is being built to cross as one value: the entries so far, and how much of the direction is
/// behind them.
#[derive(Default)]
pub(crate) struct Outgoing {
    /// The entries this packet holds.
    pub(crate) carried: Vec<Carried>,
    /// How many bytes of entries it holds.
    bytes: usize,
    /// How many units of the direction its sender has carried whole.
    ///
    /// This is what the other side moves its bar by: a bar drawn in units cannot be moved by entries,
    /// since one unit of a cut content is many entries, and only the side that expanded it knows how
    /// many.
    pub(crate) completed: u32,
}

impl Outgoing {
    /// Whether this packet has room for one more entry of `bytes`.
    ///
    /// A packet already holding nothing has room for anything, however large: an entry bigger than
    /// one packet is still an entry, and it crosses as a packet of its own rather than not at all.
    const fn room_for(&self, bytes: usize) -> bool {
        !self.carried.is_empty()
            && self.carried.len() < PACKET_ENTRIES
            && self
                .bytes
                .saturating_add(ENTRY_HEADER)
                .saturating_add(bytes)
                <= PACKET_BYTES
    }

    /// Adds one entry, whether or not there was room for it.
    fn add(&mut self, entry: Carried) {
        self.bytes = self.bytes.saturating_add(entry.bytes.len());
        self.carried.push(entry);
    }

    /// The header this packet crosses under, and the parts of its entries in order.
    ///
    /// Only the sending side builds a packet, so what this hands out borrows what the packet already
    /// holds: nothing is built to hold the packet whole on its way to a frame.
    pub(crate) fn parts(&self) -> ([u8; PACKET_HEADER], Vec<&[u8]>) {
        let mut entries = Vec::with_capacity(self.carried.len() * 2);
        for entry in &self.carried {
            entries.extend_from_slice(&entry.parts());
        }

        (
            packet_header(
                self.completed,
                u32::try_from(self.carried.len()).unwrap_or(u32::MAX),
            ),
            entries,
        )
    }

    /// The header of the packet that says a direction is over, with `finished` units behind it.
    pub(crate) const fn last_header(finished: u32) -> [u8; PACKET_HEADER] {
        packet_header(finished, 0)
    }

    /// Starts another packet where this one was, once what it held has gone.
    fn clear(&mut self) {
        self.carried.clear();
        self.bytes = 0;
    }
}

/// The header a packet crosses under: how many units of the direction are behind it, and how many
/// entries it holds.
///
/// The count is what tells a packet of no entries — the one that says a direction is over — from one
/// nothing was added to, and it is also what a packet says before the bytes a corrupt one would aim
/// at are trusted.
const fn packet_header(finished: u32, count: u32) -> [u8; PACKET_HEADER] {
    let units = finished.to_be_bytes();
    let entries = count.to_be_bytes();
    let mut header = [0_u8; PACKET_HEADER];
    let mut at = 0;
    while at < size_of::<u32>() {
        header[at] = units[at];
        header[at + size_of::<u32>()] = entries[at];
        at += 1;
    }

    header
}

/// What one entry carries, as the byte that leads it says.
///
/// Two questions in one byte: whether the bytes are a manifest naming chunks or content of its own,
/// and whether they are in the packet at all or follow it as a blob. The second is what a content
/// kept whole and bigger than one value needs — see [`push`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lead(u8);

impl Lead {
    /// The byte an entry whose bytes are of this kind has.
    const fn of(manifest: bool, large: bool) -> Self {
        let mut lead = 0_u8;
        if manifest {
            lead |= 1;
        }
        if large {
            lead |= 2;
        }

        Self(lead)
    }

    /// Whether the entry is a manifest naming chunks rather than content of its own.
    pub(crate) const fn manifest(self) -> bool {
        self.0 & 1 == 1
    }

    /// Whether the entry's bytes follow it as a blob rather than being in the packet.
    pub(crate) const fn large(self) -> bool {
        self.0 & 2 == 2
    }

    /// The lead a byte stands for, if this build knows it.
    const fn read(byte: u8) -> Option<Self> {
        if byte <= 3 { Some(Self(byte)) } else { None }
    }
}

/// One thing a packet carries: a key, what kind of bytes they are, and the bytes themselves.
///
/// This is the side that has the bytes, so it owns them: what a packet hands to a frame is a slice of
/// what was read, and the header that goes with it is built here rather than when the packet is sent.
pub(crate) struct Carried {
    /// The entry's header as it goes on the wire: its kind, its key and the length of its bytes.
    ///
    /// The key and the kind are in here rather than beside it: a packet is what a frame is made of,
    /// and a frame of the parts a packet holds is one the packet does not have to be copied into.
    header: [u8; ENTRY_HEADER],
    /// The bytes themselves: a manifest's encoding, or an object's content — and nothing at all for
    /// an entry whose bytes are too many for one value and follow it as a blob.
    pub(crate) bytes: Vec<u8>,
}

impl Carried {
    /// One entry of a packet being built.
    pub(crate) fn new(key: Key, lead: Lead, bytes: Vec<u8>) -> Self {
        let mut header = [0_u8; ENTRY_HEADER];
        header[0] = lead.0;
        header[1..=BLAKE3_HASH_LEN].copy_from_slice(key.digest());
        // A packet never holds more than one frame's worth of entries, so an entry's length cannot
        // reach what a `u32` will not hold.
        header[1 + BLAKE3_HASH_LEN..]
            .copy_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_be_bytes());

        Self { header, bytes }
    }

    /// The two runs of bytes this entry is on the wire: its header, and its bytes.
    fn parts(&self) -> [&[u8]; 2] {
        [&self.header, &self.bytes]
    }
}

/// One thing a packet carried, read back out of the bytes it arrived in.
///
/// The bytes are borrowed rather than copied: what the side taking it does with them is write them
/// into its own store, which is the one copy they need, and a packet of a thousand entries was a
/// thousand copies for nothing.
pub(crate) struct Received<'a> {
    /// The key the bytes belong to.
    pub(crate) key: Key,
    /// Whether what arrived is a manifest or content, and whether it was in the packet.
    pub(crate) lead: Lead,
    /// The bytes themselves, borrowed from the frame they arrived in.
    pub(crate) bytes: &'a [u8],
}

/// What arrived in one value: as many entries as one packet held.
///
/// The shape is written down here rather than derived, because what it is for is the number of
/// entries: a format that states a length per entry in a fixed number of bytes is a few bytes per
/// entry, and one that repeats itself in every entry is not. What a packet holds is read from the
/// packet itself, so the two ends do not have to agree on how it was cut — only on the order, which
/// both read from the same two presence answers.
pub(crate) struct Packet<'a> {
    /// How many units of the direction its sender had carried whole when it filled this packet.
    pub(crate) finished: u32,
    /// The entries, in the order the store reads them.
    pub(crate) carried: Vec<Received<'a>>,
}

impl<'a> Packet<'a> {
    /// How many bytes of entries it holds.
    pub(crate) fn bytes(&self) -> u64 {
        self.carried.iter().fold(0_u64, |total, entry| {
            total.saturating_add(u64::try_from(entry.bytes.len()).unwrap_or(u64::MAX))
        })
    }

    /// Reads a packet out of the bytes it crossed in, borrowing its entries' bytes.
    ///
    /// `None` is bytes that are not a packet this build reads: a count the bytes left could not hold,
    /// a frame cut short, or bytes left over at the end. Nothing is reserved from a count the bytes do
    /// not back — the frame the bytes came in is what bounds them, and a count is what a corrupt
    /// packet would aim at.
    pub(crate) fn decode(bytes: &'a [u8]) -> Option<Self> {
        let (finished, rest) = split_u32(bytes)?;
        let (count, mut rest) = split_u32(rest)?;
        let count = usize::try_from(count).ok()?;

        if count > rest.len() / ENTRY_HEADER {
            return None;
        }

        let mut carried = Vec::with_capacity(count);
        for _ in 0..count {
            let (lead, after) = rest.split_first()?;
            let (digest, after) = after.split_at_checked(BLAKE3_HASH_LEN)?;
            let (len, after) = split_u32(after)?;
            let len = usize::try_from(len).ok()?;
            let (entry, after) = after.split_at_checked(len)?;

            // A digest is what a key is, and a byte that leads no kind of entry this build writes is
            // not one a packet it wrote could hold.
            let digest: Blake3Hash = digest.try_into().ok()?;
            let lead = Lead::read(*lead)?;

            carried.push(Received {
                key: Key::new(digest),
                lead,
                bytes: entry,
            });
            rest = after;
        }

        if !rest.is_empty() {
            return None;
        }

        Some(Self { finished, carried })
    }
}

/// Reads the first four bytes of `bytes` as a big-endian number, and the rest after them.
fn split_u32(bytes: &[u8]) -> Option<(u32, &[u8])> {
    let (head, rest) = bytes.split_at_checked(size_of::<u32>())?;
    let head: [u8; size_of::<u32>()] = head.try_into().ok()?;

    Some((u32::from_be_bytes(head), rest))
}

/// Carries one run of bytes in the direction `to` names, and answers what arrived here.
///
/// `bytes` is what *this* side holds: [`Some`] on the side that has it and [`None`] on the side taking
/// it, which is [`carry_blob`]'s convention and for the same reason — both ends call this at the same
/// place and each decides from what it holds whether it is sending or reading.
///
/// `to` is not on the wire and does not pick the direction: the `Option` does. It is here to check
/// that the two agree. [`carry_value`] cannot get this wrong, because a value that is not this side's
/// is an empty wrapper and the wrapper says so; a run of bytes has no wrapper to say it, so a side
/// that passed the wrong thing would write where it should read and both ends would wait for good. A
/// disagreement is [`ActionError::MissingValue`] instead.
///
/// What crosses is one frame: a length and then the bytes. Nothing about them is encoded, which is
/// what a run of bytes is for.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if the side holding the bytes is not the side that sends to
/// `to`, and what the exchange fails with — see [`ActionError`].
pub(crate) async fn carry_frame(
    ctx: &mut ActionContext<'_>,
    to: Side,
    bytes: Option<&[u8]>,
) -> Result<Option<Vec<u8>>, ActionError> {
    if bytes.is_some() != sending(ctx, to) {
        return Err(ActionError::MissingValue);
    }

    match bytes {
        Some(bytes) => {
            ctx.write_frame(&[bytes]).await?;

            Ok(None)
        }
        None => Ok(Some(ctx.read_frame().await?)),
    }
}

/// Carries one blob in the direction `to` names, and answers what arrived here.
///
/// `blob` is what *this* side holds, and it is read only where this side is the one that has it: the
/// side taking a blob runs nothing of the other side's, so what it passes is never looked at. What
/// comes back is the blob when it arrived here and nothing when it left, so the same call reads the
/// same way on both ends.
///
/// A blob crosses as its length and then its bytes, in pieces of no more than [`PIECE`] — which is
/// what lets a content bigger than a frame cross at all, since no one value is ever longer than that.
/// A blob of no bytes is still a blob: what the length says is how many bytes come, so an empty
/// content crosses as an empty content rather than as nothing at all.
pub(crate) async fn carry_blob(
    ctx: &mut ActionContext<'_>,
    to: Side,
    blob: Option<Vec<u8>>,
) -> Result<Option<Vec<u8>>, ActionError> {
    carry_blob_moving(ctx, to, blob, None).await
}

/// Carries one blob as [`carry_blob`] does, and moves `task` along as each piece of it crosses.
///
/// A blob longer than one frame crosses as several pieces, and a reader watching one long content
/// wants to see it move while it moves: with a task, every piece moves the bar by as many bytes as
/// it holds. That number is the same on both ends — the side sending knows it before the piece
/// goes, and the side taking learns it as the piece arrives — so both ends move the same bar by the
/// same amount and neither runs ahead of the other.
pub(crate) async fn carry_blob_moving(
    ctx: &mut ActionContext<'_>,
    to: Side,
    blob: Option<Vec<u8>>,
    mut task: Option<&mut Task>,
) -> Result<Option<Vec<u8>>, ActionError> {
    let sending = blob.is_some();

    // How many bytes to expect comes first, and the pieces that follow are counted from it, so the
    // two ends agree on how many values cross without either being told a second time. The number is
    // eight bytes big-endian rather than a serde `usize`: what crosses here is a number and a run of
    // bytes, and the width a build's `usize` happens to have is not part of either.
    let announced = blob.as_ref().map_or(0, Vec::len);
    let said = blob.as_ref().map(|_| {
        u64::try_from(announced)
            .unwrap_or(u64::MAX)
            .to_be_bytes()
            .to_vec()
    });
    let header = carry_frame(ctx, to, said.as_deref()).await?;
    let announced = match header {
        Some(said) => {
            let said = said
                .as_chunks::<{ size_of::<u64>() }>()
                .0
                .first()
                .ok_or(ActionError::MissingValue)?;

            usize::try_from(u64::from_be_bytes(*said)).unwrap_or(usize::MAX)
        }
        None => announced,
    };

    // What the peer says it is sending is what is reserved for, not what is trusted: the pieces
    // themselves are what the bytes are put together from.
    let mut arrived = Vec::with_capacity(announced.min(PIECE));
    for at in 0..announced.div_ceil(PIECE) {
        let start = at * PIECE;
        // Handed out as what it is rather than copied into a buffer of its own: the piece is a run of
        // the blob this side already holds, and the frame under this writes it where it lies.
        let outgoing = blob
            .as_ref()
            .map(|bytes| &bytes[start..bytes.len().min(start + PIECE)]);

        // What this piece is long: the side sending cut it, so it knows; the side taking is handed
        // what the other side cut, so it learns it here. The two are the same number of bytes.
        let mut crossed = outgoing.map_or(0, <[u8]>::len);
        if let Some(piece) = carry_frame(ctx, to, outgoing).await? {
            crossed = piece.len();
            arrived.extend_from_slice(&piece);
        }

        if let Some(task) = task.as_deref_mut() {
            task.advance_by(u64::try_from(crossed).unwrap_or(u64::MAX));
        }
    }

    // The side that sent it has it already; the side taking reads what came.
    Ok((!sending).then_some(arrived))
}

/// Carries one value in the direction `to` names, and answers what arrived here.
///
/// `value` is the value *this* side holds, and it is read only where this side is the one that
/// holds it: the side taking a value runs nothing of the other side's, so what it passes is never
/// looked at. What comes back is the value when it arrived here and nothing when it left, so the
/// same call reads the same way on both ends.
pub(crate) async fn carry_value<T>(
    ctx: &mut ActionContext<'_>,
    to: Side,
    value: T,
) -> Result<Option<T>, ActionError>
where
    T: rorolala_protocol::Encodable + Send + Sync + 'static,
{
    match to {
        Side::Vault => {
            let handed: OnlyWorkspace<T> = ctx.only_workspace(|| value);
            let arrived: OnlyVault<T> = ctx.transfer(handed).await?;

            Ok(arrived.into_inner())
        }
        Side::Workspace => {
            let handed: OnlyVault<T> = ctx.only_vault(|| value);
            let arrived: OnlyWorkspace<T> = ctx.transfer(handed).await?;

            Ok(arrived.into_inner())
        }
    }
}

/// The point both ends reach once they have carried everything they had to.
///
/// A value crosses as frames that are written and read and never acknowledged, so a side that has
/// sent everything knows only that it wrote it, not that the other side took it. Both ends call
/// this after carrying, and it is the Vault's value the Workspace waits for: the Vault sends it
/// where the Workspace receives it, so a Workspace that returns has been told the Vault got here — /// and the Vault gets here only once it has published what it was sent. What crosses is
/// nothing; where it is exchanged is the whole of it.
///
/// # Errors
///
/// Returns whatever the exchange fails with — see [`ActionError`].
pub(crate) async fn reach_the_other_end(ctx: &mut ActionContext<'_>) -> Result<(), ActionError> {
    carry_value(ctx, Side::Workspace, 0_u64).await.map(drop)
}

/// What a store failure means to an action.
fn store_failed(error: StoreError) -> ActionError {
    match error {
        // A store that could not be read or written is a file, not a channel: what a caller does
        // about it is give access to a place, and the two are told apart because they are acted on
        // differently.
        StoreError::Io(source) => ActionError::LocalFile(source.into()),
        // What this side held a moment ago is not there now, which is the same answer as the check
        // having found neither side holding it.
        StoreError::NotFound(_) => ActionError::MissingObject,
        error => ActionError::Store(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use rorolala_storage::Key;

    use super::{Unit, UnitKind, holds};

    /// A key that is only itself, so a test says which one it means.
    fn key(seed: u8) -> Key {
        Key::new([seed; 32])
    }

    /// A unit naming `kind` of `key`.
    fn unit(key: Key, kind: UnitKind) -> Unit {
        Unit { key, kind }
    }

    #[test]
    fn a_manifest_unit_is_held_when_its_manifest_arrived() {
        let named = key(1);
        let objects = BTreeSet::new();
        let mut manifests = BTreeMap::new();

        assert!(!holds(
            &unit(named, UnitKind::Manifest),
            &objects,
            &manifests
        ));

        manifests.insert(named, Vec::new());
        assert!(holds(
            &unit(named, UnitKind::Manifest),
            &objects,
            &manifests
        ));
    }

    #[test]
    fn an_object_unit_is_held_when_the_object_arrived() {
        let named = key(2);
        let manifests = BTreeMap::new();
        let mut objects = BTreeSet::new();

        assert!(!holds(&unit(named, UnitKind::Object), &objects, &manifests));

        objects.insert(named);
        assert!(holds(&unit(named, UnitKind::Object), &objects, &manifests));
    }

    #[test]
    fn a_whole_unit_is_held_when_it_arrived_whole() {
        let named = key(3);
        // An object kept whole: the content is the object, and there is no manifest.
        let mut objects = BTreeSet::new();
        let manifests = BTreeMap::new();

        assert!(!holds(&unit(named, UnitKind::Whole), &objects, &manifests));

        objects.insert(named);
        assert!(holds(&unit(named, UnitKind::Whole), &objects, &manifests));
    }

    #[test]
    fn a_whole_unit_is_held_when_its_chunks_all_arrived() {
        let named = key(4);
        let first = key(5);
        let second = key(6);

        // A content kept as chunks: the manifest and the two chunks it names are three entries.
        let mut objects = BTreeSet::new();
        let mut manifests = BTreeMap::new();
        manifests.insert(named, vec![first, second]);

        assert!(!holds(&unit(named, UnitKind::Whole), &objects, &manifests));

        objects.insert(first);
        assert!(!holds(&unit(named, UnitKind::Whole), &objects, &manifests));

        objects.insert(second);
        assert!(holds(&unit(named, UnitKind::Whole), &objects, &manifests));
    }

    #[test]
    fn a_discover_unit_is_held_whichever_way_the_content_came() {
        let named = key(7);
        let chunk = key(8);

        // The caller did not know how the content is kept, so either shape answers for it.
        let mut objects = BTreeSet::new();
        let manifests = BTreeMap::new();
        assert!(!holds(
            &unit(named, UnitKind::Discover),
            &objects,
            &manifests
        ));

        objects.insert(named);
        assert!(holds(
            &unit(named, UnitKind::Discover),
            &objects,
            &manifests
        ));

        let mut objects = BTreeSet::new();
        let mut manifests = BTreeMap::new();
        manifests.insert(named, vec![chunk]);
        assert!(!holds(
            &unit(named, UnitKind::Discover),
            &objects,
            &manifests
        ));

        objects.insert(chunk);
        assert!(holds(
            &unit(named, UnitKind::Discover),
            &objects,
            &manifests
        ));
    }
}

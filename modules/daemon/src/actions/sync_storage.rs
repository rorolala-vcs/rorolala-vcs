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
//! A run of this says what it is doing as it goes, and says it only once the two answers have
//! crossed: until then how much has to move is not known, so a bar drawn from it would be a
//! guess. Once it is known, the whole is one task and each direction under it is another, and
//! the key being carried is named inside the direction it is going in — which is what a reader
//! needs to tell a run that is moving a little from one that is moving a lot.

use rorolala_errors::BincodeError;
use rorolala_protocol::{ActionContext, ActionError, Both, MAX_FRAME, OnlyVault, OnlyWorkspace};
use rorolala_storage::{
    Error as StoreError, Key, Manifest, Presence, RorolalaStorage, StorageBackend as _,
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

/// The byte that leads a discovery value which is a manifest.
const MANIFEST_LEAD: u8 = 1;

/// The byte that leads a discovery value which is an object.
const OBJECT_LEAD: u8 = 0;

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
/// side, [`ActionError::Codec`] if a manifest from the peer does not read, and
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

    sync_units(ctx, workspace, vault, Both::new(units)).await
}

/// Makes two stores hold the same units.
///
/// This is the whole of [`sync_storage`] with the expansion left to the caller: a required key is a
/// unit naming the whole of what it holds, while a manifest and its chunks are units of their own,
/// which is what lets the chunks of one content travel over as many lanes as the caller arranges.
/// The presence each side reports is per unit, so a unit already held is passed over whichever way
/// the content under its key is kept.
///
/// # Errors
///
/// Returns [`ActionError::NoChannel`] if there is nothing to exchange over,
/// [`ActionError::MissingValue`] if the context did not say which store this side has or the
/// answers did not come back, [`ActionError::MissingObject`] if a required unit is held by neither
/// side, [`ActionError::Codec`] if a manifest from the peer does not read, and
/// [`ActionError::LocalFile`] if a store could not be read or written, and
/// [`ActionError::Store`] for what a store failed with otherwise.
pub(crate) async fn sync_units(
    ctx: &mut ActionContext<'_>,
    workspace: OnlyWorkspace<RorolalaStorage>,
    vault: OnlyVault<RorolalaStorage>,
    required: Both<Vec<Unit>>,
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
        // Only the side sending a direction knows how long it is: it measures the keys it is about
        // to carry and says the total, so both ends draw the same bar.
        let announced = if ctx.is_workspace() {
            sending_len(&local, &up).await?
        } else {
            0
        };
        let total = carry_value(ctx, Side::Vault, announced)
            .await?
            .unwrap_or(announced);

        let mut task = everything.spawn(Direction::Up, "", Some(total));

        for unit in &up {
            let working = task.doing(unit.key.hex());
            carry_unit(ctx, &local, **unit, Side::Vault, &mut task).await?;
            drop(working);

            everything.advance_by(1);
        }
    }

    if !down.is_empty() {
        // As above, told the other way round: the Vault is the side sending this direction.
        let announced = if ctx.is_vault() {
            sending_len(&local, &down).await?
        } else {
            0
        };
        let total = carry_value(ctx, Side::Workspace, announced)
            .await?
            .unwrap_or(announced);

        let mut task = everything.spawn(Direction::Down, "", Some(total));

        for unit in &down {
            let working = task.doing(unit.key.hex());
            carry_unit(ctx, &local, **unit, Side::Workspace, &mut task).await?;
            drop(working);

            everything.advance_by(1);
        }
    }

    // Both ends have carried everything they had to, and this is the point they reach once they
    // have. It is what a run waits on: a value crosses as frames that are written and read and
    // never acknowledged, so a Workspace that has sent everything would otherwise return while the
    // Vault is still writing what it was sent — a manifest that reads and chunks that are not here
    // yet, which is a content not held. Waiting for the Vault to say it got here leaves the sync
    // only once both ends have nothing left to do.
    reach_the_other_end(ctx).await?;

    Ok(())
}

/// How many bytes the keys in `keys` are carried as, summed over all of them.
///
/// This is what a direction says it is about to move: the side that holds the keys can say how long
/// they are without reading them — see [`RorolalaStorage::carried_len`] — and the side taking them
/// cannot, so the length crosses rather than being worked out twice.
///
/// # Errors
///
/// Returns [`ActionError::Store`] or [`ActionError::MissingObject`] if a key that is about to cross
/// can no longer be measured.
async fn sending_len(local: &RorolalaStorage, units: &[&Unit]) -> Result<u64, ActionError> {
    let mut total = 0_u64;

    for unit in units {
        total = total.saturating_add(unit_len(local, unit).await?);
    }

    Ok(total)
}

/// How many bytes `unit` is carried as.
///
/// A manifest is measured by reading it, which is cheap because a manifest is small; a whole or an
/// object is measured without reading, the way [`sync_storage`] has always measured a key.
async fn unit_len(local: &RorolalaStorage, unit: &Unit) -> Result<u64, ActionError> {
    match unit.kind {
        UnitKind::Manifest => Ok(local
            .manifest_of(&unit.key)
            .await
            .map_err(store_failed)?
            .map_or(0, |manifest| manifest.encode().len() as u64)),
        UnitKind::Whole | UnitKind::Object | UnitKind::Discover => {
            local.carried_len(&unit.key).await.map_err(store_failed)
        }
    }
}

/// Which of `units` this side holds: a manifest unit by its manifest, anything else by its content.
async fn presence(local: &RorolalaStorage, units: &[Unit]) -> Result<Presence, ActionError> {
    let mut presence = Presence::all_missing(units.len());

    for (at, unit) in units.iter().enumerate() {
        let held = match unit.kind {
            UnitKind::Manifest => local
                .holds_manifest(&unit.key)
                .await
                .map_err(store_failed)?,
            UnitKind::Whole | UnitKind::Object | UnitKind::Discover => local
                .contains_keys(std::slice::from_ref(&unit.key))
                .await
                .map_err(store_failed)?
                .held(0),
        };

        if held {
            presence.set_held(at, true);
        }
    }

    Ok(presence)
}

/// Carries the whole of `key` to `to`, where it is written down.
///
/// `local` is the store on this side: what it holds is read here when this is the side sending,
/// and what arrives is written into it when this is the side taking. Which of the two it is is
/// `to`'s to say — the side a value goes to is the side that does not have it.
///
/// What crosses for one key is its manifest, when it has one, and then each chunk the manifest
/// names; a key that is one object has no manifest, and what crosses is the object. What is sent
/// is never what is taken, so the two ends know which of the two they are from where the key is
/// coming from.
///
/// `task` counts the bytes as they cross: a blob is as long as it was, whether this side wrote it
/// or took it, so both ends move the same bar by the same amount and a content of many chunks is
/// seen to move a chunk at a time rather than all at once at the end.
async fn carry(
    ctx: &mut ActionContext<'_>,
    local: &RorolalaStorage,
    key: &Key,
    to: Side,
    task: &mut Task,
) -> Result<(), ActionError> {
    // The side a key is going to is the side that does not have it, so the side sending is the
    // other one.
    let sending = match to {
        Side::Vault => ctx.is_workspace(),
        Side::Workspace => ctx.is_vault(),
    };

    let pieces = if sending {
        Some(pieces(local, key).await?)
    } else {
        None
    };

    // What comes first says what follows: the manifest when the content is kept as chunks, and
    // nothing when it is one object. An empty manifest is a content that was not cut, since a cut
    // content is one manifest naming the chunks it was cut into and there are never none of them.
    let manifest = pieces
        .as_ref()
        .and_then(Pieces::manifest_bytes)
        .unwrap_or_default();
    let outgoing = if sending {
        Some(manifest.clone())
    } else {
        None
    };
    let arrived = carry_blob_moving(ctx, to, outgoing, Some(&mut *task)).await?;
    let manifest = arrived.unwrap_or(manifest);

    if manifest.is_empty() {
        // One object, and it follows.
        let whole = pieces
            .as_ref()
            .and_then(Pieces::whole)
            .cloned()
            .unwrap_or_default();

        let arrived =
            carry_blob_moving(ctx, to, sending.then_some(whole), Some(&mut *task)).await?;

        if let Some(bytes) = arrived {
            local
                .write_object(&bytes, local.codec())
                .await
                .map_err(store_failed)?;
        }

        return Ok(());
    }

    let manifest = Manifest::decode(&manifest)
        .map_err(|error| ActionError::Codec(BincodeError::new(error.to_string())))?;

    // The manifest is written before the chunks arrive: a manifest whose chunks are not here yet is
    // a content that is not held, which is what the chunks that follow put right.
    if !sending {
        local
            .put_manifest(key, &manifest)
            .await
            .map_err(store_failed)?;
    }

    let chunks = pieces.as_ref().and_then(Pieces::chunks);
    for at in 0..manifest.len() {
        // As above: what one side sends, the other passes nothing for.
        let outgoing = chunks.and_then(|chunks| chunks.get(at).cloned());

        let arrived = carry_blob_moving(ctx, to, outgoing, Some(&mut *task)).await?;

        if let Some(bytes) = arrived {
            local
                .write_object(&bytes, local.codec())
                .await
                .map_err(store_failed)?;
        }
    }

    Ok(())
}

/// Carries one unit to `to`, where it is written down.
///
/// A unit naming the whole is carried as [`carry`] carries a key, expanded here; a manifest unit
/// carries only the manifest, which the taker records, and an object unit only the object, which the
/// taker writes. What crosses is never what is taken, so the two ends know which they are from where
/// the unit is coming from.
async fn carry_unit(
    ctx: &mut ActionContext<'_>,
    local: &RorolalaStorage,
    unit: Unit,
    to: Side,
    task: &mut Task,
) -> Result<(), ActionError> {
    let sending = match to {
        Side::Vault => ctx.is_workspace(),
        Side::Workspace => ctx.is_vault(),
    };

    match unit.kind {
        UnitKind::Whole => carry(ctx, local, &unit.key, to, task).await,
        UnitKind::Manifest => {
            let manifest = if sending {
                local.manifest_of(&unit.key).await.map_err(store_failed)?
            } else {
                None
            };
            let outgoing = manifest.as_ref().map(Manifest::encode);
            let arrived = carry_blob_moving(ctx, to, outgoing, Some(&mut *task)).await?;

            if let Some(bytes) = arrived {
                let manifest = Manifest::decode(&bytes)
                    .map_err(|error| ActionError::Codec(BincodeError::new(error.to_string())))?;
                local
                    .put_manifest(&unit.key, &manifest)
                    .await
                    .map_err(store_failed)?;
            }

            Ok(())
        }
        UnitKind::Object => {
            let object = if sending {
                Some(local.read_object(&unit.key).await.map_err(store_failed)?)
            } else {
                None
            };
            let arrived = carry_blob_moving(ctx, to, object, Some(&mut *task)).await?;

            if let Some(bytes) = arrived {
                local
                    .write_object(&bytes, local.codec())
                    .await
                    .map_err(store_failed)?;
            }

            Ok(())
        }
        UnitKind::Discover => {
            // What is sent is decided by what this side holds, and what arrives says which it was:
            // a manifest is recorded as one, an object written as one, and the two are told apart by
            // the byte that leads the value rather than by trying to read the bytes as either.
            let (lead, bytes) = if sending {
                match local.manifest_of(&unit.key).await.map_err(store_failed)? {
                    Some(manifest) => (MANIFEST_LEAD, manifest.encode()),
                    None => (
                        OBJECT_LEAD,
                        local.read_object(&unit.key).await.map_err(store_failed)?,
                    ),
                }
            } else {
                (OBJECT_LEAD, Vec::new())
            };

            let lead = carry_value(ctx, to, lead).await?.unwrap_or(lead);
            let arrived =
                carry_blob_moving(ctx, to, sending.then_some(bytes), Some(&mut *task)).await?;

            if let Some(bytes) = arrived {
                if lead == MANIFEST_LEAD {
                    let manifest = Manifest::decode(&bytes).map_err(|error| {
                        ActionError::Codec(BincodeError::new(error.to_string()))
                    })?;
                    local
                        .put_manifest(&unit.key, &manifest)
                        .await
                        .map_err(store_failed)?;
                } else {
                    local
                        .write_object(&bytes, local.codec())
                        .await
                        .map_err(store_failed)?;
                }
            }

            Ok(())
        }
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
async fn carry_blob_moving(
    ctx: &mut ActionContext<'_>,
    to: Side,
    blob: Option<Vec<u8>>,
    mut task: Option<&mut Task>,
) -> Result<Option<Vec<u8>>, ActionError> {
    let sending = blob.is_some();

    // How many bytes to expect comes first, and the pieces that follow are counted from it, so the
    // two ends agree on how many values cross without either being told a second time.
    let announced = blob.as_ref().map_or(0, Vec::len);
    let header = carry_value(ctx, to, announced).await?;
    let announced = header.unwrap_or(announced);

    // What the peer says it is sending is what is reserved for, not what is trusted: the pieces
    // themselves are what the bytes are put together from.
    let mut arrived = Vec::with_capacity(announced.min(PIECE));
    for at in 0..announced.div_ceil(PIECE) {
        let start = at * PIECE;
        let outgoing = blob
            .as_ref()
            .map(|bytes| bytes[start..bytes.len().min(start + PIECE)].to_vec());

        // What this piece is long: the side sending cut it, so it knows; the side taking is handed
        // what the other side cut, so it learns it here. The two are the same number of bytes.
        let mut crossed = outgoing.as_ref().map_or(0, Vec::len);
        if let Some(piece) = carry_value(ctx, to, outgoing.unwrap_or_default()).await? {
            crossed = piece.len();
            arrived.extend_from_slice(&piece);
        }

        if let Some(task) = task.as_deref_mut() {
            task.advance_by(crossed as u64);
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
async fn carry_value<T>(
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
/// where the Workspace receives it, so a Workspace that returns has been told the Vault got here —
/// and the Vault gets here only once it has written everything it was sent. What crosses is
/// nothing; where it is exchanged is the whole of it.
///
/// # Errors
///
/// Returns whatever the exchange fails with — see [`ActionError`].
pub(crate) async fn reach_the_other_end(ctx: &mut ActionContext<'_>) -> Result<(), ActionError> {
    carry_value(ctx, Side::Workspace, 0_u64).await.map(drop)
}

/// What one key is made of, as the store on this side keeps it.
enum Pieces {
    /// The content is kept as one object, which is the whole of it.
    Whole(Vec<u8>),
    /// The content is kept as chunks, and this is the manifest that names them.
    Cut {
        /// How the content is put back together.
        manifest: Manifest,
        /// The chunks the manifest names, in the order it names them.
        chunks: Vec<Vec<u8>>,
    },
}

impl Pieces {
    /// The manifest as it crosses, or nothing when the content is one object.
    fn manifest_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Self::Whole(_) => None,
            Self::Cut { manifest, .. } => Some(manifest.encode()),
        }
    }

    /// The chunks, in the order the manifest names them.
    fn chunks(&self) -> Option<&[Vec<u8>]> {
        match self {
            Self::Whole(_) => None,
            Self::Cut { chunks, .. } => Some(chunks),
        }
    }

    /// The object, when the content is one.
    const fn whole(&self) -> Option<&Vec<u8>> {
        match self {
            Self::Whole(bytes) => Some(bytes),
            Self::Cut { .. } => None,
        }
    }
}

/// Reads what `local` holds under `key`: its manifest and the chunks it names, or the object.
async fn pieces(local: &RorolalaStorage, key: &Key) -> Result<Pieces, ActionError> {
    let Some(manifest) = local.manifest_of(key).await.map_err(store_failed)? else {
        return Ok(Pieces::Whole(
            local.read_object(key).await.map_err(store_failed)?,
        ));
    };

    let mut chunks = Vec::with_capacity(manifest.len());
    for chunk in manifest.chunks() {
        chunks.push(
            local
                .read_object(&chunk.key())
                .await
                .map_err(store_failed)?,
        );
    }

    Ok(Pieces::Cut { manifest, chunks })
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

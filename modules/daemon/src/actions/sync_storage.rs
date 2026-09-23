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
use rorolala_utils_progress::Direction;

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
/// [`ActionError::Store`] or [`ActionError::Io`] if a store could not be read or written.
pub async fn sync_storage(
    ctx: &mut ActionContext<'_>,
    workspace: OnlyWorkspace<RorolalaStorage>,
    vault: OnlyVault<RorolalaStorage>,
    required_keys: Both<Vec<Key>>,
) -> Result<(), ActionError> {
    // Whichever side this is, it named the store it has; the other wrapper is empty.
    let local = workspace
        .into_inner()
        .or_else(|| vault.into_inner())
        .ok_or(ActionError::MissingValue)?;
    let keys = required_keys.into_inner();

    // What this side holds of what was asked for, and then what the other side holds: the two are
    // exchanged before anything moves, so both ends agree on which keys cross and which way.
    let mine = local.contains_keys(&keys).await.map_err(store_failed)?;

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

    // What has to move is read from the two answers, so a run says how much of it there is
    // before it starts rather than as it finds out. Nothing has moved yet, so this is the first
    // thing said, and a reader watching from the start sees a bar that is empty and honest
    // rather than one that fills up to a length it did not know it had.
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
            carry(ctx, &local, key, Side::Vault).await?;
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
            carry(ctx, &local, key, Side::Workspace).await?;
            drop(working);

            task.advance_by(1);
            everything.advance_by(1);
        }
    }

    Ok(())
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
async fn carry(
    ctx: &mut ActionContext<'_>,
    local: &RorolalaStorage,
    key: &Key,
    to: Side,
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
    let manifest = carry_blob(ctx, to, outgoing).await?.unwrap_or(manifest);

    if manifest.is_empty() {
        // One object, and it follows.
        let whole = pieces
            .as_ref()
            .and_then(Pieces::whole)
            .cloned()
            .unwrap_or_default();

        if let Some(bytes) = carry_blob(ctx, to, sending.then_some(whole)).await? {
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

        if let Some(bytes) = carry_blob(ctx, to, outgoing).await? {
            local
                .write_object(&bytes, local.codec())
                .await
                .map_err(store_failed)?;
        }
    }

    Ok(())
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

        if let Some(piece) = carry_value(ctx, to, outgoing.unwrap_or_default()).await? {
            arrived.extend_from_slice(&piece);
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
        StoreError::Io(source) => ActionError::Io(source.into()),
        // What this side held a moment ago is not there now, which is the same answer as the check
        // having found neither side holding it.
        StoreError::NotFound(_) => ActionError::MissingObject,
        error => ActionError::Store(error.to_string()),
    }
}

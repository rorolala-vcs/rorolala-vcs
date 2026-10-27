//! The Layout a Vault-side action works on, and what became of an ownership change.
//!
//! Ownership is not a thing of its own: it is the `owner` a Layout keeps for one entry, and it
//! changes the way everything else about an entry changes — through the Layout's own log. So the
//! actions here reach the Vault's Layout through this module rather than opening one each, and
//! what they answer is one [`Ownership`], so the two ends of an exchange read the same three
//! outcomes without either being told a second time.

use std::path::PathBuf;
use std::str::FromStr as _;

use rorolala_errors::{BincodeError, IoError};
use rorolala_layout::{Batch, Layout, LayoutError, LayoutPath, MutableData};
use rorolala_protocol::{ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_storage::Key;
use rorolala_utils_constants::VAULT_LAYOUT_DIR;
use rorolala_utils_location::Locate as _;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::sync_storage::{Side, carry_blob};

/// The markers a remote path may sit under, at the top of the path.
///
/// A file is not put where it belongs when it is first submitted: it lands under `new` until
/// someone locates it, and moving it under `removed` is how it is deprecated. Both are states
/// rather than places, so `@` is reserved for them and a located path does not start with one —
/// and `@` rather than `#`, which a shell reads as the start of a comment.
const MARKERS: &[&str] = &["new", "removed"];

/// The smallest log, in bytes, that is compacted for its own sake.
///
/// A compaction writes every shard's snapshot down and raises the age beside each log, so what it
/// costs is a flush per shard rather than a flush per change. Reading a log costs far less than
/// writing a snapshot, so a layout whose history is short is left alone: below this a log is many
/// thousands of changes, and replaying them is cheaper than folding them — and a layout that small
/// is one whose open is a few milliseconds either way.
const COMPACT_AT: u64 = 1024 * 1024;

/// How many times its own snapshots a log may hold before it is compacted.
///
/// What an open replays is the log, and what it reads whole is the snapshots, so the two are the
/// same kind of cost once the log is longer: a log shorter than the snapshots beside it adds less to
/// an open than the snapshots do, and one that is longer is most of it. A log no longer than them is
/// where the fold is worth its flush: at twenty thousand entries a snapshot is about a mebibyte, so
/// the log is folded back to that every eighteen thousand changes or so — a compaction of under a
/// tenth of a second, spread over changes that each cost microseconds.
const LOG_PER_SNAPSHOT: u64 = 1;

/// What became of a request to take or let go of an entry.
///
/// It is the answer of a Vault-side change, and the Workspace reads it to know whether to touch
/// the Layout it keeps a copy of: [`Owner`](Self::Owner) is the one outcome that is a change,
/// since it is the only one the Vault made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ownership {
    /// The entry is now held by this account, or by none when it is empty.
    Owner(Option<String>),
    /// The Vault holds no entry by that `Uuid`.
    Missing,
    /// Another account holds it, so nothing was changed.
    HeldBy(String),
}

/// The Vault's own Layout, made if the Vault has not one yet.
///
/// Which Vault that is follows from where this runs: an action served by a Vault works on the
/// Vault's Layout, and a Workspace side has none — so a context that is not a Vault is not one an
/// ownership change can run on.
///
/// A Layout that could not be opened is [`ActionError::LocalFile`] when the operating system
/// refused a file, naming the directory it is kept in: the path is what a reader acts on, and a
/// failure that says "access is denied" and nothing else says nothing they can go and look at.
///
/// This is the one door a Vault-side action opens the Layout by, which is what makes it the place a
/// layout that has grown too long a history is folded back down: what an open replays is paid by
/// every action, and paying it forever is what turns a log kept on purpose into a log nobody can
/// afford.
pub(crate) fn local_layout(ctx: &ActionContext<'_>) -> Result<Layout, ActionError> {
    let vault = ctx
        .current_vault()
        .into_inner()
        .ok_or(ActionError::MissingValue)?;

    let dir: PathBuf = vault
        .get_root()
        .join(VAULT_LAYOUT_DIR)
        .components()
        .collect();

    let layout = vault.layout().map_err(|error| match error {
        LayoutError::Io(source) => ActionError::LocalFile(IoError::new(
            source.kind().into(),
            format!("`{}`: {source}", dir.display()),
        )),
        other => ActionError::Store(format!("`{}`: {other}", dir.display())),
    })?;

    compact_if_stale(&layout);

    Ok(layout)
}

/// Folds a layout's log into its snapshots when the log has grown past what reading it costs.
///
/// A failure here is left to the next action rather than failing this one: a compaction is
/// maintenance the action did not ask for, and what a bad one costs is an open that replays what it
/// could not fold — the change that follows is still the change that was asked for. What it may not
/// do is fail where the action would otherwise succeed, which is why nothing is returned.
fn compact_if_stale(layout: &Layout) {
    let sizes = layout.sizes();
    let stale = sizes.logs > COMPACT_AT.max(sizes.snapshots.saturating_mul(LOG_PER_SNAPSHOT));

    if stale {
        // The layout is held for the whole of this by every shard's own lock, so a concurrent action
        // waiting on a shard waits for that shard's compaction and not for all sixteen of them.
        let _ = layout.compact();
    }
}

/// Runs one ownership change per `Uuid`, all over the one session.
///
/// The whole list crosses at once rather than one `Uuid` per exchange: the Workspace already holds
/// it, and the changes are written by one batch, so the Vault has to make every decision before it
/// writes any of them — a `Uuid` read between two writes would be read from a layout a commit is
/// part of the way through. What comes back is one outcome per `Uuid`, in the order they were named.
///
/// Which change it is is `decide`'s to make, as it was for one entry: it is handed the batch the
/// changes are collected in, so what it reads is the layout as the changes already collected leave
/// it, and what it writes is written once with the rest. The two actions that use this differ in
/// nothing else.
///
/// What the bar is called crosses with the input rather than being written here: the words for it
/// belong to whoever asked the run for them, and this crate holds none of its own.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if the Vault holds no member to act as,
/// [`ActionError::Codec`] if an input, a `Uuid` or an outcome will not cross,
/// [`ActionError::Store`] if the Vault's Layout could not be written, and whatever the exchange
/// fails with.
pub(crate) async fn ownership_batch(
    ctx: &mut ActionContext<'_>,
    input: OnlyWorkspace<String>,
    decide: impl Fn(&mut Batch<'_>, Uuid, &str) -> Result<Ownership, ActionError>,
) -> Result<Vec<Ownership>, ActionError> {
    // What to change crosses as JSON — the name the bar is drawn under, and the `Uuid`s — because a
    // list is not a thing the C ABI can hand over, and JSON is how the other batched action carries
    // its own. The Workspace holds it; the Vault is told the list as one value.
    let (name, ids) = match input.into_inner() {
        Some(json) => serde_json::from_str::<(String, Vec<String>)>(&json)
            .map_err(|error| ActionError::Json(error.into()))?,
        None => (String::new(), Vec::new()),
    };

    // The `Uuid`s cross up whole: both ends come out holding the same list rather than counting it
    // out one entry at a time.
    let announced = ctx
        .is_workspace()
        .then(|| ids.encode())
        .transpose()
        .map_err(codec_failed)?;
    let arrived = carry_blob(ctx, Side::Vault, announced.clone()).await?;
    let ids =
        Vec::<String>::decode(arrived.or(announced).unwrap_or_default()).map_err(codec_failed)?;

    // Which account this runs as is the Vault's to say: the session was established as a member, and
    // a name the caller sent would be one the Vault cannot hold itself to. The Workspace side holds no
    // member at all, which is what the guard leaves alone.
    let me = ctx
        .get_member()
        .into_inner()
        .map(|member| member.name())
        .unwrap_or_default();

    if ctx.is_vault() && me.is_empty() {
        return Err(ActionError::MissingValue);
    }

    let count = u64::try_from(ids.len()).unwrap_or(u64::MAX);
    let mut task = ctx.progress().begin(name, Some(count));

    if ctx.is_vault() {
        let (outcomes, written) = decide_all(&local_layout(ctx)?, &ids, &me, &decide);

        // What came of the write crosses before what came of each entry does: an outcome the Vault
        // could not write down is not an outcome, and the Workspace has to hear that before it
        // changes the copy it keeps. A write that failed says why, so the reason is the same one the
        // caller is told; a write that succeeded says nothing.
        let said = match &written {
            Ok(()) => Vec::new(),
            Err(error) => error.to_string().into_bytes(),
        };
        carry_blob(ctx, Side::Workspace, Some(said)).await?;

        written?;

        let encoded = outcomes.encode().map_err(codec_failed)?;
        carry_blob(ctx, Side::Workspace, Some(encoded)).await?;
        task.advance_by(count);

        Ok(outcomes)
    } else {
        let said = carry_blob(ctx, Side::Workspace, None)
            .await?
            .unwrap_or_default();
        if !said.is_empty() {
            return Err(ActionError::Store(
                String::from_utf8_lossy(&said).into_owned(),
            ));
        }

        let bytes = carry_blob(ctx, Side::Workspace, None)
            .await?
            .unwrap_or_default();
        let outcomes = Vec::<Ownership>::decode(bytes).map_err(codec_failed)?;
        task.advance_by(count);

        Ok(outcomes)
    }
}

/// Runs `decide` for every `Uuid` of `ids` against one batch, and commits it once.
///
/// A `decide` that fails part of the way through is not a reason to lose what it decided before
/// that: each of those is a change the single-entry calls would have written already, and a batch is
/// not a transaction. What it came to is handed back beside the error, so the caller can commit the
/// changes that were made and then say what stopped the rest.
fn decide_all(
    layout: &Layout,
    ids: &[String],
    me: &str,
    decide: &impl Fn(&mut Batch<'_>, Uuid, &str) -> Result<Ownership, ActionError>,
) -> (Vec<Ownership>, Result<(), ActionError>) {
    let mut batch = layout.batch();
    let mut outcomes = Vec::with_capacity(ids.len());
    let mut broken = None;

    for named in ids {
        let Ok(id) = Uuid::parse_str(named.trim()) else {
            broken = Some(codec_failed("the uuid does not read"));
            break;
        };

        match decide(&mut batch, id, me) {
            Ok(outcome) => outcomes.push(outcome),
            Err(error) => {
                broken = Some(error);
                break;
            }
        }
    }

    let written = batch
        .commit()
        .map_err(|error| ActionError::Store(error.to_string()));

    (outcomes, broken.map_or(written, Err))
}

/// Runs `write` against `layout` as one batch, so what it changes is written once per shard.
///
/// It is the shape every Vault-side write takes, one entry or a thousand: a change on its own reads
/// what others have written and flushes a write to the shard it names, and a batch reads that once
/// and flushes once per shard its changes fall in. What a writer may read while it writes is the
/// layout as the changes before it leave it, which is what [`Batch`] answers.
pub(crate) fn batched<Output>(
    layout: &Layout,
    write: impl FnOnce(&mut Batch<'_>) -> Result<Output, ActionError>,
) -> Result<Output, ActionError> {
    let mut batch = layout.batch();
    let output = write(&mut batch)?;
    batch
        .commit()
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(output)
}

/// Names the entry `id` as held by `owner`, refusing when another account holds it.
///
/// What is rewritten is the whole of what the Layout keeps for the entry, since that is what a
/// Layout's log holds: the version and what it says are read back as they were and written
/// unchanged, so naming an owner changes nothing else about the entry.
///
/// What it writes is a change of `batch`, which is committed with whatever else the run collected —
/// see [`ownership_batch`]. The entry it reads is the batch's, so a `Uuid` the run named twice is
/// decided against the first change rather than against what the layout held before it.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the change could not be made.
pub(crate) fn take(batch: &mut Batch<'_>, id: Uuid, owner: &str) -> Result<Ownership, ActionError> {
    let Some(data) = batch.entry(id) else {
        return Ok(Ownership::Missing);
    };

    if let Some(held) = data.owner()
        && held != owner
    {
        return Ok(Ownership::HeldBy(held.to_owned()));
    }

    batch
        .update_entry(id, data.renewed(Some(owner.to_owned()), data.version()))
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(Ownership::Owner(Some(owner.to_owned())))
}

/// Removes the name of whoever holds the entry `id`, refusing when it is another account's.
///
/// Naming no owner for an entry nobody holds is nothing to write, so it is answered as the
/// success it is rather than as a change: a run that lets go of what it no longer holds has
/// done what it asked.
///
/// What it writes is a change of `batch`, as [`take`] does.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the change could not be made.
pub(crate) fn giveup(
    batch: &mut Batch<'_>,
    id: Uuid,
    owner: &str,
) -> Result<Ownership, ActionError> {
    let Some(data) = batch.entry(id) else {
        return Ok(Ownership::Missing);
    };

    match data.owner() {
        Some(held) if held != owner => Ok(Ownership::HeldBy(held.to_owned())),
        None => Ok(Ownership::Owner(None)),
        Some(_) => {
            batch
                .update_entry(id, data.renewed(None, data.version()))
                .map_err(|error| ActionError::Store(error.to_string()))?;

            Ok(Ownership::Owner(None))
        }
    }
}

/// What became of a request to move a path in the Vault's Layout.
///
/// A move is how a file is located and how it is deprecated, so it is the holder's or an
/// administrator's to make: the Workspace reads which of the outcomes came back to know whether
/// the Layout it keeps a copy of is to be changed with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathMove {
    /// The entry is now at the path it was moved to.
    Moved,
    /// The Vault's Layout names nothing at the path that was to move.
    Missing,
    /// The path it was to move to already names something.
    Taken,
    /// The account neither holds the entry nor administrates the Vault.
    Refused,
    /// A path is not one a remote Layout may name.
    Malformed,
}

/// Moves whatever is at `from` to `to`, refusing what the account may not move.
///
/// Which account this runs as is already known — the session says — and whether it administrates
/// the Vault is read from the Vault's own configuration by the caller, so both come in as values
/// and the decision here is only about the layout as `batch` leaves it.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the change could not be made.
pub(crate) fn move_remote(
    batch: &mut Batch<'_>,
    from: &str,
    to: &str,
    me: &str,
    admin: bool,
) -> Result<PathMove, ActionError> {
    let (Ok(from), Ok(to)) = (LayoutPath::new(from), LayoutPath::new(to)) else {
        return Ok(PathMove::Malformed);
    };

    if !is_remote_path(&to) {
        return Ok(PathMove::Malformed);
    }

    let Some(id) = batch.id_of(&from) else {
        return Ok(PathMove::Missing);
    };
    if batch.id_of(&to).is_some() {
        return Ok(PathMove::Taken);
    }

    let held = batch
        .entry(id)
        .and_then(|data| data.owner().map(str::to_owned));
    if !admin && held.as_deref() != Some(me) {
        return Ok(PathMove::Refused);
    }

    batch
        .move_path(&from, &to)
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(PathMove::Moved)
}

/// Whether `path` is one a remote Layout may name.
///
/// A path whose first component is `@` is a marker path: it says what state the file is in rather
/// than where it belongs, so only the markers this design names may follow the `@`, and something
/// has to be under them. A path that starts anywhere else is a located path, and is left alone.
fn is_remote_path(path: &LayoutPath) -> bool {
    let mut components = path.as_str().split('/');

    if components.next() != Some("@") {
        return true;
    }

    components
        .next()
        .is_some_and(|marker| MARKERS.contains(&marker))
        && components.next().is_some()
}

/// What an encodable payload that will not cross means to an action.
pub(crate) fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(BincodeError::new(error.to_string()))
}

/// The prefix a `Uuid` only one side holds is put under when it goes up.
const NEW_PREFIX: &str = "@/new/";

/// What became of a request to write an entry of the Vault's Layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryWrite {
    /// The Vault's Layout now holds what was asked for.
    Written,
    /// The Vault already holds an entry by that `Uuid`.
    Exists,
    /// The Vault already names something at that path.
    Taken,
    /// The Vault holds no entry by that `Uuid`.
    Missing,
    /// The account does not hold the entry and does not administrate the Vault.
    Refused,
    /// A path, a `Uuid`, or a version is not one this can read.
    Malformed,
}

/// Runs one write against the Vault, carrying the input there and the outcome back.
///
/// Which write it is is `decide`'s to make: it is handed the Vault's Layout, what crossed, the
/// name of the account the session was established as, and whether that account administrates the
/// Vault. Neither of the last two is taken from the caller. What it writes it writes through
/// [`batched`], so one write and a run's thousand are the same shape.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if the Vault holds no member to act as,
/// [`ActionError::Codec`] if the input or the outcome will not cross, and whatever the exchange
/// fails with.
pub(crate) async fn remote_write<Input>(
    ctx: &mut ActionContext<'_>,
    input: OnlyWorkspace<Input>,
    decide: impl FnOnce(&Layout, Input, &str, bool) -> Result<EntryWrite, ActionError>,
) -> Result<EntryWrite, ActionError>
where
    Input: rorolala_protocol::Encodable + Send + Sync + 'static,
{
    remote_transact(ctx, input, decide).await
}

/// Runs one exchange against the Vault: the input there, the answer of `decide` back.
///
/// It is [`remote_write`] with the answer left to the caller, so an action that writes more than
/// one thing — a sync's batch — carries them all in one exchange rather than one connection and
/// handshake per entry.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if the Vault holds no member to act as,
/// [`ActionError::Codec`] if the input or the answer will not cross, and whatever the exchange
/// fails with.
pub(crate) async fn remote_transact<Input, Output>(
    ctx: &mut ActionContext<'_>,
    input: OnlyWorkspace<Input>,
    decide: impl FnOnce(&Layout, Input, &str, bool) -> Result<Output, ActionError>,
) -> Result<Output, ActionError>
where
    Input: rorolala_protocol::Encodable + Send + Sync + 'static,
    Output: rorolala_protocol::Encodable + Send + Sync + 'static,
{
    let here = input.into_inner();
    let to_vault = here
        .as_ref()
        .map(rorolala_protocol::Encodable::encode)
        .transpose()
        .map_err(codec_failed)?;
    let arrived = carry_blob(ctx, Side::Vault, to_vault.clone()).await?;
    let bytes = arrived.or(to_vault).unwrap_or_default();
    let crossed = Input::decode(bytes).map_err(codec_failed)?;

    let outcome = if ctx.is_vault() {
        let me = ctx
            .get_member()
            .into_inner()
            .map(|member| member.name())
            .ok_or(ActionError::MissingValue)?;
        let admin = ctx
            .current_vault_config()
            .into_inner()
            .is_some_and(|config| config.auth_config().is_admin(&me));

        Some(decide(&local_layout(ctx)?, crossed, &me, admin)?)
    } else {
        None
    };

    let mine = outcome
        .as_ref()
        .map(rorolala_protocol::Encodable::encode)
        .transpose()
        .map_err(codec_failed)?;
    let arrived = carry_blob(ctx, Side::Workspace, mine.clone()).await?;
    let bytes = arrived.or(mine).unwrap_or_default();

    if ctx.is_vault() {
        // UNWRAP: the answer above was made on exactly this side.
        Ok(outcome.unwrap())
    } else {
        Output::decode(bytes).map_err(codec_failed)
    }
}

/// Creates an entry of the Vault's Layout under `@/new/`, held by the account that asks.
///
/// The name is the caller's to choose and only has to be under `@/new/`: when the path the file was
/// submitted under is free it is that path, and when it is taken the caller disambiguates it, so two
/// files submitted under one name are still two paths.
///
/// The entry and the path that names it are made as one change rather than two: a create that was
/// cut in half would leave an entry no path names or a path naming no entry, and a layout holds
/// neither as anything but a `rola layout` reading of its orphans.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the change could not be made.
pub(crate) fn create_remote(
    batch: &mut Batch<'_>,
    path: &str,
    uuid: &str,
    version: &str,
    owner: &str,
) -> Result<EntryWrite, ActionError> {
    let Ok(path) = LayoutPath::new(path) else {
        return Ok(EntryWrite::Malformed);
    };
    let Ok(id) = Uuid::parse_str(uuid.trim()) else {
        return Ok(EntryWrite::Malformed);
    };
    let Ok(version) = Key::from_str(version.trim()) else {
        return Ok(EntryWrite::Malformed);
    };

    if !path.as_str().starts_with(NEW_PREFIX) {
        return Ok(EntryWrite::Malformed);
    }
    if batch.entry(id).is_some() {
        return Ok(EntryWrite::Exists);
    }
    if batch.names(&path) {
        return Ok(EntryWrite::Taken);
    }

    batch
        .create_file(
            id,
            MutableData::new(Some(owner.to_owned()), *version.digest(), String::new()),
            &path,
        )
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(EntryWrite::Written)
}

/// Sets the version an entry of the Vault's Layout is at, leaving its path and holder alone.
///
/// # Errors
///
/// Returns [`ActionError::Store`] if the change could not be made.
pub(crate) fn set_remote(
    batch: &mut Batch<'_>,
    uuid: &str,
    version: &str,
    me: &str,
    admin: bool,
) -> Result<EntryWrite, ActionError> {
    let Ok(id) = Uuid::parse_str(uuid.trim()) else {
        return Ok(EntryWrite::Malformed);
    };
    let Ok(version) = Key::from_str(version.trim()) else {
        return Ok(EntryWrite::Malformed);
    };

    let Some(data) = batch.entry(id) else {
        return Ok(EntryWrite::Missing);
    };
    if !admin && data.owner() != Some(me) {
        return Ok(EntryWrite::Refused);
    }

    batch
        .update_entry(
            id,
            data.renewed(data.owner().map(str::to_owned), *version.digest()),
        )
        .map_err(|error| ActionError::Store(error.to_string()))?;

    Ok(EntryWrite::Written)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use std::str::FromStr as _;

    use rorolala_layout::{Batch, Layout, LayoutPath, MutableData};
    use rorolala_storage::Key;

    use super::{
        EntryWrite, Ownership, PathMove, create_remote, giveup, move_remote, set_remote, take,
    };

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-layout-remote-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// Runs `write` against `layout` as one batch, as an action does, and commits what it collected.
    fn batched<T>(layout: &Layout, write: impl FnOnce(&mut Batch<'_>) -> T) -> T {
        let mut batch = layout.batch();
        let output = write(&mut batch);
        batch.commit().unwrap();

        output
    }

    /// A Layout holding one entry nobody owns.
    fn held(label: &str) -> (Layout, uuid::Uuid) {
        let layout = Layout::open(scratch(label)).unwrap();
        let id = uuid::Uuid::from_u128(1);
        layout
            .create_entry(id, MutableData::new(None, [1; 32], "x".to_owned()))
            .unwrap();

        (layout, id)
    }

    #[test]
    fn an_entry_nobody_holds_is_taken_and_let_go_of() {
        let (layout, id) = held("take");

        assert_eq!(
            batched(&layout, |batch| take(batch, id, "alice")).unwrap(),
            Ownership::Owner(Some("alice".to_owned()))
        );
        // The version and what the entry says are not what an ownership change touches.
        let data = layout.entry(id).unwrap();
        assert_eq!(data.owner(), Some("alice"));
        assert_eq!(data.version(), [1; 32]);
        assert_eq!(data.description(), "x");

        assert_eq!(
            batched(&layout, |batch| giveup(batch, id, "alice")).unwrap(),
            Ownership::Owner(None)
        );
        assert_eq!(layout.entry(id).unwrap().owner(), None);

        // Letting go of what nobody holds is already what was asked for.
        assert_eq!(
            batched(&layout, |batch| giveup(batch, id, "alice")).unwrap(),
            Ownership::Owner(None)
        );
    }

    #[test]
    fn an_entry_another_account_holds_is_left_alone() {
        let (layout, id) = held("held");

        batched(&layout, |batch| take(batch, id, "alice")).unwrap();

        assert_eq!(
            batched(&layout, |batch| take(batch, id, "bob")).unwrap(),
            Ownership::HeldBy("alice".to_owned())
        );
        assert_eq!(
            batched(&layout, |batch| giveup(batch, id, "bob")).unwrap(),
            Ownership::HeldBy("alice".to_owned())
        );

        // Neither refusal changed it.
        assert_eq!(layout.entry(id).unwrap().owner(), Some("alice"));
    }

    #[test]
    fn an_entry_the_layout_does_not_hold_is_reported_missing() {
        let (layout, _) = held("missing");

        assert_eq!(
            batched(&layout, |batch| take(
                batch,
                uuid::Uuid::from_u128(9),
                "alice"
            ))
            .unwrap(),
            Ownership::Missing
        );
        assert_eq!(
            batched(&layout, |batch| giveup(
                batch,
                uuid::Uuid::from_u128(9),
                "alice"
            ))
            .unwrap(),
            Ownership::Missing
        );
    }

    /// A Layout holding one entry at a marker path, held by `owner`.
    fn at(label: &str, path: &str, owner: Option<&str>) -> (Layout, uuid::Uuid) {
        let layout = Layout::open(scratch(label)).unwrap();
        let id = uuid::Uuid::from_u128(2);
        layout
            .create_entry(
                id,
                MutableData::new(owner.map(str::to_owned), [2; 32], String::new()),
            )
            .unwrap();
        layout
            .create_path(&LayoutPath::new(path).unwrap(), id)
            .unwrap();

        (layout, id)
    }

    #[test]
    fn a_path_is_moved_by_its_holder_or_an_administrator() {
        let (layout, id) = at(
            "move-holder",
            "@/new/临时文件/模型.fbx@a1b2c3",
            Some("alice"),
        );

        // A member who holds neither the entry nor the Vault may not move it.
        assert_eq!(
            batched(&layout, |batch| move_remote(
                batch,
                "@/new/临时文件/模型.fbx@a1b2c3",
                "Models/model.fbx",
                "bob",
                false
            ))
            .unwrap(),
            PathMove::Refused
        );
        assert_eq!(
            layout.id_of(&LayoutPath::new("@/new/临时文件/模型.fbx@a1b2c3").unwrap()),
            Some(id)
        );

        // Its holder may locate it, and the marker path stops naming it.
        assert_eq!(
            batched(&layout, |batch| move_remote(
                batch,
                "@/new/临时文件/模型.fbx@a1b2c3",
                "Models/model.fbx",
                "alice",
                false
            ))
            .unwrap(),
            PathMove::Moved
        );
        assert_eq!(
            layout.id_of(&LayoutPath::new("Models/model.fbx").unwrap()),
            Some(id)
        );
        assert!(
            layout
                .id_of(&LayoutPath::new("@/new/临时文件/模型.fbx@a1b2c3").unwrap())
                .is_none()
        );

        // An administrator may move what it does not hold.
        let (other, other_id) = at("move-admin", "@/new/a.psd@d4e5f6", Some("bob"));
        assert_eq!(
            batched(&other, |batch| move_remote(
                batch,
                "@/new/a.psd@d4e5f6",
                "@/removed/a.psd",
                "carol",
                true
            ))
            .unwrap(),
            PathMove::Moved
        );
        assert_eq!(
            other.id_of(&LayoutPath::new("@/removed/a.psd").unwrap()),
            Some(other_id)
        );
    }

    #[test]
    fn a_move_that_cannot_be_made_says_which_way() {
        let (layout, _) = at("move-missing", "@/new/a.psd@d4e5f6", Some("alice"));

        // Nothing is at the path that was to move.
        assert_eq!(
            batched(&layout, |batch| move_remote(
                batch,
                "nowhere/a.psd",
                "Models/a.psd",
                "alice",
                false
            ))
            .unwrap(),
            PathMove::Missing
        );

        // The path it was to move to already names something.
        assert_eq!(
            batched(&layout, |batch| move_remote(
                batch,
                "@/new/a.psd@d4e5f6",
                "@/new/a.psd@d4e5f6",
                "alice",
                false
            ))
            .unwrap(),
            PathMove::Taken
        );

        // `@` is reserved for the two markers this design names, and a marker names a state
        // rather than a place, so something has to be under it.
        for refused in ["@/archive/a.psd", "@/new", "@/removed"] {
            assert_eq!(
                batched(&layout, |batch| move_remote(
                    batch,
                    "@/new/a.psd@d4e5f6",
                    refused,
                    "alice",
                    true
                ))
                .unwrap(),
                PathMove::Malformed,
                "{refused}"
            );
        }
    }

    #[test]
    fn a_new_entry_goes_under_new_and_is_held_by_its_creator() {
        let layout = Layout::open(scratch("create")).unwrap();
        let id = uuid::Uuid::from_u128(3);
        let version = "11".repeat(32);

        assert_eq!(
            batched(&layout, |batch| create_remote(
                batch,
                "@/new/art/hero.psd@abc1234",
                &id.to_string(),
                &version,
                "alice"
            ))
            .unwrap(),
            EntryWrite::Written
        );
        assert_eq!(layout.entry(id).unwrap().owner(), Some("alice"));
        assert_eq!(
            layout.id_of(&LayoutPath::new("@/new/art/hero.psd@abc1234").unwrap()),
            Some(id)
        );

        // The same `Uuid` again, and a path already taken, are told apart from a write.
        assert_eq!(
            batched(&layout, |batch| create_remote(
                batch,
                "@/new/other.psd@abc1234",
                &id.to_string(),
                &version,
                "alice"
            ))
            .unwrap(),
            EntryWrite::Exists
        );
        assert_eq!(
            batched(&layout, |batch| create_remote(
                batch,
                "@/new/art/hero.psd@abc1234",
                &uuid::Uuid::from_u128(4).to_string(),
                &version,
                "alice"
            ))
            .unwrap(),
            EntryWrite::Taken
        );

        // A path outside `@/new/` is not a name a new entry is made under.
        assert_eq!(
            batched(&layout, |batch| create_remote(
                batch,
                "art/hero.psd",
                &uuid::Uuid::from_u128(4).to_string(),
                &version,
                "alice"
            ))
            .unwrap(),
            EntryWrite::Malformed
        );
    }

    #[test]
    fn a_version_is_set_by_the_holder_and_left_by_others() {
        let (layout, id) = at("set", "@/new/a.psd@d4e5f6", Some("alice"));
        let version = "22".repeat(32);

        assert_eq!(
            batched(&layout, |batch| set_remote(
                batch,
                &id.to_string(),
                &version,
                "bob",
                false
            ))
            .unwrap(),
            EntryWrite::Refused
        );
        assert_eq!(
            batched(&layout, |batch| set_remote(
                batch,
                &id.to_string(),
                &version,
                "alice",
                false
            ))
            .unwrap(),
            EntryWrite::Written
        );
        assert_eq!(
            layout.entry(id).unwrap().version(),
            *Key::from_str(&version).unwrap().digest()
        );

        // A version change touches neither the holder nor the path, and an administrator may
        // make one it does not hold.
        assert_eq!(layout.entry(id).unwrap().owner(), Some("alice"));
        assert_eq!(
            layout.id_of(&LayoutPath::new("@/new/a.psd@d4e5f6").unwrap()),
            Some(id)
        );
        assert_eq!(
            batched(&layout, |batch| set_remote(
                batch,
                &id.to_string(),
                &version,
                "carol",
                true
            ))
            .unwrap(),
            EntryWrite::Written
        );

        // A `Uuid` the Layout does not hold is missing.
        assert_eq!(
            batched(&layout, |batch| set_remote(
                batch,
                &uuid::Uuid::from_u128(9).to_string(),
                &version,
                "alice",
                false
            ))
            .unwrap(),
            EntryWrite::Missing
        );
    }
}

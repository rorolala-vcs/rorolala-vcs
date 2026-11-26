//! Carrying a sync plan out: the Layout, the index and the content.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr as _;

use rorolala_layout::{Batch, Layout, LayoutPath};
use rorolala_protocol::VaultAddress;
use rorolala_storage::{Key, StorageSet};
use rorolala_tree::{
    Baseline, EntryKind as TreeEntryKind, Staging, Tree, TreeEntry, TreeStructure,
};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_progress::{Direction as MoveDirection, Progress};
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use uuid::Uuid;

use crate::daemon::{
    EntryWrite, RemoteCreate, RemoteSet, action_sync_index_all_async, action_write_remote_entries,
};
use crate::fetch::Sources;
use crate::write::bring::write_worktree;
use crate::write::checkout::Checkout;
use crate::write::layout::{add_cached_entry, cached_copy, set_cached_version};
use crate::write::progress::WriteProgress;
use crate::write::sync::{self, Entry, Kind, SyncInput, SyncRequest};
use crate::write::tree_remove::prune_empty;
use crate::write::{Cancellation, WriteError};
use rorolala_utils_location::Locate as _;

/// The plan an apply was handed, read only for what it promised to move.
#[derive(Debug, Clone)]
struct GivenPlan {
    /// One entry per entry the plan named, which is none for a plan of nothing.
    entries: Vec<GivenEntry>,
}

/// One entry the handed plan named, as an apply reads it back.
#[derive(Debug, Clone)]
pub struct GivenEntry {
    /// The `Uuid` it is known by.
    pub uuid: String,
    /// The path this Layout names it by.
    pub local_path: Option<String>,
    /// The path the Vault's Layout names it by.
    pub remote_path: Option<String>,
    /// The name the Vault's Layout is to give it.
    pub planned_path: Option<String>,
    /// The version this Layout is at.
    pub local_version: Option<String>,
    /// The version the Vault is at.
    pub remote_version: Option<String>,
    /// What the plan means to do with it.
    pub kind: Kind,
}

/// One entry a plan promised to move, as a C caller hands it back.
///
/// # FFI
///
/// A `RolaWriteSyncAgreed` the caller fills from what a plan answered. Every `const char *` is
/// borrowed for the call and released by the caller; an empty path or version says the plan named
/// none, and `kind` is the word the plan's entry answered `_kind` with.
#[lazyffi(export = RolaWriteSyncAgreed, input)]
#[derive(Debug, Clone)]
pub struct SyncAgreed {
    /// The `Uuid` the entry is known by.
    pub uuid: String,
    /// The path this Layout names it by, empty when it names none.
    pub local_path: String,
    /// The path the Vault's Layout names it by, empty when it names none.
    pub remote_path: String,
    /// The name the Vault's Layout is to give it, empty when this creates none.
    pub planned_path: String,
    /// The version this Layout is at, empty when it is at none.
    pub local_version: String,
    /// The version the Vault is at, empty when the Vault holds none.
    pub remote_version: String,
    /// What the plan means to do with it, as the plan's `_kind` word.
    pub kind: String,
}

impl SyncAgreed {
    /// The entry this names, or why its `kind` names nothing this build writes.
    fn given(&self) -> Result<GivenEntry, WriteError> {
        let kind = Kind::from_word(&self.kind).ok_or_else(|| WriteError::Precondition {
            what: "the sync plan".to_owned(),
            reason: format!(
                "its entry kind `{}` is not one this build writes",
                self.kind
            ),
        })?;

        Ok(GivenEntry {
            uuid: self.uuid.clone(),
            local_path: none_if_empty(&self.local_path),
            remote_path: none_if_empty(&self.remote_path),
            planned_path: none_if_empty(&self.planned_path),
            local_version: none_if_empty(&self.local_version),
            remote_version: none_if_empty(&self.remote_version),
            kind,
        })
    }
}

/// The text, or nothing when it names none.
fn none_if_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_owned())
}

/// Which sides a sync run leaves alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct Skip {
    /// Whether the index is left alone.
    pub index: bool,
    /// Whether the store is left alone.
    pub storage: bool,
    /// Whether the Vault's Layout is left alone.
    pub layout: bool,
}

/// What a sync run may do beside reading the plan.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Whether a version whose chain moved apart may go up anyway.
    pub forced: bool,
    /// How many connections the content is carried over.
    pub concurrency: usize,
    /// What the run leaves alone.
    pub skip: Skip,
}

/// The words a sync failure is said in.
#[derive(Debug, Clone)]
pub struct Messages {
    /// A malformed entry.
    pub malformed: String,
    /// An entry whose content is not here.
    pub no_content: String,
    /// An entry whose content is missing.
    pub missing: String,
    /// A version the run may force.
    pub force: String,
    /// An entry the account does not hold.
    pub refused: String,
    /// A file and a tree named by one Uuid.
    pub kind_mismatch: String,
    /// A written entry.
    pub written: String,
    /// An entry that already exists.
    pub exists: String,
    /// An entry taken by another account.
    pub taken: String,
}

impl Default for Messages {
    fn default() -> Self {
        Self {
            malformed: "the entry does not read".to_owned(),
            no_content: "the entry's content is not here".to_owned(),
            missing: "the entry is missing".to_owned(),
            force: "the version moved apart, so only --force sends it".to_owned(),
            refused: "another account holds the entry".to_owned(),
            kind_mismatch: "a file and a tree share the Uuid".to_owned(),
            written: "written".to_owned(),
            exists: "already exists".to_owned(),
            taken: "another account holds it".to_owned(),
        }
    }
}
/// What one sync run works over: the two Layouts, the index, the store and the Vaults it may ask.
pub struct Carry<'a> {
    /// The Workspace being worked in.
    pub workspace: &'a crate::workspace::Workspace,
    /// The account the exchange is spoken as.
    pub account: &'a crate::auth::Account,
    /// The Vault being reached.
    pub target: &'a crate::protocol::VaultAddress,
    /// The Layout being worked in, whose versions a pull sets.
    pub layout: &'a Layout,
    /// The index below both sides.
    pub index: &'a VCSIndex,
    /// The store content is written from, when the run has one.
    pub store: Option<&'a StorageSet>,
    /// The Vaults the run may reach, for content that is not here.
    pub candidates: &'a [(String, VaultAddress)],
    /// The words a failure is said in.
    pub messages: &'a Messages,
    /// The runtime the asynchronous exchanges are waited on.
    pub runtime: &'a tokio::runtime::Runtime,
}

/// Writes what a plan names, and moves the content that gives it meaning.
///
/// Content is moved before the Layout is touched either way: a version is only a meaning once what
/// it stored is here, and what goes up names versions the Vault has to be able to read.
///
/// The two exchanges say their own progress (they are actions, and an action reports itself) and
/// what is left, one entry at a time, is said here, since no action knows it is part of a longer
/// run. The watching the bars are drawn into is the caller's.
///
/// The exchange is asynchronous and a command is not, so the runtime made by the caller waits for
/// it, and what it is doing is said to a reader on a thread of its own.
///
/// # Errors
///
/// Returns [`WriteError::Internal`] when the exchange or a read under it fails, and
/// [`WriteError::Cancelled`] when `cancellation` was set before the run or between two of its
/// entries.
// The order the two exchanges and the entries take is one story, and the comments state why each
// step comes where it does; splitting it would separate a step from the reason it is there.
#[allow(
    clippy::too_many_lines,
    reason = "the order the exchanges and the entries take is one story, and each comment explains why its step comes where it does"
)]
pub fn apply(
    progress: &Progress,
    plan: Option<&sync::Plan>,
    options: Options,
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
    cancellation: &Cancellation,
) -> Result<Applied, WriteError> {
    // A run asked to stop before it began writes nothing: the exchanges below are the work, and
    // doing them for a run nobody waits for is work for nothing.
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled);
    }

    // A plan is what names the content to move, and `--no-layout` is the run that has none. Reading
    // no Layout must not leave the content behind, though: `--no-storage` is the only thing that
    // suppresses moving content. So with no plan what moves is everything the index holds, the
    // stored object every variant points at, the new ones included, and it is read here, before the
    // index is synced, so what crosses is this side's own work rather than everything the Vault
    // holds as well.
    //
    // The list is read even when `--no-storage` leaves the store behind, because it is also the list
    // a run with no plan reports having asked for.
    let indexed = plan.is_none().then(|| indexed(carry));

    let entries = plan_entries(plan);

    let sources = sources_of(options, carry, progress).map_err(internal)?;

    // What moves the content, which is the plan's entries or, with no plan, everything the index
    // names. Both the orders below ask for it, so what it does is stated once.
    let carry_content = || -> Result<usize, WriteError> {
        let Some(sources) = &sources else {
            return Ok(0);
        };

        if let Some(keys) = &indexed {
            return sources.send(carry.store, keys).map_err(internal);
        }

        let (receive, send) = content_of(&entries, options, carry, cancellation)?;

        if let Some(store) = carry.store {
            sources.bring(store, &receive);

            bring_tree_inside(&entries, carry, store, sources);
        }

        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        sources.send(carry.store, &send).map_err(internal)
    };

    // Which of the two exchanges goes first is decided by what names what. With no plan the Vault's
    // index is about to name variants this run sends, so the content goes first: a failed upload
    // then leaves content nobody names (harmless, since a key is its content) rather than a name
    // pointing at nothing until a later run repairs it. With a plan it is the other way round: a
    // version to bring down is resolved through index objects only the Vault holds, so the index has
    // to cross before its content can be found.
    let storage_objects;
    if plan.is_none() {
        storage_objects = carry_content()?;
        sync_index(carry, options, progress).map_err(internal)?;
    } else {
        sync_index(carry, options, progress).map_err(internal)?;
        storage_objects = carry_content()?;
    }

    // What a run with no plan asked the Vault for is what the index named: the content every variant
    // points at. The index objects themselves cross in an exchange of their own, which answers with
    // no count, the action is already exported to C with a `()` output, so what is reported is the
    // list this run made, not what the Vault answered with.
    let index_objects = if options.skip.index {
        0
    } else {
        indexed.as_ref().map_or(0, Vec::len)
    };

    let up_total = count(&entries, false);
    let down_total = count(&entries, true);

    let mut moves = progress.begin("", Some(up_total + down_total));
    let mut up = (up_total > 0).then(|| moves.spawn(MoveDirection::Up, "", Some(up_total)));
    let mut down = (down_total > 0).then(|| moves.spawn(MoveDirection::Down, "", Some(down_total)));

    let mut applied = 0;
    let mut received = 0;
    let mut failed = Vec::new();

    // Every path a pull writes is remembered in the one reading, which is written once at the end
    // rather than read back and rewritten for each file brought down; the version each pulled
    // entry names is written to the Layout in one batch for the same reason.
    let mut checkout = Checkout::open(carry.layout, carry.workspace.get_root());
    let mut batch = carry.layout.batch();

    // What this side is ahead on is gathered and written in one exchange at the end, rather than
    // one connection and handshake for each entry.
    let mut ups: Vec<&Entry> = Vec::new();

    for entry in entries.iter().copied() {
        // A pull writes the tree as it goes, so the stop is checked before each entry rather than
        // only before the exchange that follows them.
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        if entry.kind == Kind::Receive {
            if let Some(task) = &mut down {
                let _named = task.doing(name_of(entry));
            }

            received += usize::from(
                receive(entry, carry, &mut checkout, &mut batch, &mut failed).map_err(internal)?,
            );

            if let Some(task) = &mut down {
                task.advance_by(1);
            }

            moves.advance_by(1);
        } else {
            ups.push(entry);
        }
    }

    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled);
    }

    apply_entries(
        &ups,
        options.forced,
        carry,
        vault_name,
        account_name,
        &mut applied,
        &mut failed,
    )
    .map_err(internal)?;

    // What is written up is one exchange of many entries rather than a step each, so its share of
    // the bars is taken once it is done: the whole bar must not read as finished while the exchange
    //, the step that reaches the network, is still in front of the reader.
    if let Some(task) = &mut up {
        task.advance_by(up_total);
    }
    moves.advance_by(up_total);

    if let Err(cause) = checkout.finish() {
        failed.push(cause.to_string());
    }

    if let Err(error) = batch.commit() {
        failed.push(error.to_string());
    }

    let took = sources.as_ref().map_or_else(Vec::new, Sources::took);

    // A run that read no Layout left this side's copy of what the Vault holds exactly as it found
    // it, which is what the reading commands go on answering from until something fetches. Saying
    // so is what keeps a reader from taking a version graph for the Vault's current options.
    Ok(Applied {
        applied,
        received,
        failed,
        index_objects,
        storage_objects,
        missing_layout: plan.is_none(),
        took,
    })
}

/// The failure an apply reports when a step under it falls short.
const fn internal(reason: String) -> WriteError {
    WriteError::Internal { reason }
}

/// The entries a plan says are to move.
///
/// The content is asked for one object at a time, by what the plan names: an object a run needs is
/// fetched, and one it does not is never reached for. An entry both sides already hold at the same
/// version, and one only the Vault holds, names nothing this run moves.
fn plan_entries(plan: Option<&sync::Plan>) -> Vec<&Entry> {
    plan.map_or_else(Vec::new, |plan| {
        plan.entries
            .iter()
            .filter(|entry| !matches!(entry.kind, Kind::UpToDate | Kind::RemoteOnly))
            .collect()
    })
}

/// The Vaults a run may send content to, or `None` when `--no-storage` leaves the store alone.
///
/// # Errors
///
/// Returns a message when the runtime the exchanges are waited on could not be made.
fn sources_of<'a>(
    options: Options,
    carry: &Carry<'a>,
    progress: &Progress,
) -> Result<Option<Sources<'a>>, String> {
    if options.skip.storage {
        return Ok(None);
    }

    // Offline is not passed through: `rola sync` is refused before it gets here, since the exchange
    // with the Vault is the whole of what it does.
    let sources = Sources::new(
        carry.workspace,
        carry.account,
        carry.candidates.to_vec(),
        false,
    )?
    .reporting(progress.clone())
    .with_concurrency(options.concurrency);

    Ok(Some(sources))
}

/// What carrying a plan out came to.
///
/// It is the shape the one-shot `rola_write_sync` export has always answered with, and it keeps
/// it: the request-shaped apply answers a [`Report`] instead, which is the shape the command line's
/// `--json` writes.
#[derive(Debug)]
pub struct Applied {
    /// How many entries were written to the Vault.
    pub applied: usize,
    /// How many versions were brought down from the Vault.
    pub received: usize,
    /// What was not done, one line each.
    pub failed: Vec<String>,
    /// How many stored objects the index named, which is what a run with no plan asked for.
    pub index_objects: usize,
    /// How many units of stored content the run sent.
    pub storage_objects: usize,
    /// Whether the Vault Layout was not read, so the copy still says what it said.
    pub missing_layout: bool,
    /// What each Vault handed over.
    pub took: Vec<(String, usize)>,
}

/// What carrying a plan out came to, in the shape the command line's `--json` writes it.
#[derive(Debug)]
pub struct Report {
    /// The Layout the run was about.
    pub layout: String,
    /// The Vault it tracks.
    pub vault: String,
    /// How many entries were written to the Vault.
    pub applied: usize,
    /// How many versions were brought down from the Vault.
    pub received: usize,
    /// How many entries both sides already held at the same version.
    pub up_to_date: usize,
    /// What was not done, one line each.
    pub failed: Vec<String>,
    /// Whether the Vault's Layout was read at all.
    pub layout_read: bool,
    /// How many stored objects the index named, which is what a run with no plan asked for.
    pub index_objects: usize,
    /// How many units of stored content the run sent.
    pub storage_objects: usize,
}

/// Makes the two indexes hold the same objects, unless the run leaves the index alone.
///
/// # Errors
///
/// Returns a message when the exchange could not be made.
fn sync_index(carry: &Carry<'_>, options: Options, progress: &Progress) -> Result<(), String> {
    if options.skip.index {
        return Ok(());
    }

    carry
        .runtime
        .block_on(action_sync_index_all_async(
            carry.workspace,
            carry.account,
            carry.target.to_string(),
            progress.clone(),
        ))
        .map_err(|error| error.to_string())
}

/// Every stored object the index names, for a run with no plan to name content by.
///
/// A variant is meaningful only once what it stored is beside it, and a run that read no Layout has
/// no list of versions to work from: what the index holds is the list. Every variant's content is
/// named, so the new variants a run just made have theirs carried the same way the old ones do.
///
/// A tree variant's stored object is its structure, and the structure names the description and every
/// file's content. Those are read here as well, because a run with no plan is still the run that has
/// to leave the Vault holding a version a later Workspace can put back: the structures alone are a
/// record of files whose content never crossed.
///
/// The scan is the index's own rather than a listing of every object read whole: only the Variants
/// are decoded, since only they name content, and the other kinds are skipped by the kind their
/// bytes lead with.
fn indexed(carry: &Carry<'_>) -> Vec<Key> {
    let Ok(hashes) = carry.runtime.block_on(carry.index.variant_storage_hashes()) else {
        return Vec::new();
    };

    let mut keys: Vec<Key> = hashes.into_iter().map(Key::new).collect();

    if let Some(store) = carry.store {
        let mut inside = Vec::new();

        for key in &keys {
            if let Ok(bytes) = carry.runtime.block_on(store.read_object(key))
                && let Ok(structure) = crate::tree::TreeStructure::decode(&bytes)
            {
                inside.extend(structure.content_keys());
            }
        }

        keys.extend(inside);
    }

    keys.sort_unstable();
    keys.dedup();

    keys
}

/// The content a plan needs moved: the objects a pull writes here, and the objects a push puts in
/// the Vault's store.
///
/// A version is only a meaning once what it stored is in the store that names it, so each entry
/// names what the version it moves stands on as well as the version itself, the whole chain back to
/// its root. Naming only the version at the top is what left every version below it a name in the
/// Layout with no content behind it: the chain crossed, the content did not, and a later run that
/// needed an older version found a name nothing could produce.
///
/// An entry that is neither (a refusal, a block, or a `Diverged` a run is not forcing) names
/// nothing to move, and its content is never asked for.
///
/// # Errors
///
/// Returns [`WriteError::Cancelled`] when `cancellation` was set between two entries.
fn content_of(
    entries: &[&Entry],
    options: Options,
    carry: &Carry<'_>,
    cancellation: &Cancellation,
) -> Result<(Vec<Key>, Vec<Key>), WriteError> {
    let mut receive = Vec::new();
    let mut send = Vec::new();

    for entry in entries {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        let pull = entry.kind == Kind::Receive;
        let push = match entry.kind {
            Kind::Create | Kind::Send => true,
            Kind::Diverged => options.forced,
            _ => false,
        };

        if !pull && !push {
            continue;
        }

        let named = if pull {
            entry.remote_version.as_deref()
        } else {
            entry.local_version.as_deref()
        };
        let Some(version) = named.and_then(|text| Key::from_str(text).ok()) else {
            continue;
        };

        // The version and everything it stands on: a version read back needs its own content, and
        // the versions below it are what a run reaches for when it moves a pointer back.
        let Ok(chain) = carry.runtime.block_on(sync::chain_store_keys(
            carry.index,
            version.digest(),
            carry.store,
            !pull,
        )) else {
            continue;
        };

        if pull {
            receive.extend(chain);
        } else {
            send.extend(chain);
        }
    }

    // One object is asked for once, however many chains name it: a store is content-addressed, and
    // the list is what the exchange counts and asks about.
    receive.sort_unstable();
    receive.dedup();
    send.sort_unstable();
    send.dedup();

    Ok((receive, send))
}

/// How many of `entries` move down, or up when `down` is false.
fn count(entries: &[&Entry], down: bool) -> u64 {
    let moved = entries
        .iter()
        .filter(|entry| (entry.kind == Kind::Receive) == down)
        .count();

    u64::try_from(moved).unwrap_or(u64::MAX)
}

/// Brings what a tree's structure names, after the structure crossed with the first exchange.
///
/// The version's storage key is the structure, and only reading it says which keys the files inside
/// hold. The second exchange asks for those, so a pulled tree's files are here before the Layout
/// names the version that holds them.
fn bring_tree_inside(
    entries: &[&Entry],
    carry: &Carry<'_>,
    store: &StorageSet,
    sources: &Sources<'_>,
) {
    let mut inside: Vec<Key> = Vec::new();

    for entry in entries {
        if entry.kind != Kind::Receive || entry.item_kind != Some("tree") {
            continue;
        }
        let Some(version) = entry
            .remote_version
            .as_deref()
            .and_then(|text| Key::from_str(text).ok())
        else {
            continue;
        };

        if let Ok(keys) = carry.runtime.block_on(sync::tree_internal_keys(
            carry.index,
            store,
            version.digest(),
        )) {
            inside.extend(keys);
        }
    }

    inside.sort_unstable();
    inside.dedup();

    if !inside.is_empty() {
        sources.bring(store, &inside);
    }
}

/// The name an entry is shown by while it is being moved.
fn name_of(entry: &Entry) -> &str {
    entry
        .local_path
        .as_deref()
        .or(entry.remote_path.as_deref())
        .or(entry.planned_path.as_deref())
        .unwrap_or_default()
}

/// Why a run could not make a plan.
/// Brings one entry's version down: what it stored into the tree, and the version into the Layout.
///
/// The path the file is at here is this side's own and is not moved by a pull; what the pull sets
/// is the version and who holds it, since the Vault is where both are kept.
///
/// A tree is put back as a directory rather than written at its path as content: its version names a
/// structure, and the work the structure describes is what the directory is made of. Writing the
/// structure there would put a file where a directory belongs, which is why the two shapes take
/// different ways down.
///
/// # Errors
///
/// Returns a message when the index could not be read.
fn receive(
    entry: &Entry,
    carry: &Carry<'_>,
    checkout: &mut Checkout,
    batch: &mut Batch<'_>,
    failed: &mut Vec<String>,
) -> Result<bool, String> {
    let (Some(text), Ok(id)) = (entry.remote_version.as_deref(), Uuid::from_str(&entry.uuid))
    else {
        failed.push(format!("{}: {}", entry.uuid, carry.messages.malformed));

        return Ok(false);
    };

    let Ok(version) = Key::from_str(text) else {
        failed.push(format!("{}: {}", entry.uuid, carry.messages.malformed));

        return Ok(false);
    };

    // What the version names has to be here before the tree can be given it.
    let stored = carry
        .runtime
        .block_on(sync::store_of(carry.index, version.digest()))?;

    if let Some(path) = entry.local_path.as_deref() {
        let (Some(stored), Some(store)) = (stored, carry.store) else {
            failed.push(format!("{}: {}", entry.uuid, carry.messages.no_content));

            return Ok(false);
        };

        let target = carry.workspace.get_root().join(path);

        if let Some(parent) = target.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            failed.push(format!("{}: {error}", entry.uuid));

            return Ok(false);
        }

        let written = if entry.item_kind == Some("tree") {
            receive_tree(
                &target,
                carry.workspace.get_root(),
                id,
                version.digest(),
                &stored,
                store,
                carry.runtime,
            )
        } else {
            carry
                .runtime
                .block_on(store.extract_file(&stored, &target))
                .map_err(|error| error.to_string())
        };

        if let Err(error) = written {
            failed.push(format!("{}: {error}", entry.uuid));

            return Ok(false);
        }

        // What a pull wrote is what the Layout now names, so the reading has to be told; a path
        // left marked as disagreed would be reported as changed by the very next run.
        if let Err(cause) = checkout.remember(path) {
            failed.push(format!("{}: {cause}", entry.uuid));

            return Ok(false);
        }
    }

    let Some(data) = carry.layout.entry(id) else {
        failed.push(format!("{}: {}", entry.uuid, carry.messages.missing));

        return Ok(false);
    };

    if let Err(error) = batch.update_entry(id, data.renewed(entry.owner.clone(), *version.digest()))
    {
        failed.push(format!("{}: {error}", entry.uuid));

        return Ok(false);
    }

    Ok(true)
}

/// Writes a tree version's work under `root`, replacing what the tree recorded before.
///
/// The version's stored object is the structure, so this is not a file to extract at the path: the
/// structure says which directory and which file hold what, and the content it names is already in
/// this side's store, asked for by [`bring_tree_inside`] before the run reached here.
///
/// What the previous structure named and this one does not is taken away first, so a path the Vault
/// removed does not stay behind under a tree that now says it is gone. A path whose kind changed is
/// taken away too, since a file cannot be written where a directory stands, nor the other way.
///
/// # Errors
///
/// Returns what the store, the directory or the mark could not do.
fn receive_tree(
    root: &Path,
    workspace_root: &Path,
    id: Uuid,
    version: &[u8; 32],
    stored: &Key,
    store: &StorageSet,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), String> {
    let bytes = runtime
        .block_on(store.read_object(stored))
        .map_err(|error| error.to_string())?;
    let structure = TreeStructure::decode(&bytes).map_err(|error| error.to_string())?;

    if let Some(previous) = Baseline::new(root)
        .tree()
        .map_err(|error| error.to_string())?
    {
        remove_stale(root, &previous, &structure);
    }

    write_worktree(root, &structure, store, runtime)?;

    Staging::new(root)
        .ensure_store()
        .map_err(|error| error.to_string())?;
    let tree = Tree::open(workspace_root, root).map_err(|error| error.to_string())?;
    tree.baseline()
        .write(&structure, version)
        .map_err(|error| error.to_string())?;
    tree.write_local_id(id).map_err(|error| error.to_string())?;

    Ok(())
}

/// Takes away what `previous` named and `structure` does not, under `root`.
///
/// Paths are taken deepest first: a directory may only be removed once what it held is gone, and a
/// file written over a directory, or the reverse, needs the old shape cleared before the new one
/// arrives. A directory goes only when it is empty, so work the structure never named, a nested tree
/// among it, is left where it is.
fn remove_stale(root: &Path, previous: &TreeStructure, structure: &TreeStructure) {
    let kept: BTreeMap<&str, TreeEntryKind> = structure
        .entries()
        .iter()
        .map(|entry| (entry.path(), entry.kind()))
        .collect();

    let mut gone: Vec<&TreeEntry> = previous
        .entries()
        .iter()
        .filter(|entry| kept.get(entry.path()) != Some(&entry.kind()))
        .collect();

    gone.sort_by_key(|entry| Reverse(entry.path().len()));

    for entry in gone {
        let at = root.join(entry.path());

        match entry.kind() {
            TreeEntryKind::Directory => {
                let _ = std::fs::remove_dir(&at);
            }
            TreeEntryKind::File | TreeEntryKind::Symlink => {
                let _ = std::fs::remove_file(&at);
                prune_empty(root, at.parent());
            }
        }
    }
}

/// Writes what the Vault is behind on, in one exchange, counting each done or naming why it was not.
///
/// A run ahead on many entries asks for them all at once: each would otherwise be its own connection
/// and handshake, and the writes have nothing to say to one another. What the Vault takes is written
/// into the fetched copy as well, so that a reading command sees the Vault as this run left it rather
/// than as it found it. That copy is what `--layout truth@<VAULT>` reads, and one that lagged behind
/// would make a run look as though it had not happened.
///
/// # Errors
///
/// Returns the exchange's own failure, or the outcomes that will not read.
fn apply_entries(
    ups: &[&Entry],
    forced: bool,
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
    applied: &mut usize,
    failed: &mut Vec<String>,
) -> Result<(), String> {
    // What is asked for, paired with the entry it is asked for, so an answer can be put back to it.
    let mut creates: Vec<RemoteCreate> = Vec::new();
    let mut create_entries: Vec<&Entry> = Vec::new();
    let mut sets: Vec<RemoteSet> = Vec::new();
    let mut setting: Vec<&Entry> = Vec::new();

    for entry in ups {
        match entry.kind {
            Kind::Create => {
                let (Some(path), Some(version)) = (
                    entry.planned_path.as_deref(),
                    entry.local_version.as_deref(),
                ) else {
                    continue;
                };

                creates.push(RemoteCreate {
                    path: path.to_owned(),
                    uuid: entry.uuid.clone(),
                    version: version.to_owned(),
                    kind: entry.item_kind.unwrap_or("file").to_owned(),
                });
                create_entries.push(entry);
            }
            Kind::Send | Kind::Diverged => {
                if entry.kind == Kind::Diverged && !forced {
                    failed.push(format!("{}: {}", entry.uuid, carry.messages.force));

                    continue;
                }

                let Some(version) = entry.local_version.as_deref() else {
                    continue;
                };

                sets.push(RemoteSet {
                    uuid: entry.uuid.clone(),
                    version: version.to_owned(),
                });
                setting.push(entry);
            }
            Kind::Refused => {
                failed.push(format!("{}: {}", entry.uuid, carry.messages.refused));
            }
            Kind::Mismatched => {
                failed.push(format!("{}: {}", entry.uuid, carry.messages.kind_mismatch));
            }
            _ => {}
        }
    }

    if creates.is_empty() && sets.is_empty() {
        return Ok(());
    }

    let answer = action_write_remote_entries(
        carry.workspace,
        carry.account,
        carry.target.to_string(),
        &creates,
        &sets,
    )
    .map_err(|error| error.to_string())?;

    let outcomes = answer.outcomes();

    // The outcomes come back in the order asked for: the creates first, then the sets.
    let mut outcomes = outcomes.into_iter();
    let mut written: Vec<&Entry> = Vec::new();
    for &entry in &create_entries {
        let Some(outcome) = outcomes.next() else {
            break;
        };

        record(
            entry,
            outcome,
            &mut written,
            applied,
            failed,
            carry.messages,
        );
    }
    for &entry in &setting {
        let Some(outcome) = outcomes.next() else {
            break;
        };

        record(
            entry,
            outcome,
            &mut written,
            applied,
            failed,
            carry.messages,
        );
    }

    write_fetched_copy(&written, carry, vault_name, account_name, failed);

    Ok(())
}

/// Puts one answer back to the entry it belongs to.
fn record<'a>(
    entry: &'a Entry,
    outcome: EntryWrite,
    written: &mut Vec<&'a Entry>,
    applied: &mut usize,
    failed: &mut Vec<String>,
    messages: &Messages,
) {
    match outcome {
        EntryWrite::Written => {
            *applied += 1;
            written.push(entry);
        }
        other => failed.push(format!("{}: {}", entry.uuid, write_reason(other, messages))),
    }
}

/// Writes down in the fetched copy what the Vault took, in one go.
///
/// The copy is opened once and every change is gathered into one batch, so a run that writes many
/// entries reads and writes the copy once rather than once per entry.
fn write_fetched_copy(
    entries: &[&Entry],
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
    failed: &mut Vec<String>,
) {
    if entries.is_empty() {
        return;
    }

    let copy = match cached_copy(carry.workspace, vault_name) {
        Ok(Some(copy)) => copy,
        Ok(None) => return,
        Err(error) => {
            failed.push(error.to_string());

            return;
        }
    };

    let mut batch = copy.batch();
    for entry in entries {
        if let Err(cause) = remember_written(&mut batch, entry, account_name, carry.messages) {
            failed.push(format!("{}: {cause}", entry.uuid));
        }
    }

    if let Err(error) = batch.commit() {
        failed.push(error.to_string());
    }
}

/// Writes down in the fetched copy what the Vault just took.
///
/// A new `Uuid` is added at the name it was made under, held by this account; one the Vault was
/// behind on has its version moved. Nothing else about the copy changes, since nothing else about
/// the Vault did.
///
/// # Errors
///
/// Returns a message when the entry does not read or the change cannot be made in `copy`.
fn remember_written(
    copy: &mut Batch<'_>,
    entry: &Entry,
    account_name: &str,
    messages: &Messages,
) -> Result<(), String> {
    let Ok(id) = Uuid::from_str(&entry.uuid) else {
        return Err(messages.malformed.clone());
    };
    let Some(version) = entry.local_version.as_deref() else {
        return Ok(());
    };
    let Ok(version) = Key::from_str(version) else {
        return Err(messages.malformed.clone());
    };

    match entry.kind {
        Kind::Create => {
            let Some(path) = entry.planned_path.as_deref() else {
                return Ok(());
            };
            let Ok(path) = LayoutPath::new(path) else {
                return Err(messages.malformed.clone());
            };

            add_cached_entry(
                copy,
                id,
                &path,
                *version.digest(),
                Some(account_name.to_owned()),
            )
            .map_err(|error| error.to_string())
        }
        Kind::Send | Kind::Diverged => {
            set_cached_version(copy, id, *version.digest()).map_err(|error| error.to_string())
        }
        _ => Ok(()),
    }
}

/// What did not happen to one entry, in words.
fn write_reason(outcome: EntryWrite, messages: &Messages) -> String {
    match outcome {
        EntryWrite::Written => messages.written.clone(),
        EntryWrite::Exists => messages.exists.clone(),
        EntryWrite::Taken => messages.taken.clone(),
        EntryWrite::Missing => messages.missing.clone(),
        EntryWrite::Refused => messages.refused.clone(),
        EntryWrite::Malformed => messages.malformed.clone(),
    }
}

/// Carries a sync run out under a Workspace.
///
/// # FFI
///
/// A `RolaWriteSync *` the caller releases with `free_rola_write_sync`, or a `RolaWriteError *`
/// the caller releases with `free_rola_write_error`. `direction` is `up`, `down` or `both`;
/// `layout` says whether the Vault's Layout is read and written. `key_path` is the private key the
/// run signs with.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] or [`WriteError::NoIndex`] when the parts are missing,
/// [`WriteError::NotCached`] when the Vault's Layout was never fetched here, and
/// [`WriteError::Internal`] when the exchange fails.
#[allow(
    clippy::fn_params_excessive_bools,
    clippy::too_many_arguments,
    reason = "the C ABI spells each side a sync may leave alone as its own bool, beside the account and address it signs for"
)]
#[lazyffi(export = rola_write_sync)]
pub fn sync_at(
    directory: &std::path::Path,
    vault: &str,
    account: &str,
    key_path: &str,
    address: &str,
    direction: &str,
    forced: bool,
    storage: bool,
    index: bool,
    layout: bool,
) -> Result<WriteSync, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let read_layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let vcs = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let store = workspace.storage_set().ok();
    let target = VaultAddress::from_str(address).map_err(|error| WriteError::InvalidPath {
        path: address.to_owned(),
        reason: error.to_string(),
    })?;
    let signing = crate::auth::Account::from_key_path(account, key_path);
    let direction = match direction {
        "up" => sync::Direction::Up,
        "down" => sync::Direction::Down,
        "both" => sync::Direction::Both,
        other => {
            return Err(WriteError::Internal {
                reason: format!("unknown sync direction `{other}`"),
            });
        }
    };
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let plan = if layout {
        let copy = workspace
            .readonly_layout_copy(vault, VAULT_LAYOUT_NAME)
            .ok_or(WriteError::NotCached)?;
        let remote =
            crate::layout::Layout::open_existing(&copy).map_err(|error| WriteError::Layout {
                reason: error.to_string(),
            })?;
        Some(runtime.block_on(sync::plan(
            &workspace,
            &read_layout,
            &remote,
            &vcs,
            account,
            direction,
            forced,
            // A one-shot export has no handle to be cancelled by, so the run it makes is
            // never stopped. The request-shaped entries take the caller's own flag.
            &Cancellation::new(),
        ))?)
    } else {
        None
    };
    let messages = Messages::default();
    let candidates = vec![(vault.to_owned(), target.clone())];
    let carry = Carry {
        workspace: &workspace,
        account: &signing,
        target: &target,
        layout: &read_layout,
        index: &vcs,
        store: store.as_ref(),
        candidates: &candidates,
        messages: &messages,
        runtime: &runtime,
    };
    let options = Options {
        forced,
        concurrency: 1,
        skip: Skip {
            index: !index,
            storage: !storage,
            layout: !layout,
        },
    };
    let applied = apply(
        &Progress::silent(),
        plan.as_ref(),
        options,
        &carry,
        vault,
        account,
        &Cancellation::new(),
    )?;

    Ok(WriteSync::of(&applied))
}

/// What a sync run came to, as the one-shot export hands it over.
///
/// The run it answers has no plan the caller agreed to, so it says what it did rather than what it
/// was to do: how much each side moved, what was not done, and which Vaults handed something over.
///
/// # FFI
///
/// A `RolaWriteSync *` the caller releases with `free_rola_write_sync`. `_applied`, `_received`,
/// `_index_objects` and `_storage_objects` are the numbers, `_missing_layout` says whether the
/// Vault's Layout was read, `_failed_count` and `_failed_at` hold one line per entry that was not
/// written, and `_took_count` and `_took_at` name what each Vault handed over, a
/// `RolaWriteSyncTook *` released with `free_rola_write_sync_took`. Every `char *` is released with
/// `free_string`.
#[lazyffi(export = RolaWriteSync)]
#[derive(Debug, Clone, Default)]
pub struct WriteSync {
    /// How many entries were written to the Vault.
    applied: u64,
    /// How many versions were brought down from the Vault.
    received: u64,
    /// What was not done, one line each.
    failed: Vec<String>,
    /// How many stored objects the index named.
    index_objects: u64,
    /// How many units of stored content the run sent.
    storage_objects: u64,
    /// Whether the Vault's Layout was not read.
    missing_layout: bool,
    /// What each Vault handed over.
    took: Vec<WriteSyncTook>,
}

impl WriteSync {
    /// The answer a one-shot sync run is handed over as.
    fn of(applied: &Applied) -> Self {
        Self {
            applied: u64::try_from(applied.applied).unwrap_or(u64::MAX),
            received: u64::try_from(applied.received).unwrap_or(u64::MAX),
            failed: applied.failed.clone(),
            index_objects: u64::try_from(applied.index_objects).unwrap_or(u64::MAX),
            storage_objects: u64::try_from(applied.storage_objects).unwrap_or(u64::MAX),
            missing_layout: applied.missing_layout,
            took: applied
                .took
                .iter()
                .map(|(vault, count)| WriteSyncTook {
                    vault: vault.clone(),
                    count: u64::try_from(*count).unwrap_or(u64::MAX),
                })
                .collect(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteSync {
    /// How many entries were written to the Vault.
    #[must_use]
    pub const fn applied(&self) -> u64 {
        self.applied
    }

    /// How many versions were brought down from the Vault.
    #[must_use]
    pub const fn received(&self) -> u64 {
        self.received
    }

    /// How many entries were not written.
    #[must_use]
    pub const fn failed_count(&self) -> usize {
        self.failed.len()
    }

    /// One entry that was not written, or an empty string past the end.
    #[must_use]
    pub fn failed_at(&self, index: usize) -> String {
        self.failed.get(index).cloned().unwrap_or_default()
    }

    /// How many stored objects the index named.
    #[must_use]
    pub const fn index_objects(&self) -> u64 {
        self.index_objects
    }

    /// How many units of stored content the run sent.
    #[must_use]
    pub const fn storage_objects(&self) -> u64 {
        self.storage_objects
    }

    /// Whether the Vault's Layout was not read, so the copy still says what it said.
    #[must_use]
    pub const fn missing_layout(&self) -> bool {
        self.missing_layout
    }

    /// How many Vaults handed something over.
    #[must_use]
    pub const fn took_count(&self) -> usize {
        self.took.len()
    }

    /// One Vault that handed something over, or null past the end.
    #[must_use]
    pub fn took_at(&self, index: usize) -> Option<WriteSyncTook> {
        self.took.get(index).cloned()
    }
}

/// How many objects one Vault handed over, as a sync run reports it.
///
/// # FFI
///
/// A `RolaWriteSyncTook *` the caller releases with `free_rola_write_sync_took`. `_vault` returns
/// a `char *` the caller releases with `free_string`, and `_count` the objects it handed over.
#[lazyffi(export = RolaWriteSyncTook)]
#[derive(Debug, Clone, Default)]
pub struct WriteSyncTook {
    /// The Vault's name.
    vault: String,
    /// The objects it handed over.
    count: u64,
}

#[lazyffi(export = rola_)]
impl WriteSyncTook {
    /// The Vault's name.
    #[must_use]
    pub fn vault(&self) -> String {
        self.vault.clone()
    }

    /// How many objects it handed over.
    #[must_use]
    pub const fn objects(&self) -> u64 {
        self.count
    }
}

/// Whether a plan made now names the entries a plan was handed, in the same order and with the
/// same versions.
///
/// The order is compared: the entries are the plan's path order, and an entry added, removed,
/// reordered or moved to another version is work this run did not agree to. What a plan says about
/// an entry beyond the version it moves, such as how many entries are already up to date, is not
/// compared, since it names no write.
fn same_plan(plan: &sync::Plan, given: &GivenPlan) -> bool {
    plan.entries.len() == given.entries.len()
        && plan.entries.iter().zip(&given.entries).all(|(now, then)| {
            now.uuid == then.uuid
                && now.local_path == then.local_path
                && now.remote_path == then.remote_path
                && now.planned_path == then.planned_path
                && now.local_version == then.local_version
                && now.remote_version == then.remote_version
                && now.kind == then.kind
        })
}

impl SyncRequest {
    /// Carries out a plan a caller agreed to, once the work has been read again.
    ///
    /// The plan is read again first, because the Workspace may have changed since it was made: an
    /// entry added, removed, reordered or moved to another version is refused rather than written,
    /// so the plan a caller previewed is the plan applied. The Vault's Layout is not fetched a
    /// second time: the copy the plan was read from is the one an apply validates against, and what
    /// the Vault itself refuses is answered by the exchange that writes to it.
    ///
    /// A file changed here that the Vault is ahead on is refused before anything is written, the
    /// same refusal the command line makes, unless the request is forced. The refusal is the whole
    /// apply rather than one entry: a sync's writes depend on each other, and skipping one would
    /// leave another reading a state that is no longer there.
    ///
    /// `progress` is what the exchanges and the move report themselves to, and `cancellation` is
    /// checked before the run, after the readings, and between two of its entries.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Cancelled`] when `cancellation` was set before the run or between two
    /// of its entries, what
    /// [`SyncRequest::request_plan`](crate::write::sync::SyncRequest::request_plan) names when the
    /// parts do not resolve, [`WriteError::Precondition`] when the plan no longer matches the work
    /// or holds a blocked entry the run was not forced past, and [`WriteError::Internal`] when an
    /// exchange or a write fails.
    pub fn request_apply(
        &self,
        given: Option<&[GivenEntry]>,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Report, WriteError> {
        // A run asked to stop before it began writes nothing.
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        // A request that reads no Layout has no plan a caller could have agreed to, so there is
        // nothing to compare and the run is the index-and-store one `--no-layout` names.
        let given = if self.layout {
            given.map(|entries| GivenPlan {
                entries: entries.to_vec(),
            })
        } else {
            None
        };

        let prepared = self.prepare(progress, cancellation, false)?;
        let plan = prepared.plan.as_ref();

        if let (Some(plan), Some(given)) = (plan, given.as_ref()) {
            if !same_plan(plan, given) {
                return Err(WriteError::Precondition {
                    what: "the sync plan".to_owned(),
                    reason: "it no longer matches the work".to_owned(),
                });
            }

            if !self.forced && plan.entries.iter().any(|entry| entry.kind == Kind::Blocked) {
                return Err(WriteError::Precondition {
                    what: "the sync".to_owned(),
                    reason: "a file changed here is behind the Vault, so a pull would overwrite it"
                        .to_owned(),
                });
            }
        }

        let messages = Messages::default();
        let carry = Carry {
            workspace: &prepared.workspace,
            account: &prepared.account,
            target: &prepared.reached.address,
            layout: &prepared.layout,
            index: &prepared.index,
            store: prepared.store.as_ref(),
            candidates: &prepared.candidates,
            messages: &messages,
            runtime: &prepared.runtime,
        };
        let options = Options {
            forced: self.forced,
            concurrency: self.common.concurrency,
            skip: Skip {
                index: !self.index,
                storage: !self.storage,
                layout: !self.layout,
            },
        };

        let applied = apply(
            progress,
            plan,
            options,
            &carry,
            &prepared.reached.name,
            &prepared.account.name(),
            cancellation,
        )?;

        // A run that read no Layout still worked in one, so the Layout it is about is the one it
        // read; what it did not read is the Vault's copy, which `layout_read` says.
        Ok(Report {
            layout: plan.map_or_else(
                || sync::layout_name(&prepared.layout),
                |plan| plan.layout.clone(),
            ),
            vault: prepared.reached.name.clone(),
            applied: applied.applied,
            received: applied.received,
            up_to_date: plan.map_or(0, |plan| plan.up_to_date),
            failed: applied.failed,
            layout_read: plan.is_some(),
            index_objects: applied.index_objects,
            storage_objects: applied.storage_objects,
        })
    }
}

/// Carries out a sync plan a caller agreed to.
///
/// # FFI
///
/// `request` is a `RolaWriteSyncRequest` the caller fills, the same one `rola_write_sync_plan`
/// takes. `entries` and `entries_len` are the caller's array of the entries the plan promised to
/// move, borrowed for the call, and a length of zero lets the run make its own plan. Every
/// `const char *` is borrowed for the call and released by the caller.
///
/// A `RolaWriteSyncApply *` the caller releases with `free_rola_write_sync_apply` answers it:
/// `_layout` and `_vault` say what the run was about, `_applied`, `_received`, `_up_to_date`,
/// `_index_objects` and `_storage_objects` are the numbers, `_layout_read` says whether the Vault's
/// Layout was read, and `_failed_count` and `_failed_at` hold one line per entry that was not
/// written. `progress` is the handle the caller polls with `rola_progress_next`, and `cancellation`
/// is the flag it sets with `rola_cancellation_cancel`.
///
/// # Errors
///
/// Returns a `RolaWriteError *` the caller releases with `free_rola_write_error`, with the tag
/// [`WriteError`]'s: `Precondition` when the plan no longer matches the work, names an entry kind
/// this build does not write, or holds a blocked entry the run was not forced past, `Cancelled`
/// when `cancellation` was set, and every failure [`SyncRequest::request_apply`] names when the run
/// cannot be carried out.
#[lazyffi(export = rola_write_sync_apply)]
pub fn sync_apply_at(
    request: &SyncInput,
    entries: &[SyncAgreed],
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<WriteSyncApply, WriteError> {
    let request: SyncRequest = request.clone().into();
    let agreed: Vec<GivenEntry> = entries
        .iter()
        .map(SyncAgreed::given)
        .collect::<Result<_, _>>()?;
    let watched = crate::write::progress::watching(progress);
    let report = request.request_apply(
        (!agreed.is_empty()).then_some(agreed.as_slice()),
        &watched,
        cancellation,
    )?;

    Ok(WriteSyncApply::of(&report))
}

/// What carrying a plan out came to, as the C ABI hands it over.
///
/// It is the shape the command line's `--json` writes: what the run was about, how much each side
/// moved, and what was not done.
///
/// # FFI
///
/// A `RolaWriteSyncApply *` the caller releases with `free_rola_write_sync_apply`. `_layout` and
/// `_vault` return a `char *` the caller releases with `free_string`; `_applied`, `_received`,
/// `_up_to_date`, `_index_objects` and `_storage_objects` are the numbers; `_layout_read` says
/// whether the Vault's Layout was read; and `_failed_count` and `_failed_at` hold one line per
/// entry that was not written.
#[lazyffi(export = RolaWriteSyncApply)]
#[derive(Debug, Clone, Default)]
pub struct WriteSyncApply {
    /// The Layout the run was about.
    layout: String,
    /// The Vault it tracks.
    vault: String,
    /// How many entries were written to the Vault.
    applied: u64,
    /// How many versions were brought down from the Vault.
    received: u64,
    /// How many entries both sides already held at the same version.
    up_to_date: u64,
    /// What was not done, one line each.
    failed: Vec<String>,
    /// Whether the Vault's Layout was read at all.
    layout_read: bool,
    /// How many stored objects the index named.
    index_objects: u64,
    /// How many units of stored content the run sent.
    storage_objects: u64,
}

impl WriteSyncApply {
    /// The answer a carried-out plan is handed over as.
    fn of(report: &Report) -> Self {
        Self {
            layout: report.layout.clone(),
            vault: report.vault.clone(),
            applied: u64::try_from(report.applied).unwrap_or(u64::MAX),
            received: u64::try_from(report.received).unwrap_or(u64::MAX),
            up_to_date: u64::try_from(report.up_to_date).unwrap_or(u64::MAX),
            failed: report.failed.clone(),
            layout_read: report.layout_read,
            index_objects: u64::try_from(report.index_objects).unwrap_or(u64::MAX),
            storage_objects: u64::try_from(report.storage_objects).unwrap_or(u64::MAX),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteSyncApply {
    /// The Layout the run was about.
    #[must_use]
    pub fn layout(&self) -> String {
        self.layout.clone()
    }

    /// The Vault it tracks.
    #[must_use]
    pub fn vault(&self) -> String {
        self.vault.clone()
    }

    /// How many entries were written to the Vault.
    #[must_use]
    pub const fn applied(&self) -> u64 {
        self.applied
    }

    /// How many versions were brought down from the Vault.
    #[must_use]
    pub const fn received(&self) -> u64 {
        self.received
    }

    /// How many entries both sides already held at the same version.
    #[must_use]
    pub const fn up_to_date(&self) -> u64 {
        self.up_to_date
    }

    /// How many entries were not written.
    #[must_use]
    pub const fn failed_count(&self) -> usize {
        self.failed.len()
    }

    /// One entry that was not written, or an empty string past the end.
    #[must_use]
    pub fn failed_at(&self, index: usize) -> String {
        self.failed.get(index).cloned().unwrap_or_default()
    }

    /// Whether the Vault's Layout was read at all.
    #[must_use]
    pub const fn layout_read(&self) -> bool {
        self.layout_read
    }

    /// How many stored objects the index named.
    #[must_use]
    pub const fn index_objects(&self) -> u64 {
        self.index_objects
    }

    /// How many units of stored content the run sent.
    #[must_use]
    pub const fn storage_objects(&self) -> u64 {
        self.storage_objects
    }
}

#[cfg(test)]
mod tests {
    use rorolala_storage::{Codec, RorolalaStorage, StorageSet};
    use rorolala_tree::{Baseline, FileMode, TreeEntry, TreeStructure};
    use uuid::Uuid;

    use super::{receive_tree, sync_at};
    use crate::write::WriteError;

    #[test]
    fn a_sync_run_with_every_side_left_alone_answers_with_nothing() {
        let dir = std::env::temp_dir().join(format!("rola-write-sync-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        rorolala_workspace::Workspace::create(&dir).unwrap();

        let answer = sync_at(
            &dir,
            "vault",
            "alice",
            "missing.pem",
            "rola://127.0.0.1/",
            "both",
            false,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(answer.applied(), 0);
        assert_eq!(answer.received(), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sync_run_outside_a_workspace_is_refused() {
        let dir = std::env::temp_dir().join(format!("rola-write-sync-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let error = sync_at(
            &dir,
            "vault",
            "alice",
            "alice.pem",
            "rola://127.0.0.1/",
            "both",
            false,
            true,
            true,
            true,
        )
        .unwrap_err();
        assert!(matches!(error, WriteError::NoWorkspace));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_received_tree_is_written_as_the_directory_it_is() {
        let dir = std::env::temp_dir().join(format!("rola-write-sync-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        rorolala_workspace::Workspace::create(&dir).unwrap();

        let tree = dir.join("charts");
        std::fs::create_dir_all(tree.join(".rola-tree")).unwrap();
        std::fs::write(tree.join("gone.txt"), b"old").unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = StorageSet::sole(RorolalaStorage::create(dir.join(".rola").join("pulled")));

        let kept = runtime
            .block_on(store.write_object(b"new", Codec::Raw))
            .unwrap();
        let structure =
            TreeStructure::build(None, [TreeEntry::file("kept.txt", FileMode::Regular, kept)])
                .unwrap();

        // What the tree recorded before the pull: `gone.txt` is the path the new version no longer
        // names, and taking it away is the caller's part of writing a structure.
        let previous =
            TreeStructure::build(None, [TreeEntry::file("gone.txt", FileMode::Regular, kept)])
                .unwrap();
        Baseline::new(&tree).write(&previous, &[0; 32]).unwrap();

        let stored = runtime
            .block_on(store.write_object(&structure.encode(), Codec::Raw))
            .unwrap();

        receive_tree(
            &tree,
            &dir,
            Uuid::from_u128(7),
            &[1; 32],
            &stored,
            &store,
            &runtime,
        )
        .unwrap();

        assert_eq!(std::fs::read(tree.join("kept.txt")).unwrap(), b"new");
        assert!(!tree.join("gone.txt").exists());
        assert_eq!(Baseline::new(&tree).version(), Some([1; 32]));
        assert_eq!(
            std::fs::read_to_string(tree.join(".rola-tree/id")).unwrap(),
            Uuid::from_u128(7).to_string()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod cross_tests {
    use super::{
        Applied, Kind, Messages, Report, SyncAgreed, WriteError, WriteSync, WriteSyncApply,
        WriteSyncTook, none_if_empty,
    };

    #[test]
    fn an_agreed_entry_is_read_back_with_its_empty_names_absent() {
        let agreed = SyncAgreed {
            uuid: "u".to_owned(),
            local_path: "a".to_owned(),
            remote_path: String::new(),
            planned_path: String::new(),
            local_version: String::new(),
            remote_version: String::new(),
            kind: "create".to_owned(),
        };

        let given = agreed.given().unwrap();

        assert_eq!(given.uuid, "u");
        assert_eq!(given.local_path.as_deref(), Some("a"));
        assert_eq!(given.remote_path, None);
        assert!(matches!(given.kind, Kind::Create));

        let broken = SyncAgreed {
            kind: "not a kind".to_owned(),
            ..agreed
        };
        assert!(matches!(
            broken.given(),
            Err(WriteError::Precondition { .. })
        ));

        assert_eq!(none_if_empty(""), None);
        assert_eq!(none_if_empty("a"), Some("a".to_owned()));
    }

    #[test]
    fn a_one_shot_sync_crosses_its_counts_and_takes() {
        let applied = Applied {
            applied: 1,
            received: 2,
            failed: vec!["bad".to_owned()],
            index_objects: 3,
            storage_objects: 4,
            missing_layout: true,
            took: vec![("alpha".to_owned(), 5)],
        };

        let answer = WriteSync::of(&applied);

        assert_eq!(answer.applied(), 1);
        assert_eq!(answer.received(), 2);
        assert_eq!(answer.failed_count(), 1);
        assert_eq!(answer.failed_at(0), "bad");
        assert_eq!(answer.failed_at(1), "");
        assert_eq!(answer.index_objects(), 3);
        assert_eq!(answer.storage_objects(), 4);
        assert!(answer.missing_layout());
        assert_eq!(answer.took_count(), 1);
        assert_eq!(answer.took_at(0).unwrap().vault(), "alpha");
        assert_eq!(answer.took_at(0).unwrap().objects(), 5);
        assert!(answer.took_at(1).is_none());
    }

    #[test]
    fn an_applied_sync_crosses_what_it_did() {
        let report = Report {
            layout: "main".to_owned(),
            vault: "alpha".to_owned(),
            applied: 1,
            received: 2,
            up_to_date: 3,
            failed: vec!["bad".to_owned()],
            layout_read: true,
            index_objects: 4,
            storage_objects: 5,
        };

        let answer = WriteSyncApply::of(&report);

        assert_eq!(answer.layout(), "main");
        assert_eq!(answer.vault(), "alpha");
        assert_eq!(answer.applied(), 1);
        assert_eq!(answer.received(), 2);
        assert_eq!(answer.up_to_date(), 3);
        assert_eq!(answer.failed_count(), 1);
        assert_eq!(answer.failed_at(0), "bad");
        assert_eq!(answer.failed_at(1), "");
        assert!(answer.layout_read());
        assert_eq!(answer.index_objects(), 4);
        assert_eq!(answer.storage_objects(), 5);
    }

    #[test]
    fn a_took_answer_names_its_vault_and_count() {
        let took = WriteSyncTook {
            vault: "alpha".to_owned(),
            count: 7,
        };

        assert_eq!(took.vault(), "alpha");
        assert_eq!(took.objects(), 7);
    }

    #[test]
    fn the_default_words_are_the_ones_the_library_uses() {
        let messages = Messages::default();

        assert_eq!(messages.malformed, "the entry does not read");
        assert_eq!(messages.written, "written");
    }
}

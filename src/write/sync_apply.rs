//! Carrying a sync plan out: the Layout, the index and the content.

use std::str::FromStr as _;

use rorolala_layout::{Batch, Layout, LayoutPath};
use rorolala_protocol::VaultAddress;
use rorolala_storage::{Key, RorolalaStorage, StorageBackend as _};
use rorolala_utils_progress::{Direction as MoveDirection, Progress};
use rorolala_vcs::VCSIndex;
use uuid::Uuid;

use crate::daemon::{
    EntryWrite, RemoteWrites, action_sync_index_all_async, action_write_remote_entries,
};
use crate::fetch::Sources;
use crate::write::checkout::Checkout;
use crate::write::layout::{add_cached_entry, cached_copy, set_cached_version};
use crate::write::sync::{self, Entry, Kind};
use rorolala_utils_location::Locate as _;

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
    pub store: Option<&'a RorolalaStorage>,
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
/// Returns a message when the exchange could not be made.
pub fn apply(
    progress: &Progress,
    plan: Option<&sync::Plan>,
    options: Options,
    carry: &Carry<'_>,
    vault_name: &str,
    account_name: &str,
) -> Result<Applied, String> {
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

    let sources = sources_of(options, carry, progress)?;

    // What moves the content, which is the plan's entries or, with no plan, everything the index
    // names. Both the orders below ask for it, so what it does is stated once.
    let carry_content = || -> Result<usize, String> {
        let Some(sources) = &sources else {
            return Ok(0);
        };

        if let Some(keys) = &indexed {
            return sources.send(carry.store, keys);
        }

        let (receive, send) = content_of(&entries, options, carry);

        if let Some(store) = carry.store {
            sources.bring(store, &receive);

            bring_tree_inside(&entries, carry, store, sources);
        }

        sources.send(carry.store, &send)
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
        sync_index(carry, options, progress)?;
    } else {
        sync_index(carry, options, progress)?;
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
        if entry.kind == Kind::Receive {
            if let Some(task) = &mut down {
                let _named = task.doing(name_of(entry));
            }

            received += usize::from(receive(
                entry,
                carry,
                &mut checkout,
                &mut batch,
                &mut failed,
            )?);

            if let Some(task) = &mut down {
                task.advance_by(1);
            }

            moves.advance_by(1);
        } else {
            ups.push(entry);
        }
    }

    apply_entries(
        &ups,
        options.forced,
        carry,
        vault_name,
        account_name,
        &mut applied,
        &mut failed,
    )?;

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
    // it, which is what the reading commands go on answering from until something fetches. Saying so
    // is what keeps a reader from taking a version graph for the Vault's current options.
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
            // The exchange has one answer whichever side asks: a full sync is everything either side
            // holds, and an input names nothing, see `ActionSyncIndexAll`.
            String::new(),
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
fn content_of(entries: &[&Entry], options: Options, carry: &Carry<'_>) -> (Vec<Key>, Vec<Key>) {
    let mut receive = Vec::new();
    let mut send = Vec::new();

    for entry in entries {
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

    (receive, send)
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
    store: &RorolalaStorage,
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

        if let Err(error) = carry.runtime.block_on(store.extract_file(&stored, &target)) {
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
    let mut creates: Vec<(String, String, String, String)> = Vec::new();
    let mut create_entries: Vec<&Entry> = Vec::new();
    let mut sets: Vec<(String, String)> = Vec::new();
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

                creates.push((
                    path.to_owned(),
                    entry.uuid.clone(),
                    version.to_owned(),
                    entry.item_kind.unwrap_or("file").to_owned(),
                ));
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

                sets.push((entry.uuid.clone(), version.to_owned()));
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

    let writes = serde_json::to_string(&RemoteWrites { creates, sets })
        .map_err(|error| error.to_string())?;

    let answer = action_write_remote_entries(
        carry.workspace,
        carry.account,
        carry.target.to_string(),
        writes,
    )
    .map_err(|error| error.to_string())?;

    let outcomes: Vec<EntryWrite> =
        serde_json::from_str(&answer).map_err(|error| error.to_string())?;

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

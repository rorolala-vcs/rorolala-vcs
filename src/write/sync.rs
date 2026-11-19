//! The plan `rola sync` makes: what the Layout being worked in and the Vault it tracks each hold,
//! and what is to move between them.
//!
//! The plan is read off three things: the Layout being worked in, the copy of the Vault's Layout a
//! fetch brings here, and the index below both. A `Uuid` only one side holds is a creation or a
//! file to bring in; one both hold is compared by version, along the chain the index keeps, to say
//! which side is ahead. Ownership is read from the Vault's copy, since the Vault is where it is
//! kept, and a file changed here is read off the working tree, since that is what a pull would
//! overwrite.

use std::collections::{BTreeSet, HashMap};

use crate::auth::Account;
use crate::daemon::action_fetch_layout;
use crate::ignore::IgnoreRuleSet;
use crate::layout::{EntryKind, Layout, LayoutPath};
use crate::protocol::VaultAddress;
use crate::storage::{Blake3Hash, Key, RorolalaStorage};
use crate::vcs::{VCSIndex, VCSIndexObject, VCSIndexReadingError, Version};
use crate::workspace::Workspace;
use crate::workspace_analyze::tree_diff;
use crate::write::layout::cached_copy;
use crate::write::progress::WriteProgress;
use crate::write::request::{Common, Reached};
use crate::write::{Cancellation, WriteError};
use rorolala_errors::Failure as _;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_progress::Progress;
use serde::{Deserialize, Serialize};

/// How alike two text files have to be to count as the same file moved.
const ALIKE: f32 = 0.6;

/// The marker a remote path carries when the Vault has deprecated the file.
const REMOVED_PREFIX: &str = "@/removed/";

/// The name the Vault's Layout is to give a `Uuid` only this Layout holds.
///
/// It is the path the file is tracked under here, so the Vault's Layout names a first submission
/// where the work holds it rather than in a staging area of the Vault's own. When that name is
/// taken, the parent, the stem, the `Uuid` in short form, and the extension: `art/hero.psd` becomes
/// `art/hero@1a2b3c4.psd`. The short form grows a character at a time while even that name is taken,
/// so two files submitted under one path stay two names, the whole `Uuid` last, which no
/// `Uuid`-carrying name can share.
///
/// `None` when even the whole `Uuid` leaves the name taken, which cannot happen for a real one.
fn create_remote_path(
    local_path: &LayoutPath,
    uuid: &str,
    taken: impl Fn(&LayoutPath) -> bool,
) -> Option<String> {
    let clean = LayoutPath::new(local_path.as_str()).ok()?;
    if !taken(&clean) {
        return Some(clean.as_str().to_owned());
    }

    let (parent, file) = split_parent(local_path.as_str());
    let (stem, extension) = split_extension(file);
    let plain: String = uuid.chars().filter(|c| *c != '-').collect();

    for length in plain.len().min(7)..=plain.len() {
        let short = &plain[..length];
        let candidate = if parent.is_empty() {
            format!("{stem}@{short}{extension}")
        } else {
            format!("{parent}/{stem}@{short}{extension}")
        };

        let Ok(named) = LayoutPath::new(&candidate) else {
            continue;
        };
        if !taken(&named) {
            return Some(named.as_str().to_owned());
        }
    }

    None
}

/// `text` split into its parent and its last component; the parent is empty at the root.
fn split_parent(text: &str) -> (&str, &str) {
    text.rfind('/')
        .map_or(("", text), |at| (&text[..at], &text[at + 1..]))
}

/// A file name split at its last `.` into the stem and the extension, dot included.
///
/// `a.png` is `("a", ".png")` and `a.pkg.tar.zst` is `("a.pkg.tar", ".zst")`. A name with no dot, or
/// one whose only dot leads it, has no extension: `.gitignore` is all stem.
fn split_extension(file: &str) -> (&str, &str) {
    match file.rfind('.') {
        Some(at) if at > 0 => file.split_at(at),
        _ => (file, ""),
    }
}

/// How far a run is to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Only what this Layout holds goes up.
    Up,
    /// Only what the Vault holds comes down.
    Down,
    /// Both directions.
    #[default]
    Both,
}

/// What a plan means to do with one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Only this Layout names it, so it goes up as an entry of the Vault's Layout.
    Create,
    /// The Vault is behind and this account holds it, so the newer version goes up.
    Send,
    /// This Layout is behind, so the Vault's version comes down.
    Receive,
    /// The two moved apart, so only `--force` may send this one.
    Diverged,
    /// The Vault holds it and this account does not, so it is not sent.
    Refused,
    /// Bringing the Vault's version down would overwrite a file changed here.
    Blocked,
    /// The Vault holds a `Uuid` this Layout does not; `bring` is what brings one.
    RemoteOnly,
    /// The two name it as different things, a file and a tree, so neither side may take it.
    #[serde(rename = "kind_mismatch")]
    Mismatched,
    /// Both are at the version the Vault holds.
    UpToDate,
}

/// One entry the plan speaks about.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// The `Uuid` it is known by.
    pub uuid: String,
    /// The path this Layout names it by, when it names one.
    pub local_path: Option<String>,
    /// The path the Vault's Layout names it by, when it names one.
    pub remote_path: Option<String>,
    /// The name the Vault's Layout is to give it, when this would create one.
    pub planned_path: Option<String>,
    /// The version this Layout is at, as hex.
    pub local_version: Option<String>,
    /// The version the Vault is at, as hex.
    pub remote_version: Option<String>,
    /// The account the Vault names as its holder.
    pub owner: Option<String>,
    /// Whether the Vault has moved it under `@/removed/`.
    pub deprecated: bool,
    /// What the entry is, a file or a tree, as the Layout names it.
    pub item_kind: Option<&'static str>,
    /// Whether the file this Layout names has been changed in the tree.
    pub modified: bool,
    /// What the plan means to do with it.
    pub kind: Kind,
}

/// What a run is to do.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// The Layout being worked in.
    pub layout: String,
    /// The Vault it tracks.
    pub vault: String,
    /// Whether the run was told to change history rather than refuse.
    pub forced: bool,
    /// What is to move, in path order.
    pub entries: Vec<Entry>,
    /// How many entries both sides already hold at the same version.
    pub up_to_date: usize,
}

/// Builds the plan for `local` beside `remote`, both read against `index` and the tree under
/// `workspace`.
///
/// # Errors
///
/// Returns [`WriteError::Layout`] when the tree or a Layout cannot be read,
/// [`WriteError::Internal`] when the index cannot be read, and [`WriteError::Cancelled`] when
/// `cancellation` was set before the plan or between two of its entries.
// The plan reads through the Workspace, both Layouts, the index and the caller's flag, and its
// callers hold exactly those; grouping them into a struct here would only move the same list.
#[allow(
    clippy::too_many_arguments,
    reason = "the plan reads through the Workspace, both Layouts, the index and the caller's flag, which every caller holds side by side"
)]
pub async fn plan(
    workspace: &Workspace,
    local: &Layout,
    remote: &Layout,
    index: &VCSIndex,
    me: &str,
    direction: Direction,
    forced: bool,
    cancellation: &Cancellation,
) -> Result<Plan, WriteError> {
    // A run asked to stop before it began reads nothing: the tree and the index below are the
    // work, and doing it for a run nobody waits for is work for nothing.
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled);
    }

    // A sync moves content between entries the Layouts name; what the tree gained and nobody
    // recorded is not synced, so the ignore rules have nothing to govern here.
    let diff = tree_diff(local, workspace, ALIKE, &IgnoreRuleSet::empty()).map_err(|error| {
        WriteError::Layout {
            reason: error.to_string(),
        }
    })?;
    let modified: BTreeSet<LayoutPath> = diff.modified.into_iter().collect();

    let (mut entries, up_to_date) = local_entries(
        local,
        remote,
        &mut Relations::new(index, cancellation),
        me,
        direction,
        &modified,
        cancellation,
    )
    .await?;
    entries.extend(remote_only(local, remote, direction, cancellation)?);

    entries.sort_by(|left, right| {
        left.local_path
            .cmp(&right.local_path)
            .then_with(|| left.remote_path.cmp(&right.remote_path))
            .then_with(|| left.uuid.cmp(&right.uuid))
    });

    Ok(Plan {
        layout: layout_name(local),
        vault: String::new(),
        forced,
        entries,
        up_to_date,
    })
}

/// The name the Layout a plan is made in goes by.
pub(crate) fn layout_name(local: &Layout) -> String {
    local
        .dir()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}

/// The failure a plan reports when a read under it falls short.
const fn failed(reason: String) -> WriteError {
    WriteError::Internal { reason }
}

/// What this Layout holds, read against the Vault's copy and the index.
///
/// `modified` is the set of paths the tree has changed, so an entry the Vault is ahead on and that
/// was changed here is [`Kind::Blocked`] rather than [`Kind::Receive`]: bringing the Vault's
/// version down would overwrite the change.
///
/// # Errors
///
/// Returns [`WriteError::Internal`] when the index cannot be read, and [`WriteError::Cancelled`]
/// when `cancellation` was set between two entries.
async fn local_entries(
    local: &Layout,
    remote: &Layout,
    relations: &mut Relations<'_>,
    me: &str,
    direction: Direction,
    modified: &BTreeSet<LayoutPath>,
    cancellation: &Cancellation,
) -> Result<(Vec<Entry>, usize), WriteError> {
    let mut entries = Vec::new();
    let mut up_to_date = 0;

    for (id, data) in local.entries() {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        let local_path = local.path_of(id);
        let changed = local_path
            .as_ref()
            .is_some_and(|path| modified.contains(path));

        let Some(held) = remote.entry(id) else {
            if kept(Kind::Create, direction) {
                let uuid = id.to_string();
                let planned_path = local_path.as_ref().and_then(|path| {
                    create_remote_path(path, &uuid, |candidate| remote.id_of(candidate).is_some())
                });

                entries.push(Entry {
                    uuid,
                    local_path: local_path.map(|path| path.as_str().to_owned()),
                    remote_path: None,
                    planned_path,
                    local_version: Some(hex(data.version())),
                    remote_version: None,
                    owner: None,
                    deprecated: false,
                    item_kind: Some(kind_name(data.kind())),
                    modified: changed,
                    kind: Kind::Create,
                });
            }

            continue;
        };

        let remote_path = remote.path_of(id);
        let deprecated = remote_path
            .as_ref()
            .is_some_and(|path| path.as_str().starts_with(REMOVED_PREFIX));

        let kind = if data.version() == held.version() {
            Kind::UpToDate
        } else {
            let relation = relations.relation(&data.version(), &held.version()).await?;
            decide(&data.version(), &held.version(), held.owner(), me, relation)
        };
        let kind = match kind {
            Kind::Receive if changed => Kind::Blocked,
            other => other,
        };

        // A path the two name as different things is not one either side may take: a file and a tree
        // are not two shapes of one thing, and a transfer would write one over the other.
        let kind = if data.kind() == held.kind() {
            kind
        } else {
            Kind::Mismatched
        };

        if kind == Kind::UpToDate {
            up_to_date += 1;

            continue;
        }
        if !kept(kind, direction) {
            continue;
        }

        entries.push(Entry {
            uuid: id.to_string(),
            local_path: local_path.map(|path| path.as_str().to_owned()),
            remote_path: remote_path.map(|path| path.as_str().to_owned()),
            planned_path: None,
            local_version: Some(hex(data.version())),
            remote_version: Some(hex(held.version())),
            owner: held.owner().map(str::to_owned),
            deprecated,
            item_kind: Some(kind_name(data.kind())),
            modified: changed,
            kind,
        });
    }

    Ok((entries, up_to_date))
}

/// The name of what a Layout entry is, as a plan says it.
const fn kind_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::File => "file",
        EntryKind::Tree => "tree",
    }
}

/// What the Vault holds and this Layout does not.
///
/// It is not the sync's to bring, a `Uuid` enters a Layout by `bring`, so these are named
/// rather than planned.
///
/// # Errors
///
/// Returns [`WriteError::Cancelled`] when `cancellation` was set between two entries.
fn remote_only(
    local: &Layout,
    remote: &Layout,
    direction: Direction,
    cancellation: &Cancellation,
) -> Result<Vec<Entry>, WriteError> {
    if !kept(Kind::RemoteOnly, direction) {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for (id, data) in remote.entries() {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        if local.entry(id).is_some() {
            continue;
        }

        let remote_path = remote.path_of(id);
        let deprecated = remote_path
            .as_ref()
            .is_some_and(|path| path.as_str().starts_with(REMOVED_PREFIX));

        entries.push(Entry {
            uuid: id.to_string(),
            local_path: None,
            remote_path: remote_path.map(|path| path.as_str().to_owned()),
            planned_path: None,
            local_version: None,
            remote_version: Some(hex(data.version())),
            owner: data.owner().map(str::to_owned),
            deprecated,
            item_kind: Some(kind_name(data.kind())),
            modified: false,
            kind: Kind::RemoteOnly,
        });
    }

    Ok(entries)
}

/// Whether a plan for `direction` names this kind of action.
const fn kept(kind: Kind, direction: Direction) -> bool {
    match direction {
        Direction::Both => true,
        Direction::Up => matches!(
            kind,
            Kind::Create | Kind::Send | Kind::Diverged | Kind::Refused | Kind::Mismatched
        ),
        Direction::Down => matches!(
            kind,
            Kind::Receive | Kind::Blocked | Kind::RemoteOnly | Kind::Mismatched
        ),
    }
}

/// What to do with one `Uuid` both Layouts hold, once the versions are known.
fn decide(
    local_version: &[u8; 32],
    remote_version: &[u8; 32],
    owner: Option<&str>,
    me: &str,
    relation: Relation,
) -> Kind {
    if local_version == remote_version {
        return Kind::UpToDate;
    }

    match relation {
        Relation::LocalAhead => {
            if owner == Some(me) {
                Kind::Send
            } else {
                Kind::Refused
            }
        }
        Relation::RemoteAhead => Kind::Receive,
        Relation::Apart => {
            if owner == Some(me) {
                Kind::Diverged
            } else {
                Kind::Refused
            }
        }
    }
}

/// How the version this Layout is at stands beside the Vault's.
#[derive(Clone, Copy)]
enum Relation {
    /// The Vault's version is on the chain below this one.
    LocalAhead,
    /// This version is on the chain below the Vault's, or the Vault's is not here to compare.
    RemoteAhead,
    /// Neither is below the other.
    Apart,
}

/// The relations between versions, read once and remembered for the whole plan.
///
/// A plan compares every entry both sides hold, and the chains those entries lie on share their
/// ancestors: a version read to walk one entry's chain lies below the next entry's as well. What
/// each read found is kept here, so a plan costs one index read per version rather than one per step
/// of every walk.
struct Relations<'a> {
    /// The index the versions are read from.
    index: &'a VCSIndex,

    /// The flag the caller sets to stop the walk.
    cancellation: &'a Cancellation,

    /// What the index holds under a version digest, or nothing when it holds none.
    versions: HashMap<Blake3Hash, Option<Version>>,

    /// The version below a version, its variant's base, or nothing when it is the root.
    parents: HashMap<Blake3Hash, Option<Blake3Hash>>,

    /// Whether one version is on the chain below another, by the pair of digests.
    below: HashMap<(Blake3Hash, Blake3Hash), bool>,
}

impl<'a> Relations<'a> {
    /// Reads relations against `index`, remembering nothing yet.
    fn new(index: &'a VCSIndex, cancellation: &'a Cancellation) -> Self {
        Self {
            index,
            cancellation,
            versions: HashMap::new(),
            parents: HashMap::new(),
            below: HashMap::new(),
        }
    }

    /// The version the index holds under `digest`, or nothing when it holds none.
    async fn version(&mut self, digest: &Blake3Hash) -> Result<Option<Version>, String> {
        if let Some(held) = self.versions.get(digest) {
            return Ok(held.clone());
        }

        let read = version(self.index, digest).await?;
        self.versions.insert(*digest, read.clone());

        Ok(read)
    }

    /// The version below `digest`, or nothing when it is the root or holds no version.
    async fn parent(&mut self, digest: &Blake3Hash) -> Result<Option<Blake3Hash>, String> {
        if let Some(held) = self.parents.get(digest) {
            return Ok(*held);
        }

        let parent = match self.version(digest).await? {
            Some(version) if !version.is_root() => Some(
                *read_variant(self.index, version.variant())
                    .await?
                    .base_version(),
            ),
            _ => None,
        };
        self.parents.insert(*digest, parent);

        Ok(parent)
    }

    /// Whether `ancestor` is on the chain below `descendant`.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Internal`] when the index cannot be read, and
    /// [`WriteError::Cancelled`] when `cancellation` was set between two steps of the walk.
    async fn descends(
        &mut self,
        descendant: &Blake3Hash,
        ancestor: &Blake3Hash,
    ) -> Result<bool, WriteError> {
        let key = (*descendant, *ancestor);
        if let Some(held) = self.below.get(&key) {
            return Ok(*held);
        }

        let mut at = *descendant;
        let below = loop {
            if self.cancellation.is_cancelled() {
                return Err(WriteError::Cancelled);
            }
            if at == *ancestor {
                break true;
            }
            match self.parent(&at).await.map_err(failed)? {
                Some(parent) => at = parent,
                None => break false,
            }
        };
        self.below.insert(key, below);

        Ok(below)
    }

    /// How the version this Layout is at stands beside the Vault's.
    ///
    /// A version the index does not hold is one this side never had, so it can only have come from
    /// elsewhere: the Vault is ahead. Both being here, the chain below each is walked to see whether
    /// the other is on it.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Internal`] when the index cannot be read, and
    /// [`WriteError::Cancelled`] when `cancellation` was set between two steps of the walk.
    async fn relation(
        &mut self,
        local_version: &Blake3Hash,
        remote_version: &Blake3Hash,
    ) -> Result<Relation, WriteError> {
        if self.version(local_version).await.map_err(failed)?.is_none() {
            return Ok(Relation::Apart);
        }
        if self
            .version(remote_version)
            .await
            .map_err(failed)?
            .is_none()
        {
            return Ok(Relation::RemoteAhead);
        }

        if self.descends(local_version, remote_version).await? {
            return Ok(Relation::LocalAhead);
        }
        if self.descends(remote_version, local_version).await? {
            return Ok(Relation::RemoteAhead);
        }

        Ok(Relation::Apart)
    }
}

/// The version the index holds under `digest`, or nothing when it holds none.
async fn version(index: &VCSIndex, digest: &[u8; 32]) -> Result<Option<Version>, String> {
    match index.read(Key::new(*digest)).await {
        Ok(VCSIndexObject::Version(version)) => Ok(Some(version)),
        Ok(_) | Err(VCSIndexReadingError::NotFound { .. }) => Ok(None),
        Err(error) => Err(error.reason()),
    }
}

/// The store key the version `digest` names, when the index holds the chain to it.
///
/// It is how a version is turned back into content: the version names a variant, and the variant
/// names what was stored. A version the index does not hold names nothing.
///
/// # Errors
///
/// Returns a message when the index could not be read.
pub async fn store_of(index: &VCSIndex, digest: &[u8; 32]) -> Result<Option<Key>, String> {
    let Some(version) = version(index, digest).await? else {
        return Ok(None);
    };

    match index.read(Key::new(*version.variant())).await {
        Ok(VCSIndexObject::Variant(variant)) => Ok(Some(Key::new(*variant.storage_hash()))),
        Ok(_) | Err(VCSIndexReadingError::NotFound { .. }) => Ok(None),
        Err(error) => Err(error.reason()),
    }
}

/// Every store key the chain from `head` back to its root names, head first.
///
/// A version is only a meaning once what it stored is in the store that names it, so what a run has
/// to carry for a version is not one content but the whole chain the version stands on. A run that
/// named only the top of a chain left every version below it a name in the Layout with no content
/// behind it: the version crossed, nothing it stored did, and nothing said so.
///
/// A tree version's storage key is its structure, and the structure names the description and every
/// file's content. `expand_all` asks for those on every version of the chain, which is what an upload
/// needs: a version below the head is exactly what a new Workspace reaches for when it moves a pointer
/// back. A pull expands only the head, because a version below it can ask for its inside when it is
/// reached, and fetching every tree's history to move one pointer would carry work nobody asked for.
///
/// A version the index does not hold ends the walk, a chain the index cannot read is one it cannot
/// name content for, and guessing past it would name content no version is known to want.
///
/// # Errors
///
/// Returns a message when the index could not be read.
pub async fn chain_store_keys(
    index: &VCSIndex,
    head: &[u8; 32],
    store: Option<&crate::storage::RorolalaStorage>,
    expand_all: bool,
) -> Result<Vec<Key>, String> {
    let mut keys = Vec::new();
    let mut seen = BTreeSet::new();
    let mut at = *head;

    while seen.insert(at) {
        if let Some(key) = store_of(index, &at).await? {
            keys.push(key);

            if (expand_all || at == *head)
                && let Some(store) = store
                && let Ok(bytes) = store.read_object(&key).await
                && let Ok(structure) = crate::tree::TreeStructure::decode(&bytes)
            {
                for inside in structure.content_keys() {
                    if seen.insert(*inside.digest()) {
                        keys.push(inside);
                    }
                }
            }
        }

        let Some(base) = base_of(index, &at).await? else {
            break;
        };
        if base == [0_u8; 32] {
            break;
        }

        at = base;
    }

    Ok(keys)
}

/// What a tree version holds inside it: the description and every file's content.
///
/// The version's storage key is its structure, and the structure names what the version's files
/// hold. A pull reads the structure after the first exchange and asks for these in a second: the
/// structure is what says which keys the inside is made of.
///
/// # Errors
///
/// Returns a message when the version's chain cannot be read.
pub async fn tree_internal_keys(
    index: &VCSIndex,
    store: &crate::storage::RorolalaStorage,
    version: &[u8; 32],
) -> Result<Vec<Key>, String> {
    let Some(structure_key) = store_of(index, version).await? else {
        return Ok(Vec::new());
    };

    let Ok(bytes) = store.read_object(&structure_key).await else {
        return Ok(Vec::new());
    };
    let Ok(structure) = crate::tree::TreeStructure::decode(&bytes) else {
        return Ok(Vec::new());
    };

    Ok(structure.content_keys())
}

/// The version `digest` stands on, when the index holds the chain to it.
async fn base_of(index: &VCSIndex, digest: &[u8; 32]) -> Result<Option<Blake3Hash>, String> {
    let Some(version) = version(index, digest).await? else {
        return Ok(None);
    };

    match index.read(Key::new(*version.variant())).await {
        Ok(VCSIndexObject::Variant(variant)) => Ok(Some(*variant.base_version())),
        Ok(_) | Err(VCSIndexReadingError::NotFound { .. }) => Ok(None),
        Err(error) => Err(error.reason()),
    }
}

/// The variant the index holds under `digest`.
async fn read_variant(
    index: &VCSIndex,
    digest: &Blake3Hash,
) -> Result<crate::vcs::Variant, String> {
    match index.read(Key::new(*digest)).await {
        Ok(VCSIndexObject::Variant(variant)) => Ok(variant),
        Ok(_) => Err(format!(
            "the index holds no variant {}",
            Key::new(*digest).hex()
        )),
        Err(error) => Err(error.reason()),
    }
}

/// A version as the hex a listing prints.
fn hex(digest: [u8; 32]) -> String {
    Key::new(digest).hex()
}

/// A sync asked for as one value, so a command line and a C caller fill the same request.
///
/// [`Common`] comes flattened, so the workspace, the account, the Vault, the offline flag, the
/// concurrency and the labels are the fields every write request carries. `direction`, `forced`,
/// `storage`, `index` and `layout` are the sync's own choices, and the defaults are the ones a run
/// that said nothing makes: both directions, a run that may not change history, and every side
/// moving.
///
/// `forced` is the sync's own gate. The shared `force` field is not read here, because what a sync
/// forces is a version whose chain moved apart rather than the gates the other workflows open.
// The struct is the command line's flags as a document, and each one is a switch a caller sets:
// folding them into a state machine would make the wire shape a program has to read harder, not
// easier, and the command line's own result struct is spelled the same way.
#[allow(
    clippy::struct_excessive_bools,
    reason = "the request is the command's flags as a document, and each is a switch its caller sets"
)]
#[derive(Debug, Deserialize)]
pub struct SyncRequest {
    /// The fields every write request carries.
    #[serde(flatten)]
    pub(crate) common: Common,
    /// How far the run goes.
    #[serde(default)]
    pub(crate) direction: Direction,
    /// Whether a version whose chain moved apart may go up anyway.
    #[serde(default)]
    pub(crate) forced: bool,
    /// Whether the store moves.
    #[serde(default = "yes")]
    pub(crate) storage: bool,
    /// Whether the index moves.
    #[serde(default = "yes")]
    pub(crate) index: bool,
    /// Whether the Vault's Layout is read and written.
    #[serde(default = "yes")]
    pub(crate) layout: bool,
}

/// The default a side that a run may leave alone has.
const fn yes() -> bool {
    true
}

/// What a sync plan says, in the shape the command line's `--json` writes it.
///
/// It is the plan [`Plan`] carries, read together with the two sides the request may leave behind:
/// a plan names what is to move, and `index` and `storage` say whether the run that made it moves
/// the index and the store as well. `layout_read` is always true here, since a request that leaves
/// the Vault's Layout alone has no plan to answer with.
// The plan's shape is the command line's `--json` plan, where each side a run may leave alone is
// its own field; the four switches are the answer's contract, not a state a caller walks through.
#[allow(
    clippy::struct_excessive_bools,
    reason = "the plan is the command line's JSON plan, where each side a run may leave alone is its own field"
)]
#[derive(Debug, Clone, Serialize)]
pub struct SyncPlan {
    /// The Layout being worked in.
    pub layout: String,
    /// The Vault it tracks.
    pub vault: String,
    /// Whether the run was told to change history rather than refuse.
    pub forced: bool,
    /// What is to move, in path order.
    pub entries: Vec<Entry>,
    /// How many entries both sides already hold at the same version.
    pub up_to_date: usize,
    /// Whether the Vault's Layout was read.
    pub layout_read: bool,
    /// Whether the index is to move.
    pub index: bool,
    /// Whether the store is to move.
    pub storage: bool,
}

/// What a request resolved to, and the plan it is about.
pub(crate) struct Prepared {
    /// The Workspace being worked in.
    pub(crate) workspace: Workspace,
    /// The Layout being worked in.
    pub(crate) layout: Layout,
    /// The Vault the request reaches for.
    pub(crate) reached: Reached,
    /// The account the run acts as.
    pub(crate) account: Account,
    /// Every Vault the Workspace knows, the one the run reaches for first.
    pub(crate) candidates: Vec<(String, VaultAddress)>,
    /// The index below both sides.
    pub(crate) index: VCSIndex,
    /// The store the content is written from, when the Workspace keeps one.
    pub(crate) store: Option<RorolalaStorage>,
    /// The runtime the asynchronous exchanges are waited on.
    pub(crate) runtime: tokio::runtime::Runtime,
    /// The plan, or nothing when the request leaves the Vault's Layout alone.
    pub(crate) plan: Option<Plan>,
}

impl SyncRequest {
    /// Builds the plan the request is about.
    ///
    /// The order is the one a sync takes whichever caller asked for it: the Workspace, the Layout,
    /// the Vault, the account and the index are resolved, the Vault's Layout is fetched unless the
    /// run is offline, and the plan is read against the copy the fetch left. What the plan is made
    /// from is what the Vault holds now rather than a copy that may have gone stale, which is the
    /// reason the fetch is not the caller's step.
    ///
    /// `progress` is what the fetch says itself to, and `cancellation` is checked before the run
    /// reaches a Vault, after the fetch, and between two entries of the plan.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Precondition`] when the request leaves the Vault's Layout alone, since
    /// such a run has no plan to make, and when the Vault is named only by an address, since its
    /// Layout copy is kept under its name. Returns [`WriteError::NoWorkspace`],
    /// [`WriteError::NoLayout`], [`WriteError::NoTrack`], [`WriteError::NoAccount`] or
    /// [`WriteError::NoIndex`] when a part is missing, [`WriteError::Action`] when the fetch fails,
    /// [`WriteError::NotCached`] when no copy of the Vault's Layout is here, and
    /// [`WriteError::Layout`] or [`WriteError::Internal`] when the plan cannot be made.
    /// [`WriteError::Cancelled`] when `cancellation` was set before the run or between two of its
    /// steps.
    pub fn request_plan(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<SyncPlan, WriteError> {
        if !self.layout {
            return Err(WriteError::Precondition {
                what: "the sync plan".to_owned(),
                reason: "a run that leaves the Vault's Layout alone has none to make".to_owned(),
            });
        }

        let prepared = self.prepare(progress, cancellation, true)?;
        let Some(plan) = prepared.plan else {
            return Err(WriteError::Internal {
                reason: "the sync plan was not made".to_owned(),
            });
        };

        Ok(SyncPlan {
            layout: plan.layout,
            vault: plan.vault,
            forced: plan.forced,
            entries: plan.entries,
            up_to_date: plan.up_to_date,
            layout_read: true,
            index: self.index,
            storage: self.storage,
        })
    }

    /// Resolves every part a sync works through, and makes the plan when the request has one.
    ///
    /// A plan and an apply both begin here rather than each reading the parts its own way: an apply
    /// re-reads them because the Workspace may have changed since the plan was made, and a second
    /// reading written apart would be a second set of rules to keep right.
    ///
    /// `fetch` is what a plan asks for and an apply does not: an apply compares the plan it was
    /// handed with the work as it is now, and the copy the plan was just read from is new enough
    /// for that.
    ///
    /// # Errors
    ///
    /// Returns what [`request_plan`](Self::request_plan) names, without the refusal about a request
    /// that leaves the Vault's Layout alone: `plan` is `None` for such a request rather than a
    /// failure, so an apply can carry it out.
    pub(crate) fn prepare(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
        fetch: bool,
    ) -> Result<Prepared, WriteError> {
        // A run asked to stop before it began reaches nothing and reads nothing.
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        let workspace = self.common.workspace()?;
        let layout = self.common.layout(&workspace)?;
        let reached = self.common.vault(&workspace)?;

        // The copy a plan is read against is kept under the Vault's name, so an address alone
        // names no copy and there would be nothing to read.
        if reached.name.is_empty() {
            return Err(WriteError::Precondition {
                what: "the Vault the sync reaches for".to_owned(),
                reason: "its Layout is kept under the Vault's name, and an address is not one"
                    .to_owned(),
            });
        }

        let account = self.common.account(&workspace)?;
        let candidates = self.common.candidates(&workspace)?;
        let index = self.common.index(&workspace)?;
        let store = self.common.store(&workspace).ok();
        let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        let plan = if self.layout {
            // The fetch and the plan are the two steps a reader watches here, and the bar opens
            // before the exchange rather than after it, so it is on screen while the slow half is.
            let mut step = (fetch && !self.common.offline)
                .then(|| progress.begin(self.common.labels.fetching.clone(), Some(2)));

            if let Some(step) = &mut step {
                action_fetch_layout(
                    &workspace,
                    &account,
                    reached.address.to_string(),
                    reached.name.clone(),
                )
                .map_err(|error| WriteError::Action {
                    reason: error.to_string(),
                })?;
                step.advance_by(1);
            }

            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled);
            }

            // The copy is the one the fetch left. A run that fetched nothing reads what is here,
            // and one that has none is told so rather than planned against nothing.
            let copy = cached_copy(&workspace, &reached.name)
                .map_err(|error| WriteError::Layout {
                    reason: error.to_string(),
                })?
                .ok_or(WriteError::NotCached)?;

            let mut planned = runtime.block_on(plan(
                &workspace,
                &layout,
                &copy,
                &index,
                &account.name(),
                self.direction,
                self.forced,
                cancellation,
            ))?;
            planned.vault.clone_from(&reached.name);

            if let Some(step) = &mut step {
                step.advance_by(1);
            }

            Some(planned)
        } else {
            None
        };

        Ok(Prepared {
            workspace,
            layout,
            reached,
            account,
            candidates,
            index,
            store,
            runtime,
            plan,
        })
    }
}

/// Builds a sync plan from a JSON request.
///
/// # FFI
///
/// `request` is a JSON object of the shape [`SyncRequest`] reads: the fields every write request
/// carries (`workspace`, `account`, `target`, `offline`, `force`, `concurrency` and `labels`) at
/// the top level, and `direction` (`up`, `down` or `both`), `forced`, `storage`, `index` and
/// `layout` when they are needed. The answer is an owned JSON string the caller releases with
/// `free_string`:
/// `{"layout":"main","vault":"default","forced":false,"entries":[...],"up_to_date":0,"layout_read":true,"index":true,"storage":true}`.
/// An entry is
/// `{"uuid":"...","local_path":"...","remote_path":"...","planned_path":null,"local_version":"...","remote_version":"...","owner":"...","deprecated":false,"item_kind":"file","modified":false,"kind":"send"}`.
/// `progress` is the handle the caller polls with `rola_progress_next_json`, and `cancellation` is
/// the flag it sets with `rola_cancellation_cancel`.
///
/// # Errors
///
/// Returns a `RolaWriteError *` the caller releases with `free_rola_write_error`, with the tag
/// [`WriteError`]'s: `Json` when `request` does not read, and every failure
/// [`SyncRequest::request_plan`] names when the plan cannot be made.
#[lazyffi(export = rola_write_sync_plan)]
pub fn sync_plan_at(
    request: &str,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<String, WriteError> {
    let request: SyncRequest = serde_json::from_str(request).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })?;
    let watched = crate::write::progress::watching(progress);
    let plan = request.request_plan(&watched, cancellation)?;

    serde_json::to_string(&plan).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use crate::layout::LayoutPath;
    use crate::storage::{Codec, Key, RorolalaStorage};
    use crate::tree::{FileMode, TreeEntry, TreeStructure};
    use crate::vcs::{VCSIndex, Variant, Version};
    use crate::write::WriteError;
    use rorolala_workspace::Workspace;

    use super::{
        Direction, Kind, Relation, chain_store_keys, create_remote_path, decide, kept,
        split_extension,
    };

    const UUID: &str = "1a2b3c4d-0000-0000-0000-000000000000";

    /// An index of its own, in a directory of its own that is emptied first.
    fn index(label: &str) -> VCSIndex {
        let path =
            std::env::temp_dir().join(format!("rola-sync-chain-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);

        VCSIndex::create(path)
    }

    /// Writes `variant` and the version made of it down, and answers both.
    fn record(index: &VCSIndex, variant: &Variant) -> (String, Version) {
        let version = variant.new_version();

        index
            .write_variant(variant)
            .expect("the variant is written");
        let hex = index
            .write_version(&version)
            .expect("the version is written");

        (hex, version)
    }

    /// The digest a version is named by, read back out of its hex.
    fn digest(hex: &str) -> [u8; 32] {
        *Key::from_str(hex)
            .expect("a version reads as a key")
            .digest()
    }

    /// Every version a chain stands on names content, not just the version at the top: what a run
    /// carries for a version is the whole chain, because a version below it is what a pointer moved
    /// back reaches for.
    #[test]
    fn a_chain_names_the_content_of_every_version_it_stands_on() {
        let index = index("every");
        let first = [1_u8; 32];
        let second = [2_u8; 32];
        let third = [3_u8; 32];

        let (_, one) = record(
            &index,
            &Version::root().new_variant(first, [0; 32], [0; 32]),
        );
        let (_, two) = record(&index, &one.new_variant(second, [0; 32], [0; 32]));
        let (three, _) = record(&index, &two.new_variant(third, [0; 32], [0; 32]));

        let keys = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(chain_store_keys(&index, &digest(&three), None, false))
            .expect("the index reads");

        assert_eq!(
            keys,
            vec![Key::new(third), Key::new(second), Key::new(first)],
            "the version, then every version it stands on"
        );
    }

    /// A chain that is one version long names that version's content and stops at the root.
    #[test]
    fn a_chain_of_one_stops_at_the_root() {
        let index = index("one");
        let only = [7_u8; 32];

        let (one, _) = record(&index, &Version::root().new_variant(only, [0; 32], [0; 32]));

        let keys = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(chain_store_keys(&index, &digest(&one), None, false))
            .expect("the index reads");

        assert_eq!(keys, vec![Key::new(only)]);
    }

    /// A tree version names its files through its structure. An upload carries every version of the
    /// chain, so every structure's files are carried too; a Workspace that moves a pointer back to an
    /// older version must find what that version held and not only its structure.
    #[test]
    fn a_chain_expands_the_inside_of_every_tree_version_it_stands_on() {
        let store_dir =
            std::env::temp_dir().join(format!("rola-sync-tree-chain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store_dir);

        let store = RorolalaStorage::create(&store_dir);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let index = index("tree-chain");

        let first = tree_version(&runtime, &store, [1_u8; 32]);
        let second = tree_version(&runtime, &store, [2_u8; 32]);
        let third = tree_version(&runtime, &store, [3_u8; 32]);

        let (_, one) = record(
            &index,
            &Version::root().new_variant(first, [0; 32], [0; 32]),
        );
        let (_, two) = record(&index, &one.new_variant(second, [0; 32], [0; 32]));
        let (three, _) = record(&index, &two.new_variant(third, [0; 32], [0; 32]));

        let keys = runtime
            .block_on(chain_store_keys(
                &index,
                &digest(&three),
                Some(&store),
                true,
            ))
            .expect("the index reads");

        for seed in [1_u8, 2, 3] {
            assert!(
                keys.contains(&Key::new([seed; 32])),
                "the content of a tree version below the head is carried"
            );
        }
    }

    /// A tree structure naming one file, written into the store, and the key it is stored under.
    fn tree_version(
        runtime: &tokio::runtime::Runtime,
        store: &RorolalaStorage,
        content: [u8; 32],
    ) -> [u8; 32] {
        let structure = TreeStructure::build(
            None,
            [TreeEntry::file(
                "a.psd",
                FileMode::Regular,
                Key::new(content),
            )],
        )
        .expect("a tree structure");

        let key = runtime
            .block_on(store.write_object(&structure.encode(), Codec::Raw))
            .expect("the structure is written");

        *key.digest()
    }

    #[test]
    fn a_new_file_is_named_by_the_path_it_is_tracked_at() {
        let free = |_: &LayoutPath| false;

        assert_eq!(
            create_remote_path(&LayoutPath::new("art/hero.psd").unwrap(), UUID, free).as_deref(),
            Some("art/hero.psd")
        );
    }

    #[test]
    fn a_taken_path_takes_the_short_uuid_before_the_extension() {
        let taken = |candidate: &LayoutPath| candidate.as_str() == "art/hero.psd";

        assert_eq!(
            create_remote_path(&LayoutPath::new("art/hero.psd").unwrap(), UUID, taken).as_deref(),
            Some("art/hero@1a2b3c4.psd")
        );

        // A dotted name keeps everything before the last dot as the stem: `a.pkg.tar.zst` is
        // `a.pkg.tar` and `.zst`.
        let taken = |candidate: &LayoutPath| candidate.as_str() == "a.pkg.tar.zst";

        assert_eq!(
            create_remote_path(&LayoutPath::new("a.pkg.tar.zst").unwrap(), UUID, taken).as_deref(),
            Some("a.pkg.tar@1a2b3c4.zst")
        );
    }

    #[test]
    fn a_taken_short_uuid_grows_a_character_at_a_time() {
        let taken =
            |candidate: &LayoutPath| matches!(candidate.as_str(), "hero.psd" | "hero@1a2b3c4.psd");

        assert_eq!(
            create_remote_path(&LayoutPath::new("hero.psd").unwrap(), UUID, taken).as_deref(),
            Some("hero@1a2b3c4d.psd")
        );
    }

    #[test]
    fn an_extension_is_what_follows_the_last_dot() {
        assert_eq!(split_extension("a.png"), ("a", ".png"));
        assert_eq!(split_extension("a.pkg.tar.zst"), ("a.pkg.tar", ".zst"));
        assert_eq!(split_extension("a"), ("a", ""));
        assert_eq!(split_extension(".gitignore"), (".gitignore", ""));
    }

    #[test]
    fn which_side_is_ahead_decides_what_an_entry_is() {
        let local = [1; 32];
        let remote = [2; 32];

        // The same version is nothing to do, whatever either side is at.
        assert_eq!(
            decide(&local, &local, Some("alice"), "alice", Relation::Apart),
            Kind::UpToDate
        );

        // Ahead or apart, only the holder may send; behind, anyone may take it down.
        assert_eq!(
            decide(
                &local,
                &remote,
                Some("alice"),
                "alice",
                Relation::LocalAhead
            ),
            Kind::Send
        );
        assert_eq!(
            decide(&local, &remote, Some("bob"), "alice", Relation::LocalAhead),
            Kind::Refused
        );
        assert_eq!(
            decide(&local, &remote, Some("bob"), "alice", Relation::RemoteAhead),
            Kind::Receive
        );
        assert_eq!(
            decide(&local, &remote, Some("alice"), "alice", Relation::Apart),
            Kind::Diverged
        );
        assert_eq!(
            decide(&local, &remote, Some("bob"), "alice", Relation::Apart),
            Kind::Refused
        );
    }

    #[test]
    fn a_sync_request_without_a_vault_is_refused_and_a_cancelled_one_stops() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-sync-request-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();

        let text = serde_json::json!({"workspace": dir.display().to_string()}).to_string();
        let request: super::SyncRequest = serde_json::from_str(&text).unwrap();

        // A request that said nothing moves every side and both directions.
        assert_eq!(request.direction, Direction::Both);
        assert!(request.storage && request.index && request.layout);
        assert!(!request.forced);

        let progress = rorolala_utils_progress::Progress::silent();
        let live = crate::write::Cancellation::new();
        let error = request.request_plan(&progress, &live).unwrap_err();
        assert!(matches!(error, WriteError::NoTrack));

        // The apply half reads the same parts, so a Workspace with no Vault fails it the same way.
        let error = request.request_apply("{}", &progress, &live).unwrap_err();
        assert!(matches!(error, WriteError::NoTrack));

        // A run asked to stop before it began stops before it reads anything, whichever half asked.
        let cancelled = crate::write::Cancellation::new();
        cancelled.cancel();
        let error = request.request_plan(&progress, &cancelled).unwrap_err();
        assert!(matches!(error, WriteError::Cancelled));
        let error = request
            .request_apply("{}", &progress, &cancelled)
            .unwrap_err();
        assert!(matches!(error, WriteError::Cancelled));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A Workspace that knows a Vault can plan against the copy it has and apply the plan it made:
    /// the two halves read the same parts, so the plan a caller previewed is the one carried out.
    #[test]
    fn a_sync_plan_and_its_apply_read_the_same_parts() {
        use rorolala_utils_configure::Configure as _;
        use rorolala_utils_location::Locate as _;

        let dir =
            std::env::temp_dir().join(format!("rola-write-sync-round-trip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        // The Vault a request names has to be one the Workspace bound, and the key it signs as has
        // to be one its scopes hold.
        let mut config = rorolala_workspace::Config::read_from(&workspace.config_path()).unwrap();
        config.vaults_mut().bind("vault", "rola://127.0.0.1/");
        config.write_to(&workspace.config_path()).unwrap();
        let keys = dir.join(".rola").join("auth");
        std::fs::create_dir_all(&keys).unwrap();
        std::fs::write(keys.join("alice.pem"), "a key this test never signs with").unwrap();

        // A fetch is what leaves the copy a plan is read against, so a copy is made here: the plan
        // then reads the shape a run that fetched would have left.
        let copy =
            workspace.readonly_layouts_dir("vault", rorolala_utils_constants::VAULT_LAYOUT_NAME);
        std::fs::create_dir_all(&copy).unwrap();
        rorolala_layout::Layout::open(&copy).unwrap();

        let text = serde_json::json!({
            "workspace": dir.display().to_string(),
            "account": "alice",
            "target": {"kind": "named", "name": "vault"},
            // The copy is the one made above, so the plan is read from it rather than fetched; a
            // run that fetched would need a Vault answering.
            "offline": true,
            "storage": false,
            "index": false,
        })
        .to_string();
        let request: super::SyncRequest = serde_json::from_str(&text).unwrap();

        let progress = rorolala_utils_progress::Progress::silent();
        let live = crate::write::Cancellation::new();
        let plan = request.request_plan(&progress, &live).unwrap();

        assert_eq!(plan.vault, "vault");
        assert_eq!(
            plan.layout,
            workspace
                .layout()
                .unwrap()
                .dir()
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap()
        );
        assert!(plan.layout_read);
        assert!(!plan.index && !plan.storage);
        assert!(plan.entries.is_empty());

        let json = serde_json::to_string(&plan).unwrap();
        let report = request.request_apply(&json, &progress, &live).unwrap();

        assert_eq!(report.layout, plan.layout);
        assert_eq!(report.vault, "vault");
        assert!(report.layout_read);
        assert_eq!(report.applied, 0);
        assert_eq!(report.received, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_direction_keeps_only_the_actions_it_names() {
        for up in [Kind::Create, Kind::Send, Kind::Diverged, Kind::Refused] {
            assert!(kept(up, Direction::Up), "{up:?}");
            assert!(!kept(up, Direction::Down), "{up:?}");
        }

        for down in [Kind::Receive, Kind::Blocked, Kind::RemoteOnly] {
            assert!(kept(down, Direction::Down), "{down:?}");
            assert!(!kept(down, Direction::Up), "{down:?}");
        }

        for any in [Kind::Create, Kind::Send, Kind::Receive, Kind::Diverged] {
            assert!(kept(any, Direction::Both), "{any:?}");
        }
    }
}

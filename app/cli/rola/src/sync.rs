//! The plan `rola sync` makes: what the Layout being worked in and the Vault it tracks each hold,
//! and what is to move between them.
//!
//! The plan is read off three things: the Layout being worked in, the copy of the Vault's Layout a
//! fetch brings here, and the index below both. A `Uuid` only one side holds is a creation or a
//! file to check in; one both hold is compared by version, along the chain the index keeps, to say
//! which side is ahead. Ownership is read from the Vault's copy, since the Vault is where it is
//! kept, and a file changed here is read off the working tree, since that is what a pull would
//! overwrite.

use std::collections::BTreeSet;

use librorolala::layout::{Layout, LayoutPath};
use librorolala::storage::{Blake3Hash, Key};
use librorolala::tree_analyze::tree_diff;
use librorolala::vcs::{VCSIndex, VCSIndexObject, VCSIndexReadingError, VCSWrite as _, Version};
use librorolala::workspace::Workspace;
use rorolala_errors::Failure as _;
use serde::Serialize;

/// How alike two text files have to be to count as the same file moved.
const ALIKE: f32 = 0.6;

/// The marker a remote path carries when the Vault has deprecated the file.
const REMOVED_PREFIX: &str = "#/removed/";

/// The short form of a `Uuid` that a new file's remote name carries.
///
/// The name keeps the path it was submitted under and adds this, so two files submitted under one
/// path are still two names in the Vault's Layout.
#[must_use]
pub fn short_name(id: &str) -> String {
    let plain: String = id.chars().filter(|c| *c != '-').collect();

    plain[..plain.len().min(7)].to_owned()
}

/// How far a run is to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Only what this Layout holds goes up.
    Up,
    /// Only what the Vault holds comes down.
    Down,
    /// Both directions.
    Both,
}

/// What a plan means to do with one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Only this Layout names it, so it goes up to the Vault's `#/new/`.
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
    /// The Vault holds a `Uuid` this Layout does not; `checkin` is what brings one.
    RemoteOnly,
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
    /// Whether the Vault has moved it under `#/removed/`.
    pub deprecated: bool,
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
/// Returns a message when the tree, the Layout, or the index could not be read.
pub async fn plan(
    workspace: &Workspace,
    local: &Layout,
    remote: &Layout,
    index: &VCSIndex,
    me: &str,
    direction: Direction,
    forced: bool,
) -> Result<Plan, String> {
    let diff = tree_diff(local, workspace, ALIKE).map_err(|error| error.to_string())?;
    let modified: BTreeSet<LayoutPath> = diff.modified.into_iter().collect();

    let (mut entries, up_to_date) =
        local_entries(local, remote, index, me, direction, &modified).await?;
    entries.extend(remote_only(local, remote, direction));

    entries.sort_by(|left, right| {
        left.local_path
            .cmp(&right.local_path)
            .then_with(|| left.remote_path.cmp(&right.remote_path))
            .then_with(|| left.uuid.cmp(&right.uuid))
    });

    Ok(Plan {
        layout: local
            .dir()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        vault: String::new(),
        forced,
        entries,
        up_to_date,
    })
}

/// What this Layout holds, read against the Vault's copy and the index.
///
/// `modified` is the set of paths the tree has changed, so an entry the Vault is ahead on and that
/// was changed here is [`Kind::Blocked`] rather than [`Kind::Receive`]: bringing the Vault's
/// version down would overwrite the change.
async fn local_entries(
    local: &Layout,
    remote: &Layout,
    index: &VCSIndex,
    me: &str,
    direction: Direction,
    modified: &BTreeSet<LayoutPath>,
) -> Result<(Vec<Entry>, usize), String> {
    let mut entries = Vec::new();
    let mut up_to_date = 0;

    for (id, data) in local.entries() {
        let local_path = local.path_of(id);
        let changed = local_path
            .as_ref()
            .is_some_and(|path| modified.contains(path));

        let Some(held) = remote.entry(id) else {
            if kept(Kind::Create, direction) {
                let uuid = id.to_string();
                let planned_path = local_path
                    .as_ref()
                    .map(|path| format!("#/new/{}@{}", path.as_str(), short_name(&uuid)));

                entries.push(Entry {
                    uuid,
                    local_path: local_path.map(|path| path.as_str().to_owned()),
                    remote_path: None,
                    planned_path,
                    local_version: Some(hex(data.version())),
                    remote_version: None,
                    owner: None,
                    deprecated: false,
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
            let relation = relation(index, &data.version(), &held.version()).await?;
            decide(&data.version(), &held.version(), held.owner(), me, relation)
        };
        let kind = match kind {
            Kind::Receive if changed => Kind::Blocked,
            other => other,
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
            modified: changed,
            kind,
        });
    }

    Ok((entries, up_to_date))
}

/// What the Vault holds and this Layout does not.
///
/// It is not the sync's to bring — a `Uuid` enters a Layout by `checkin` — so these are named
/// rather than planned.
fn remote_only(local: &Layout, remote: &Layout, direction: Direction) -> Vec<Entry> {
    if !kept(Kind::RemoteOnly, direction) {
        return Vec::new();
    }

    let mut entries = Vec::new();

    for (id, data) in remote.entries() {
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
            modified: false,
            kind: Kind::RemoteOnly,
        });
    }

    entries
}

/// Whether a plan for `direction` names this kind of action.
fn kept(kind: Kind, direction: Direction) -> bool {
    match direction {
        Direction::Both => true,
        Direction::Up => matches!(
            kind,
            Kind::Create | Kind::Send | Kind::Diverged | Kind::Refused
        ),
        Direction::Down => matches!(kind, Kind::Receive | Kind::Blocked | Kind::RemoteOnly),
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

/// How the version this Layout is at stands beside the Vault's, read along the index.
///
/// A version the index does not hold is one this side never had, so it can only have come from
/// elsewhere: the Vault is ahead. Both being here, the chain below each is walked to see whether
/// the other is on it.
async fn relation(
    index: &VCSIndex,
    local_version: &[u8; 32],
    remote_version: &[u8; 32],
) -> Result<Relation, String> {
    let Some(local) = version(index, local_version).await? else {
        return Ok(Relation::Apart);
    };
    let Some(remote) = version(index, remote_version).await? else {
        return Ok(Relation::RemoteAhead);
    };

    if descends(index, &local, remote_version).await? {
        return Ok(Relation::LocalAhead);
    }
    if descends(index, &remote, local_version).await? {
        return Ok(Relation::RemoteAhead);
    }

    Ok(Relation::Apart)
}

/// Whether the version `ancestor` is on the chain below `descendant`.
async fn descends(
    index: &VCSIndex,
    descendant: &Version,
    ancestor: &[u8; 32],
) -> Result<bool, String> {
    let mut current = descendant.clone();

    loop {
        if current.hash().digest() == ancestor {
            return Ok(true);
        }
        if current.is_root() {
            return Ok(false);
        }

        current = read_version(
            index,
            read_variant(index, current.variant()).await?.base_version(),
        )
        .await?;
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

/// The version the index holds under `digest`.
async fn read_version(index: &VCSIndex, digest: &Blake3Hash) -> Result<Version, String> {
    version(index, digest).await?.map_or_else(
        || {
            Err(format!(
                "the index holds no version {}",
                Key::new(*digest).hex()
            ))
        },
        Ok,
    )
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

/// The variant the index holds under `digest`.
async fn read_variant(
    index: &VCSIndex,
    digest: &Blake3Hash,
) -> Result<librorolala::vcs::Variant, String> {
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

#[cfg(test)]
mod tests {
    use super::{Direction, Kind, Relation, decide, kept, short_name};

    #[test]
    fn a_new_file_is_named_by_its_path_and_a_short_uuid() {
        assert_eq!(
            short_name("1a2b3c4d-0000-0000-0000-000000000000"),
            "1a2b3c4"
        );
        assert_eq!(short_name("ab"), "ab");
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

//! How the version this side records for an entry stands beside the one upstream records.
//!
//! An entry's Layout records a version, and the fetched copy of the Vault's Layout records one too. The
//! two are the same while the work is in step, and differ as soon as either side has moved: this side
//! having recorded something the Vault has not been told of is *ahead*, which is ordinary work; the
//! Vault having moved on without this side is *behind*, and two sides that have each moved on are
//! *apart*. The last two are what makes a base wrong — a change made on one of them is a change made
//! against a version the other side is not at — which is why they are told apart from being ahead.
//!
//! Telling them apart costs reading the chain: the index holds the variants and versions to walk, and a
//! version it does not hold is one this side never had, so how far apart the two are is not known here.

use rorolala_layout::{Layout, LayoutPath};
use rorolala_storage::{Blake3Hash, Key};
use rorolala_vcs::{VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version};
use uuid::Uuid;

/// Where a version stands beside another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// The two are the same version.
    Same,

    /// This side is further on: it holds work the other has not been told of.
    Ahead,

    /// The other side is further on, so what is based here is based on a version that has moved.
    Behind,

    /// Both sides have moved on, so neither is based on the other.
    Apart,

    /// One of the versions is not in the index, so how they stand is not known here.
    Unknown,
}

/// How one version stands beside another, and how many versions apart they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base {
    /// Where the two stand.
    pub relation: Relation,

    /// How many versions apart they are, when the chain says; nothing when it does not.
    pub distance: Option<u64>,
}

/// How the version `local` stands beside `upstream`.
#[must_use]
pub fn compare(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    local: &Blake3Hash,
    upstream: &Blake3Hash,
) -> Base {
    if local == upstream {
        return Base {
            relation: Relation::Same,
            distance: Some(0),
        };
    }

    // A version the index does not hold is one this side never had, so how far apart the two are is not
    // known here: the other side has moved on and the chain that says how far is not local.
    let (Some(local), Some(upstream)) = (
        read_version(index, runtime, local),
        read_version(index, runtime, upstream),
    ) else {
        return Base {
            relation: Relation::Unknown,
            distance: None,
        };
    };

    if let Some(distance) = distance_to(index, runtime, &local, upstream.hash().digest()) {
        return Base {
            relation: Relation::Ahead,
            distance: Some(distance),
        };
    }

    if let Some(distance) = distance_to(index, runtime, &upstream, local.hash().digest()) {
        return Base {
            relation: Relation::Behind,
            distance: Some(distance),
        };
    }

    Base {
        relation: Relation::Apart,
        distance: None,
    }
}

/// How one path's version stands beside the one upstream records, and how many versions apart they are.
///
/// It is [`compare`]'s answer named for what a reader of a *path* asks, which is not what a reader of two
/// versions asks: a path whose version is the same has nothing to say about itself, and one whose standing
/// is not known here is not the same as one that is in step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    /// Where the two stand.
    pub relation: Relation,

    /// How many versions apart they are, when the chain says; nothing when it does not.
    pub distance: Option<u64>,
}

/// How every version this Layout records stands beside the one upstream records for the same entry.
///
/// A path this side records and upstream does not name is left out: there is no version to be beside, and
/// nothing there for a record to be based on. So is one either side names without a version, which is an
/// entry that was never recorded rather than one that is behind.
///
/// This is the whole of what [`wrong_base`] is a filter of, and it is read the same way — one walk of the
/// chain per path, which is what the command line already pays for every path it draws.
#[must_use]
pub fn every(
    local: &Layout,
    copy: &Layout,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
) -> Vec<(LayoutPath, Standing)> {
    local
        .paths()
        .into_iter()
        .filter_map(|(path, id)| {
            let (Some(local), Some(upstream)) = (version_of(local, id), version_of(copy, id)) else {
                return None;
            };

            let base = compare(index, runtime, &local, &upstream);

            Some((
                path,
                Standing {
                    relation: base.relation,
                    distance: base.distance,
                },
            ))
        })
        .collect()
}

/// The paths among `paths` whose version stands behind or apart from the one upstream records.
///
/// These are the changes made against a version the other side is not at: what is recorded here would be
/// based on something the Vault does not have. A path upstream does not name — one it was never told of —
/// is not one of them: nothing there is in the way, and there is no version to be based on.
///
/// A path whose version is *ahead* is not one of them either: this side holding work the other has not
/// been told of is what working looks like, not a base gone wrong.
#[must_use]
pub fn wrong_base(
    local: &Layout,
    copy: &Layout,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    paths: &[LayoutPath],
) -> Vec<String> {
    wrong(local, copy, index, runtime, paths)
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect()
}

/// The same, as the paths themselves rather than as their names.
///
/// Split out because [`Status::read`](crate::Status) wants the paths and the command line wants the names:
/// answering one with the other would be a second copy of the walk, and the walk is the expensive half.
#[must_use]
pub fn wrong(
    local: &Layout,
    copy: &Layout,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    paths: &[LayoutPath],
) -> Vec<LayoutPath> {
    paths
        .iter()
        .filter(|path| {
            let Some(id) = local.id_of(path) else {
                return false;
            };
            let (Some(local), Some(upstream)) = (version_of(local, id), version_of(copy, id))
            else {
                return false;
            };

            matches!(
                compare(index, runtime, &local, &upstream).relation,
                Relation::Behind | Relation::Apart
            )
        })
        .cloned()
        .collect()
}

/// The version the Layout records for an entry, when it records one.
fn version_of(layout: &Layout, id: Uuid) -> Option<Blake3Hash> {
    let version = layout.entry(id)?.version();

    (version != [0; 32]).then_some(version)
}

/// How many versions below `descendant` the version `ancestor` is, when it is below it at all.
fn distance_to(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    descendant: &Version,
    ancestor: &Blake3Hash,
) -> Option<u64> {
    let mut current = descendant.clone();
    let mut steps = 0;

    while !current.is_root() {
        let variant = read_variant(index, runtime, current.variant())?;
        let base = read_version(index, runtime, variant.base_version())?;

        steps += 1;
        if base.hash().digest() == ancestor {
            return Some(steps);
        }

        current = base;
    }

    None
}

/// The version the index holds under `hash`, when it holds one.
fn read_version(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &Blake3Hash,
) -> Option<Version> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Version(version)) => Some(version),
        _ => None,
    }
}

/// The variant the index holds under `hash`, when it holds one.
pub fn read_variant(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &Blake3Hash,
) -> Option<Variant> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Variant(variant)) => Some(variant),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use rorolala_storage::Blake3Hash;
    use rorolala_vcs::VCSIndex;

    use super::{Base, Relation, compare};

    /// The runtime the chain is walked on.
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().expect("a runtime for the walk")
    }

    /// An index with nothing in it, which is what a version nothing holds is read against.
    fn empty() -> VCSIndex {
        VCSIndex::default()
    }

    #[test]
    fn a_version_is_the_same_as_itself() {
        let hash: Blake3Hash = [1; 32];

        assert_eq!(
            compare(&empty(), &runtime(), &hash, &hash),
            Base {
                relation: Relation::Same,
                distance: Some(0),
            }
        );
    }

    /// The two chains that say ahead from behind are the index's, so a version it does not hold is one
    /// this side never had: how the two stand is not known, which is not the same as being wrong.
    #[test]
    fn a_version_the_index_does_not_hold_is_not_known() {
        let local: Blake3Hash = [1; 32];
        let upstream: Blake3Hash = [2; 32];

        assert_eq!(
            compare(&empty(), &runtime(), &local, &upstream),
            Base {
                relation: Relation::Unknown,
                distance: None,
            }
        );
    }
}

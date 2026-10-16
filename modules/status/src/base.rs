//! How the version this side records for an entry stands beside the one upstream records.
//!
//! An entry's Layout records a version, and the fetched copy of the Vault's Layout records one too. The
//! two are the same while the work is in step, and differ as soon as either side has moved: this side
//! having recorded something the Vault has not been told of is *ahead*, which is ordinary work; the
//! Vault having moved on without this side is *behind*, and two sides that have each moved on are
//! *apart*. The last two are what makes a base wrong — a change made on one of them is a change made
//! against a version the other side is not at — which is why they are told apart from being ahead.
//!
//! Telling them apart costs reading the chain: the index holds the variants and versions to walk. What a
//! fetch brings is the Vault's Layout and never its versions, so being behind is ordinarily answered
//! without a distance — the direction is known from the version having come from elsewhere, and the chain
//! that would count the versions between is the Vault's own. See [`compare`] for the whole of it.

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
///
/// # Being behind
///
/// A fetch brings the Vault's *Layout* here and never its versions, so the version upstream records is
/// ordinarily **not** in the index this reads. That is not a reading that cannot answer: a version this side
/// never had can only have come from elsewhere — the Vault is ahead of this side — and it is answered
/// [`Relation::Behind`] with no distance, because the chain that would measure the gap is the Vault's own and
/// is not here. What is *not* answered that way is a version this side holds and cannot place either: see
/// [`Relation::Unknown`].
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

    let here = read_version(index, runtime, local);

    // Ahead is asked first, and it walks this side's own chain: the other version is compared by its hash
    // alone, which the Layout records whether or not the version itself was ever fetched here. So being ahead
    // is answered without the Vault's chain, which is the one thing a fetch does not bring.
    if let Some(distance) = here
        .as_ref()
        .and_then(|held| distance_to(index, runtime, held, upstream))
    {
        return Base {
            relation: Relation::Ahead,
            distance: Some(distance),
        };
    }

    // What is left to walk is the other side's chain, which starts at a version that has to be here.
    let Some(remote) = read_version(index, runtime, upstream) else {
        // It is not here, and the chain that would say how far apart the two are is the Vault's own. What is
        // known is the direction: the Vault holds a version this side never had (see the note above).
        return Base {
            relation: Relation::Behind,
            distance: None,
        };
    };

    if let Some(distance) = distance_to(index, runtime, &remote, local) {
        return Base {
            relation: Relation::Behind,
            distance: Some(distance),
        };
    }

    // Both versions are here and neither is below the other, so the two sides have moved on from each other —
    // unless this side's own version is the one that cannot be read, which is a reading that cannot place
    // what it recorded and says nothing rather than calling the two chains separate.
    if here.is_none() {
        return Base {
            relation: Relation::Unknown,
            distance: None,
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
            let (Some(local), Some(upstream)) = (version_of(local, id), version_of(copy, id))
            else {
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
/// based on something the Vault does not have. That includes the ordinary case of being behind — the Vault
/// moved on and this side never fetched the version it is at, which is a base gone wrong whether or not the
/// chain that would measure the gap is here (see [`compare`]). A path upstream does not name — one it was
/// never told of — is not one of them: nothing there is in the way, and there is no version to be based on.
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
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::Blake3Hash;
    use rorolala_vcs::{VCSIndex, VCSWrite as _, Version};

    use super::{Base, Relation, compare};

    /// The runtime the chain is walked on.
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().expect("a runtime for the walk")
    }

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-status-base-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);

        dir
    }

    /// An index with nothing in it, which is what a version nothing holds is read against.
    fn empty() -> VCSIndex {
        VCSIndex::default()
    }

    /// An index holding a chain of `many` versions, oldest first, with this side at the newest.
    ///
    /// Every version and the variant that fixes it are written, since a walk reads both; the root is
    /// written too, because a chain is walked down to it.
    fn chain(label: &str, many: u8) -> (PathBuf, VCSIndex, Vec<Version>, tokio::runtime::Runtime) {
        let runtime = runtime();
        let dir = scratch(label);
        let index = VCSIndex::create(&dir);
        let mut versions = vec![Version::root()];

        runtime.block_on(index.write(Version::root())).unwrap();

        for step in 0..many {
            // UNWRAP: the chain always holds the version the last step made, which is what the first
            // pass is given and every pass after it appends.
            let base = versions.last().unwrap();
            let variant = base.new_variant([step.wrapping_add(1); 32], [0; 32], [0; 32]);
            let version = variant.new_version();

            runtime.block_on(index.write(variant)).unwrap();
            runtime.block_on(index.write(version.clone())).unwrap();

            versions.push(version);
        }

        (dir, index, versions, runtime)
    }

    /// A version of a chain of its own, based on the same root as the one beside it.
    ///
    /// It is what two sides that have each moved on look like: neither is below the other, and both are
    /// here to be walked.
    fn branch(label: &str, seed: u8) -> (PathBuf, VCSIndex, Version, tokio::runtime::Runtime) {
        let runtime = runtime();
        let dir = scratch(label);
        let index = VCSIndex::create(&dir);

        runtime.block_on(index.write(Version::root())).unwrap();

        let variant = Version::root().new_variant([seed; 32], [0; 32], [0; 32]);
        let version = variant.new_version();

        runtime.block_on(index.write(variant)).unwrap();
        runtime.block_on(index.write(version.clone())).unwrap();

        (dir, index, version, runtime)
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

    /// This side holding work the other has not been told of is ahead, and how far is counted along the
    /// chain the index holds — which is this side's own.
    #[test]
    fn work_the_other_side_has_not_been_told_of_is_ahead() {
        let (dir, index, versions, runtime) = chain("ahead", 3);

        assert_eq!(
            compare(
                &index,
                &runtime,
                versions[3].hash().digest(),
                versions[1].hash().digest()
            ),
            Base {
                relation: Relation::Ahead,
                distance: Some(2),
            }
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// The Vault having moved on is behind, and how far is counted along the Vault's chain — which is
    /// walked when the version it starts at is here.
    #[test]
    fn the_other_side_having_moved_on_is_behind() {
        let (dir, index, versions, runtime) = chain("behind", 3);

        assert_eq!(
            compare(
                &index,
                &runtime,
                versions[1].hash().digest(),
                versions[3].hash().digest()
            ),
            Base {
                relation: Relation::Behind,
                distance: Some(2),
            }
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A version the index does not hold is one this side never had, so it can only have come from
    /// elsewhere: the other side is ahead of this one, whatever the chain that would count the gap is.
    ///
    /// This is the ordinary shape of being behind, because a fetch brings the Vault's Layout and never its
    /// versions: what is checked here is the answer a reading that cannot walk the other chain gives.
    #[test]
    fn a_version_only_the_other_side_has_is_behind_with_no_distance() {
        let (dir, index, versions, runtime) = chain("behind-elsewhere", 1);
        let elsewhere: Blake3Hash = [0xE1; 32];

        assert_eq!(
            compare(&index, &runtime, versions[1].hash().digest(), &elsewhere),
            Base {
                relation: Relation::Behind,
                distance: None,
            }
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// Two sides that have each moved on from the same version are apart: neither is based on the other,
    /// which is not the same as one of them being ahead.
    #[test]
    fn two_sides_that_moved_on_from_one_version_are_apart() {
        let (dir, index, ours, runtime) = branch("apart", 0x11);

        // The other side's branch, in the same index, so both versions are here to be walked.
        let theirs_variant = Version::root().new_variant([0x22; 32], [0; 32], [0; 32]);
        let theirs = theirs_variant.new_version();

        runtime.block_on(index.write(theirs_variant)).unwrap();
        runtime.block_on(index.write(theirs.clone())).unwrap();

        assert_eq!(
            compare(
                &index,
                &runtime,
                ours.hash().digest(),
                theirs.hash().digest()
            ),
            Base {
                relation: Relation::Apart,
                distance: None,
            }
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A version this side cannot read either is a reading that cannot place what it recorded: it says
    /// nothing about the two rather than calling them separate chains.
    #[test]
    fn a_version_this_side_cannot_place_is_not_known() {
        let (dir, index, versions, runtime) = chain("unknown", 1);
        let ours: Blake3Hash = [0xD1; 32];

        assert_eq!(
            compare(&index, &runtime, &ours, versions[1].hash().digest()),
            Base {
                relation: Relation::Unknown,
                distance: None,
            }
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

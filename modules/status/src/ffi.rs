//! How a reading of the tree crosses into C.
//!
//! C has no list of its own, so every list in a [`Status`] crosses as a count and one entry at a
//! time by index. A caller reads the count first: past it the reading has nothing to give, which is
//! answered as an empty string, a false, or [`BrokenKind::None`] rather than by inventing an entry.
//! That is why an empty answer here means "no such entry" and never "an entry that is empty": a
//! path a Layout names is never empty, and neither is the hex a variant is named by.

use std::path::Path;

use rorolala_utils_lazyffi::lazyffi;

use crate::{Broken, Merge, PathState, Rename, Status};
use crate::base::Relation;

/// Why a merge in progress cannot go on, as C reads it.
///
/// # FFI
///
/// One variant per reason, and [`BrokenKind::None`] for the reading having no such entry. What the
/// reason is about — the variant, or the path its file lies at — is answered beside it by
/// `rola_status_broken_what`.
#[lazyffi(export = RolaStatusBroken)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokenKind {
    /// There is no such entry: the reading has fewer merges that cannot go on than were asked for.
    None,

    /// The file the variant is for is no longer named by the Layout.
    NoTarget,

    /// Nowhere here can hold its variant file.
    NoPlace,

    /// The variant file is gone from where the record says it lies.
    FileGone,

    /// This reading has no index to read the variant from.
    NoIndex,

    /// The index does not hold that variant.
    NoVariant,

    /// The variant file could not be read.
    Unreadable,

    /// The variant file no longer holds the variant's content.
    Changed,
}

impl BrokenKind {
    /// The kind of reason `broken` is.
    ///
    /// The match is exhaustive on purpose: a reason added to [`Broken`] will not compile until it is
    /// named here, so the two cannot come apart.
    const fn of(broken: &Broken) -> Self {
        match broken {
            Broken::NoTarget { .. } => Self::NoTarget,
            Broken::NoPlace { .. } => Self::NoPlace,
            Broken::FileGone { .. } => Self::FileGone,
            Broken::NoIndex { .. } => Self::NoIndex,
            Broken::NoVariant { .. } => Self::NoVariant,
            Broken::Unreadable { .. } => Self::Unreadable,
            Broken::Changed { .. } => Self::Changed,
        }
    }
}

/// How the version recorded for a path stands beside the one upstream records, as C reads it.
///
/// # FFI
///
/// One variant per standing, which is [`PathState`]'s shape and not its subject: a path's *state* is what
/// is wrong with it, and its *standing* is how far it is from what the Vault has. A path may be in either
/// without the other — a file that is in step and a file that was never recorded are both nothing to act
/// on — so the two are asked separately.
///
/// The distance is not part of this by value: it is asked for by `rola_status_standing_distance_of`, which
/// answers how many versions apart the two are, or a negative number when nothing says. See that function
/// for what a caller has to ask in what order.
#[lazyffi(export = RolaStanding)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The two are the same version, so there is nothing between them and nothing to tell.
    Same,

    /// This side records work the Vault has not been told of.
    Ahead,

    /// The Vault has moved on, so what is here is based on a version that has.
    ///
    /// This is also the answer when the version the Vault records is not in the index — which is the ordinary
    /// case, since a fetch brings the Vault's Layout and never its versions. The direction is known from the
    /// version having come from elsewhere; how far is not, and is asked for by
    /// `rola_status_standing_distance_of`, which answers nothing for it.
    Behind,

    /// Both sides have moved on, so neither is based on the other.
    Apart,

    /// How the two stand is not known here: the version this side records is not in the index, or the reading
    /// holds no standing for the path at all. A caller that must tell those apart asks the distance: it is zero
    /// for the two being the same, and nothing here is.
    Unknown,
}

impl Standing {
    /// The standing `relation` is.
    const fn of(relation: Relation) -> Self {
        match relation {
            Relation::Same => Self::Same,
            Relation::Ahead => Self::Ahead,
            Relation::Behind => Self::Behind,
            Relation::Apart => Self::Apart,
            Relation::Unknown => Self::Unknown,
        }
    }
}

#[lazyffi(export = rola_status_)]
impl Status {
    /// How many paths the Layout names that the tree does not hold.
    #[must_use]
    #[lazyffi(export = rola_status_lost_count)]
    pub const fn lost_count(&self) -> usize {
        self.lost.len()
    }

    /// The path at `index` among what the Layout names and the tree does not hold.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_lost_at)]
    pub fn lost_at(&self, index: usize) -> String {
        at(&self.lost, index)
    }

    /// How many paths the tree holds that the Layout does not name.
    #[must_use]
    #[lazyffi(export = rola_status_untagged_count)]
    pub const fn untagged_count(&self) -> usize {
        self.untagged.len()
    }

    /// The path at `index` among what the tree holds and the Layout does not name.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_untagged_at)]
    pub fn untagged_at(&self, index: usize) -> String {
        at(&self.untagged, index)
    }

    /// How many paths changed where they were.
    ///
    /// What is counted is everything modified that did not move, and never a path another account
    /// holds: a reading that took those as its own work would be reading past the one thing it is
    /// not to do.
    #[must_use]
    #[lazyffi(export = rola_status_modified_count)]
    pub const fn modified_count(&self) -> usize {
        self.modified.len()
    }

    /// The path at `index` among what changed where it was.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_modified_at)]
    pub fn modified_at(&self, index: usize) -> String {
        at(&self.modified, index)
    }

    /// How many changed paths are based on a version upstream has moved past.
    ///
    /// What is counted is what changed while the version recorded for it stands behind or apart from the
    /// one the Vault's fetched copy records — so what is recorded here would be based on something the
    /// other side does not have. A version this side is *ahead* on is not one of them.
    #[must_use]
    #[lazyffi(export = rola_status_wrong_base_count)]
    pub const fn wrong_base_count(&self) -> usize {
        self.wrong_base.len()
    }

    /// The path at `index` among the ones based on a version upstream has moved past.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_wrong_base_at)]
    pub fn wrong_base_at(&self, index: usize) -> String {
        at(&self.wrong_base, index)
    }

    /// How many paths the fetched copy of the Vault's Layout says another account holds.
    #[must_use]
    #[lazyffi(export = rola_status_unowned_count)]
    pub const fn unowned_count(&self) -> usize {
        self.unowned.len()
    }

    /// The path at `index` among the ones another account holds.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_unowned_at)]
    pub fn unowned_at(&self, index: usize) -> String {
        at(&self.unowned, index)
    }

    /// How many paths moved.
    #[must_use]
    #[lazyffi(export = rola_status_renamed_count)]
    pub const fn renamed_count(&self) -> usize {
        self.renamed.len()
    }

    /// The path the move at `index` came from.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_renamed_from)]
    pub fn renamed_from(&self, index: usize) -> String {
        rename(self, index).map_or_else(String::new, |rename| rename.from.clone())
    }

    /// The path the move at `index` went to.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_renamed_to)]
    pub fn renamed_to(&self, index: usize) -> String {
        rename(self, index).map_or_else(String::new, |rename| rename.to.clone())
    }

    /// Whether the move at `index` was edited as well as moved.
    ///
    /// # FFI
    ///
    /// False when there is no such entry.
    #[must_use]
    #[lazyffi(export = rola_status_renamed_modified)]
    pub fn renamed_modified(&self, index: usize) -> bool {
        rename(self, index).is_some_and(|rename| rename.modified)
    }

    /// Whether the move at `index` is one the two being the same bytes makes rather than one only
    /// their likeness suggests.
    ///
    /// # FFI
    ///
    /// False when there is no such entry.
    #[must_use]
    #[lazyffi(export = rola_status_renamed_strong)]
    pub fn renamed_strong(&self, index: usize) -> bool {
        rename(self, index).is_some_and(|rename| rename.strong)
    }

    /// How many variants are waiting to be joined into files.
    #[must_use]
    #[lazyffi(export = rola_status_merging_count)]
    pub const fn merging_count(&self) -> usize {
        self.merging.len()
    }

    /// The file the variant at `index` is to be joined into.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_merging_target)]
    pub fn merging_target(&self, index: usize) -> String {
        merge(self, index).map_or_else(String::new, |merge| merge.target.clone())
    }

    /// The variant at `index`, as the hex it is named by.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_merging_variant)]
    pub fn merging_variant(&self, index: usize) -> String {
        merge(self, index).map_or_else(String::new, |merge| merge.variant.clone())
    }

    /// The path the file of the variant at `index` lies at, or lay at before a move.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_merging_path)]
    pub fn merging_path(&self, index: usize) -> String {
        merge(self, index).map_or_else(String::new, |merge| merge.path.clone())
    }

    /// Where the tree holds the file of the variant at `index` now, when a move was found.
    ///
    /// # FFI
    ///
    /// An empty string when the file did not move, or when there is no such entry. The caller owns
    /// the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_merging_moved)]
    pub fn merging_moved(&self, index: usize) -> String {
        merge(self, index)
            .and_then(|merge| merge.moved.clone())
            .unwrap_or_default()
    }

    /// How many merges in progress cannot go on.
    #[must_use]
    #[lazyffi(export = rola_status_broken_count)]
    pub const fn broken_count(&self) -> usize {
        self.broken.len()
    }

    /// Why the merge at `index` cannot go on.
    ///
    /// # FFI
    ///
    /// [`BrokenKind::None`] when there is no such entry.
    #[must_use]
    #[lazyffi(export = rola_status_broken_kind)]
    pub fn broken_kind(&self, index: usize) -> BrokenKind {
        self.broken
            .get(index)
            .map_or(BrokenKind::None, BrokenKind::of)
    }

    /// What the reason at `index` is about: the variant, or the path its file lies at.
    ///
    /// # FFI
    ///
    /// An empty string when there is no such entry. The caller owns the string and releases it with
    /// `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_broken_what)]
    pub fn broken_what(&self, index: usize) -> String {
        self.broken
            .get(index)
            .map_or_else(String::new, |broken| broken.what().to_owned())
    }

    /// The file the merge at `index` cannot go on is for, when the Layout still names one.
    ///
    /// # FFI
    ///
    /// An empty string when the Layout names no such file, or when there is no such entry. The caller
    /// owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_broken_target)]
    pub fn broken_target(&self, index: usize) -> String {
        self.broken
            .get(index)
            .map_or_else(String::new, |broken| broken.target().to_owned())
    }

    /// Where the Workspace the reading was made of is rooted, which the paths it reports are relative to.
    ///
    /// # FFI
    ///
    /// The root as a path. The caller owns the string and releases it with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_status_root)]
    pub fn root_string(&self) -> String {
        self.root().to_string_lossy().into_owned()
    }

    /// What the reading says about the entry at a path a caller names.
    ///
    /// # FFI
    ///
    /// A [`PathState`] by value, which is `Unknown` for a path this reading is not about.
    #[must_use]
    #[lazyffi(export = rola_status_state_of)]
    pub fn state(&self, path: &Path) -> PathState {
        self.state_of(path)
    }

    /// How the version recorded for a path stands beside the one upstream records.
    ///
    /// # FFI
    ///
    /// A [`Standing`] by value, which is `Unknown` for a path this reading holds no standing for: one it is
    /// not about, one nothing was ever recorded for, and one upstream does not name. `Same` and `Unknown`
    /// are told apart by `rola_status_standing_distance_of`, which is zero for the first and negative for
    /// the second.
    #[must_use]
    #[lazyffi(export = rola_status_standing_of)]
    pub fn standing(&self, path: &Path) -> Standing {
        Standing::of(self.standing_of(path).0)
    }

    /// How many versions apart the path named is from the one upstream records.
    ///
    /// # FFI
    ///
    /// A signed number rather than an `Option`, because C has no `Option` and this is one number: a negative
    /// answer is "nothing says how far", and zero is "they are the same version", which is the one distance
    /// that is not a count of anything. A caller wanting to tell "in step" from "not known" reads this
    /// rather than the standing, since both of those are answered `Unknown` above — in step because there is
    /// nothing to act on, and not known because the chain is not here.
    #[must_use]
    #[lazyffi(export = rola_status_standing_distance_of)]
    pub fn standing_distance(&self, path: &Path) -> i64 {
        // WRAP: a chain longer than `i64` can count is longer than any workspace holds, and a number that
        // large would be meaningless rather than wrong.
        self.standing_of(path)
            .1
            .map_or(-1, |distance| i64::try_from(distance).unwrap_or(i64::MAX))
    }
}

/// The string at `index`, or an empty one when there is no such entry.
fn at(strings: &[String], index: usize) -> String {
    strings.get(index).cloned().unwrap_or_default()
}

/// The move at `index`, when there is one.
fn rename(status: &Status, index: usize) -> Option<&Rename> {
    status.renamed.get(index)
}

/// The merge at `index`, when there is one.
fn merge(status: &Status, index: usize) -> Option<&Merge> {
    status.merging.get(index)
}

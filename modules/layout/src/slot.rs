//! What one `Uuid` is in one shard.

use crate::data::MutableData;
use crate::path::LayoutPath;

/// What a shard keeps for one `Uuid`: where it is, and what it holds.
///
/// The two are kept together so that everything about a `Uuid` is in one shard, which is what lets
/// a shard be written down on its own, snapshotted and its log dropped, without touching another.
/// Either may be absent between the two changes a create is.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The path this `Uuid` is at, if it is at one.
    pub path: Option<LayoutPath>,

    /// What it holds, if it holds anything.
    pub data: Option<MutableData>,
}

impl Slot {
    /// Whether this `Uuid` has neither a path nor anything held: it can be forgotten.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.path.is_none() && self.data.is_none()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::path::LayoutPath;

    use super::Slot;

    #[test]
    fn a_slot_is_empty_only_without_a_path_and_without_data() {
        assert!(Slot::default().is_empty());

        let placed = Slot {
            path: Some(LayoutPath::from_relative(Path::new("a.psd")).unwrap()),
            data: None,
        };
        assert!(!placed.is_empty());
    }
}

use serde::Serialize;

use crate::Key;

/// What a check is asked to look at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FsckOptions {
    /// Whether every key's content is read back and checked against its hash.
    ///
    /// It is off by default: the structural check names the packs, manifests and chunks that are
    /// gone or do not read without reading a byte of content, and a full check costs one read of
    /// every object a store holds.
    pub full: bool,
}

/// A content a store can no longer put back together, and the chunks it is missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingChunks {
    /// The key of the content whose manifest names the missing chunks.
    pub manifest: Key,
    /// The chunks the manifest names that the store does not hold, in the manifest's order.
    pub chunks: Vec<Key>,
}

/// What a check found in one store.
///
/// The counts say what was looked at and the lists say what was wrong with it. A report whose lists
/// are all empty is a store that holds together, see [`is_clean`](Self::is_clean).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FsckReport {
    /// How many packs the store names, of both kinds.
    pub checked_packs: usize,
    /// How many manifest keys the store names.
    pub checked_manifests: usize,
    /// How many keys were read and checked, which is zero unless the check was full.
    pub checked_objects: usize,
    /// The packs whose index does not read.
    pub broken_packs: Vec<u64>,
    /// The packs whose index reads but whose pack file is not there.
    pub missing_packs: Vec<u64>,
    /// The manifests that are there and do not read.
    pub broken_manifests: Vec<Key>,
    /// The contents whose manifests name chunks the store does not hold.
    pub missing_chunks: Vec<MissingChunks>,
    /// The contents that are not there at all, found by a full check.
    pub missing: Vec<Key>,
    /// The contents whose bytes do not hash to their keys, found by a full check.
    pub corrupt: Vec<Key>,
    /// The contents that are there and do not read as this build's frames, found by a full check.
    pub malformed: Vec<Key>,
}

impl FsckReport {
    /// Whether the store holds together: every list of findings is empty.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.broken_packs.is_empty()
            && self.missing_packs.is_empty()
            && self.broken_manifests.is_empty()
            && self.missing_chunks.is_empty()
            && self.missing.is_empty()
            && self.corrupt.is_empty()
            && self.malformed.is_empty()
    }

    /// How many findings there are, counted the way they are drawn: one per pack, manifest, content
    /// and manifest-with-missing-chunks rather than one per missing chunk.
    #[must_use]
    pub const fn problems(&self) -> usize {
        self.broken_packs.len()
            + self.missing_packs.len()
            + self.broken_manifests.len()
            + self.missing_chunks.len()
            + self.missing.len()
            + self.corrupt.len()
            + self.malformed.len()
    }
}

#[cfg(test)]
mod tests {
    use super::FsckReport;

    #[test]
    fn a_report_is_clean_only_when_every_list_is_empty() {
        assert!(FsckReport::default().is_clean());
        assert_eq!(FsckReport::default().problems(), 0);

        let dirty = FsckReport {
            broken_packs: vec![1],
            ..FsckReport::default()
        };
        assert!(!dirty.is_clean());
        assert_eq!(dirty.problems(), 1);
    }
}

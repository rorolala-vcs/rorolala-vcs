//! Where a store puts the things it keeps.
//!
//! The paths are the store's own — a caller hands over content and keys — so nothing here is more
//! than `pub(crate)`, and what a test needs of it is reached through [`Internals`](crate::internals).

use std::path::PathBuf;

use rorolala_utils_constants::STORAGE_CONFIG_PATH;

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, OBJECTS_DIR, PACKED_DIR, SLICE_FIRST, SLICE_SECOND};
use crate::Key;

impl RorolalaStorage {
    /// The file this store's configuration is kept in.
    ///
    /// It is the file a search finds the store by, so it is also what says a directory is one.
    pub(crate) fn config_path(&self) -> PathBuf {
        self.root.join(STORAGE_CONFIG_PATH)
    }

    /// Where the object stored under `key` sits, once it is loose.
    ///
    /// The digest is sharded two levels deep, so no directory collects an unbounded number of
    /// entries: a store with millions of objects still holds a few hundred per directory.
    pub(crate) fn object_path(&self, key: &Key) -> PathBuf {
        self.sharded(OBJECTS_DIR, key)
    }

    /// Where the manifest of the content stored under `key` sits.
    pub(crate) fn manifest_path(&self, key: &Key) -> PathBuf {
        self.sharded(MANIFEST_DIR, key)
    }

    /// Where the packed file of `index` sits, and where its index sits beside it.
    ///
    /// A pack is one file of many objects with a second file that says where each one is
    /// inside it, so the two are named together and are meant to be kept together.
    pub(crate) fn pack_paths(&self, index: u64) -> (PathBuf, PathBuf) {
        let root = self.root.join(PACKED_DIR);

        (
            root.join(format!("packed_{index}.pack")),
            root.join(format!("packed_{index}.idx")),
        )
    }

    /// Where a keyed entry sits under `directory`, sharded by the key's digest.
    fn sharded(&self, directory: &str, key: &Key) -> PathBuf {
        let hex = key.hex();

        self.root
            .join(directory)
            .join(&hex[..SLICE_FIRST])
            .join(&hex[SLICE_FIRST..SLICE_FIRST + SLICE_SECOND])
            .join(&hex)
    }
}

/// The pack number a pack index's name stands for, if it is one.
///
/// A pack is found by its index rather than by its pack file, so this is what says which packs a
/// store holds: a `.pack` still being written, or one whose index never arrived, is one no reader
/// would be able to reach anyway. Only the names this build writes are read: `packed_007.idx` parses
/// as seven, but the pack of seven is `packed_7.idx`, so listing it would name a pack nothing could
/// then find.
pub(super) fn index_of_pack(name: &str) -> Option<u64> {
    name.strip_prefix("packed_")
        .and_then(|rest| rest.strip_suffix(".idx"))
        .and_then(|number| number.parse().ok())
        .filter(|index| name == format!("packed_{index}.idx"))
}

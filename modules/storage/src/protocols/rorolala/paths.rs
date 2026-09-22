//! Where a store puts the things it keeps.
//!
//! The paths are the store's own — a caller hands over content and keys — so nothing here is more
//! than `pub(crate)`, and what a test needs of it is reached through [`Internals`](crate::internals).

use std::path::PathBuf;

use rorolala_utils_constants::STORAGE_CONFIG_PATH;

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, OBJECTS_DIR, PACKED_DIR, PackKind, SLICE_FIRST, SLICE_SECOND};
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

    /// Where the loose entry of `kind` stored under `key` sits.
    ///
    /// An object and a manifest of one content are two things a store keeps and two files it lays
    /// down, so which of them a path names is said by the kind rather than left to the caller.
    pub(super) fn entry_path(&self, kind: PackKind, key: &Key) -> PathBuf {
        match kind {
            PackKind::Object => self.object_path(key),
            PackKind::Manifest => self.manifest_path(key),
        }
    }

    /// Where the packed file of `index` sits.
    ///
    /// Every pack is one file, whichever kind of entry it holds: a pack number names one pack, and
    /// what that pack is *read* with is its index — see [`index_path`](Self::index_path).
    pub(super) fn pack_path(&self, index: u64) -> PathBuf {
        self.root
            .join(PACKED_DIR)
            .join(format!("packed_{index}.pack"))
    }

    /// Where the index of the pack of `index` sits, which is what says what the pack holds.
    ///
    /// An object pack keeps its index beside it; a manifest pack keeps its index among the manifests,
    /// so that reading everything that says how content was cut — which is what tells a chunk that is
    /// spoken for from one nothing refers to — is reading one directory rather than every pack.
    pub(super) fn index_path(&self, kind: PackKind, index: u64) -> PathBuf {
        let directory = match kind {
            PackKind::Object => PACKED_DIR,
            PackKind::Manifest => MANIFEST_DIR,
        };

        self.root
            .join(directory)
            .join(format!("packed_{index}.idx"))
    }

    /// Where the packed file of `index` sits, and where its index sits beside it.
    ///
    /// This is the shape of an *object* pack, which is the one anything that knows no better means.
    pub(crate) fn pack_paths(&self, index: u64) -> (PathBuf, PathBuf) {
        (
            self.pack_path(index),
            self.index_path(PackKind::Object, index),
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
/// A pack is found by its index rather than by its pack file, so this is what says which packs a store
/// holds: a `.pack` still being written, or one whose index never arrived, is one no reader would be
/// able to reach anyway. Only the names this build writes are read: `packed_007.idx` parses as seven,
/// but the pack of seven is `packed_7.idx`, so listing it would name a pack nothing could then find.
pub(super) fn index_of_pack(name: &str) -> Option<u64> {
    name.strip_prefix("packed_")
        .and_then(|rest| rest.strip_suffix(".idx"))
        .and_then(|number| number.parse().ok())
        .filter(|index| name == format!("packed_{index}.idx"))
}

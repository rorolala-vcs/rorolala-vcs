//! A layout written down as a file — a snapshot of what a workspace held at one moment.
//!
//! Everything else about a version control is content-addressed and never changes; the layout is the
//! one thing that changes. Writing it down is what lets one moment of it be kept, carried to another
//! machine, and put back: what each path named, which file upstream it was, and which version it was
//! at.
//!
//! It is lossy on purpose. What a snapshot is for is putting the work back, so it keeps what that
//! needs — the path, the `Uuid` a file is known by upstream, and the version — and drops what only
//! describes it: who held it and what it said. An entry no path names is in no place, so it is left
//! out of a file that is a list of places.
//!
//! A file is written two ways, told apart by a flag. One is written from a layout alone, and says
//! only what each path named. The other is **packed**: written with the index and the store beside
//! it, it also names every key a checkout of it needs — the versions and the variants from the
//! index, and the content and its chunks from the store — so a reader knows what it must be able to
//! reach before it can put the work back. A layout that cannot be packed whole is not packed at all:
//! what is missing is named rather than left for a checkout to discover.
//!
//! A file may also carry the Vault its layout was tracking, when it had one, so that what a layout
//! came from is not lost on the way out and does not have to be named again on the way in.
//!
//! # The bytes
//!
//! A file is a header, then a payload the header's checksum covers:
//!
//! ```text
//!   magic: 4 bytes  = "RLYT"
//!   version: 1      = the format this build writes
//!   flags: 1        = bit 0: the keys a checkout needs follow; bit 1: a tracked Vault is named
//!   checksum: 4     = crc32 of the payload
//!   payload:
//!     [str] the tracked Vault, when the flag is set
//!     [count: 8] keys when the flag is set, each: [key: 32], ascending
//!     [count: 8] entries, each: [len: 4][path: utf-8][uuid: 16][version: 32]
//! ```

use std::collections::BTreeSet;
use std::path::Path;

use rorolala_errors::Failure as _;
use rorolala_storage::{BLAKE3_HASH_LEN, Blake3Hash, Key, RorolalaStorage, StorageBackend as _};
use rorolala_vcs::{VCSIndex, VCSIndexReadingError, Variant, Version};
use uuid::Uuid;

use crate::bytes::{
    crc32, put_count, put_str, put_uuid, rest_empty, take_checksum, take_count, take_str, take_uuid,
};
use crate::error::LayoutError;
use crate::layout::Layout;
use crate::path::LayoutPath;

/// The magic every layout file starts with.
pub const LAYOUT_FILE_MAGIC: [u8; 4] = *b"RLYT";

/// The version of the layout file format this build writes.
pub const LAYOUT_FILE_VERSION: u8 = 1;

/// The flag bit that says the file carries the keys a checkout needs.
const FLAG_REQUIRES: u8 = 0b0000_0001;

/// The flag bit that says the file names the Vault its layout was tracking.
const FLAG_TRACK: u8 = 0b0000_0010;

/// How long a layout file's header is: the magic, the version, the flags and the checksum.
const HEADER_LEN: usize = LAYOUT_FILE_MAGIC.len() + 1 + 1 + 4;

/// The least a layout entry can take: a path length, a `Uuid` and a version.
const ENTRY_MIN_LEN: usize = 4 + size_of::<Uuid>() + BLAKE3_HASH_LEN;

/// One entry a layout file names: a path, the `Uuid` the file is upstream, and the version it is at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutEntry {
    /// Where the entry was.
    path: LayoutPath,

    /// The `Uuid` the file is known by upstream, which is how a newer version of it is found.
    id: Uuid,

    /// The version the entry was at, as the hash of that version.
    version: Blake3Hash,
}

impl LayoutEntry {
    /// Where the entry was.
    #[must_use]
    pub const fn path(&self) -> &LayoutPath {
        &self.path
    }

    /// The `Uuid` the file is known by upstream.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// The version the entry was at, as the hash of that version.
    #[must_use]
    pub const fn version(&self) -> [u8; BLAKE3_HASH_LEN] {
        self.version
    }
}

/// A layout written down as a file — a snapshot of what a workspace held at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFile {
    /// What each path named, in path order.
    entries: Vec<LayoutEntry>,

    /// The keys a checkout needs — versions, variants, content and chunks — ascending; `None` when
    /// the file was written from the layout alone and names no such thing.
    requires: Option<Vec<Key>>,

    /// The Vault the layout was tracking when it was written, if it was tracking one.
    track: Option<String>,
}

impl LayoutFile {
    /// The layout as it is held now, without a store: what each path named and what version it was.
    ///
    /// What it reads is the layout's own memory, so a caller that wants to see what another writer
    /// has written asks for that first — see [`Layout::refresh`]. An entry that names a path but
    /// holds no version, and one that holds a version no path names, are both left out: a snapshot
    /// is a list of what was in a place, and a half-made entry was not.
    #[must_use]
    pub fn of(layout: &Layout) -> Self {
        let mut entries: Vec<LayoutEntry> = layout
            .paths()
            .into_iter()
            .filter_map(|(path, id)| {
                let data = layout.entry(id)?;
                Some(LayoutEntry {
                    path,
                    id,
                    version: data.version(),
                })
            })
            .collect();

        entries.sort_unstable_by(|left, right| left.path.cmp(&right.path));

        Self {
            entries,
            requires: None,
            track: None,
        }
    }

    /// The layout packed against `index` and `store`, naming every key a checkout of it needs.
    ///
    /// For each entry the version is read from `index`, the variant it points at from the same, and
    /// the content that variant names is looked up in `store`: the content and, when it was kept
    /// cut, every chunk it is made of. All of these — versions, variants, content and chunks — are
    /// named in [`requires`](Self::requires), which is what a reader checks itself against before it
    /// puts the work back.
    ///
    /// The store is asked whether it holds the content, so a pack is only made where what it names
    /// is really there. A version, a variant or a chunk that is not is left to be named rather than
    /// packed around.
    ///
    /// What it reads is the layout's own memory, so a caller that wants to see what another writer
    /// has written asks for that first — see [`Layout::refresh`].
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::MissingContent`] naming everything a checkout would need that is not
    /// there, [`LayoutError::Index`] if the index fails to be read, [`LayoutError::Storage`] if the
    /// store does, and [`LayoutError::Malformed`] if what is stored is not the version or variant
    /// it should be.
    pub async fn pack(
        layout: &Layout,
        index: &VCSIndex,
        store: &RorolalaStorage,
    ) -> Result<Self, LayoutError> {
        let mut entries = Vec::new();
        let mut requires: BTreeSet<Key> = BTreeSet::new();
        let mut storages: BTreeSet<Key> = BTreeSet::new();
        let mut missing: Vec<Key> = Vec::new();

        for (path, id) in layout.paths() {
            let Some(data) = layout.entry(id) else {
                continue;
            };
            let version = data.version();
            entries.push(LayoutEntry { path, id, version });

            let version_key = Key::new(version);
            requires.insert(version_key);
            let Some(object) = version_at(index, &version_key).await? else {
                missing.push(version_key);
                continue;
            };

            let variant_key = Key::new(*object.variant());
            requires.insert(variant_key);
            let Some(variant) = variant_at(index, &variant_key).await? else {
                missing.push(variant_key);
                continue;
            };

            let content = Key::new(*variant.storage_hash());
            requires.insert(content);
            storages.insert(content);
            if let Some(manifest) = store.manifest_of(&content).await? {
                for chunk in manifest.chunks() {
                    requires.insert(chunk.key());
                    storages.insert(chunk.key());
                }
            }
        }

        // What the store is asked about is the storage keys alone: a version and a variant live in
        // the index and were found there, so asking the store for them would call them missing.
        let wanted: Vec<Key> = storages.into_iter().collect();
        let presence = store.contains_keys(&wanted).await?;
        for (key, held) in wanted.iter().zip(presence.iter()) {
            if !held {
                missing.push(*key);
            }
        }

        if !missing.is_empty() {
            missing.sort_unstable();
            missing.dedup();
            return Err(LayoutError::MissingContent(missing));
        }

        entries.sort_unstable_by(|left, right| left.path.cmp(&right.path));

        Ok(Self {
            entries,
            requires: Some(requires.into_iter().collect()),
            track: None,
        })
    }

    /// What each path named, in path order.
    #[must_use]
    pub fn entries(&self) -> &[LayoutEntry] {
        &self.entries
    }

    /// The keys a checkout needs, ascending, or `None` when the file names none.
    #[must_use]
    pub fn requires(&self) -> Option<&[Key]> {
        self.requires.as_deref()
    }

    /// Whether the file was packed against a store and names what a checkout needs.
    #[must_use]
    pub const fn is_packed(&self) -> bool {
        self.requires.is_some()
    }

    /// The Vault the layout was tracking when it was written, if it was tracking one.
    #[must_use]
    pub fn track(&self) -> Option<&str> {
        self.track.as_deref()
    }

    /// Names the Vault the layout tracks, or names none when it is empty.
    pub fn set_track(&mut self, track: Option<String>) {
        self.track = track.and_then(|track| {
            let track = track.trim().to_owned();
            (!track.is_empty()).then_some(track)
        });
    }

    /// This file, as the bytes it is written down as.
    ///
    /// The keys and the entries are already in order, so the same layout is written the same way
    /// every time — a file is a snapshot, and two snapshots of one state should be one file.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut payload = Vec::new();

        if let Some(track) = &self.track {
            put_str(&mut payload, track);
        }

        if let Some(requires) = &self.requires {
            put_count(&mut payload, requires.len());
            for key in requires {
                payload.extend_from_slice(key.digest());
            }
        }

        put_count(&mut payload, self.entries.len());
        for entry in &self.entries {
            put_str(&mut payload, entry.path.as_str());
            put_uuid(&mut payload, entry.id);
            payload.extend_from_slice(&entry.version);
        }

        let mut flags = 0_u8;
        if self.track.is_some() {
            flags |= FLAG_TRACK;
        }
        if self.requires.is_some() {
            flags |= FLAG_REQUIRES;
        }

        let mut bytes = Vec::with_capacity(HEADER_LEN + payload.len());
        bytes.extend_from_slice(&LAYOUT_FILE_MAGIC);
        bytes.push(LAYOUT_FILE_VERSION);
        bytes.push(flags);
        bytes.extend_from_slice(&crc32(&payload).to_be_bytes());
        bytes.extend_from_slice(&payload);

        bytes
    }

    /// The file written down as `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Malformed`] if `bytes` does not read as a layout file this build
    /// wrote — the wrong magic, a version or a flag this build does not know, a checksum that does
    /// not hold, a record that is cut short, or bytes left over at the end — and
    /// [`LayoutError::Path`] if a path it names is not one a layout may name.
    pub fn decode(bytes: &[u8]) -> Result<Self, LayoutError> {
        let rest = bytes
            .strip_prefix(LAYOUT_FILE_MAGIC.as_slice())
            .ok_or(LayoutError::Malformed)?;

        let (&version, rest) = rest.split_first().ok_or(LayoutError::Malformed)?;
        if version != LAYOUT_FILE_VERSION {
            return Err(LayoutError::Malformed);
        }

        let (&flags, rest) = rest.split_first().ok_or(LayoutError::Malformed)?;
        if flags & !(FLAG_REQUIRES | FLAG_TRACK) != 0 {
            return Err(LayoutError::Malformed);
        }

        let (checksum, payload) = take_checksum(rest)?;
        if crc32(payload) != checksum {
            return Err(LayoutError::Malformed);
        }

        let mut rest = payload;

        let track = if flags & FLAG_TRACK != 0 {
            let (track, after) = take_str(rest)?;
            rest = after;

            Some(track.to_owned())
        } else {
            None
        };

        let requires = if flags & FLAG_REQUIRES != 0 {
            let (count, after) = take_count(rest)?;
            rest = after;

            let mut keys = Vec::with_capacity(count.min(rest.len() / BLAKE3_HASH_LEN));
            for _ in 0..count {
                let (digest, after) = rest
                    .split_at_checked(BLAKE3_HASH_LEN)
                    .ok_or(LayoutError::Malformed)?;
                keys.push(Key::new(
                    digest.try_into().map_err(|_| LayoutError::Malformed)?,
                ));
                rest = after;
            }

            Some(keys)
        } else {
            None
        };

        let (count, mut rest) = take_count(rest)?;
        let mut entries = Vec::with_capacity(count.min(rest.len() / ENTRY_MIN_LEN));
        for _ in 0..count {
            let (text, after) = take_str(rest)?;
            let (id, after) = take_uuid(after)?;
            let (digest, after) = after
                .split_at_checked(BLAKE3_HASH_LEN)
                .ok_or(LayoutError::Malformed)?;

            entries.push(LayoutEntry {
                path: LayoutPath::new(text)?,
                id,
                version: digest.try_into().map_err(|_| LayoutError::Malformed)?,
            });
            rest = after;
        }

        rest_empty(rest)?;

        Ok(Self {
            entries,
            requires,
            track,
        })
    }

    /// The layout file kept at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Io`] if `path` cannot be read, and whatever [`decode`](Self::decode)
    /// fails with when what it holds is not a layout file.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, LayoutError> {
        Self::decode(&std::fs::read(path)?)
    }

    /// Writes the file to `path`, replacing whatever was there.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Io`] if `path` cannot be written.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), LayoutError> {
        std::fs::write(path, self.encode())?;

        Ok(())
    }
}

/// The version stored under `key`, or `None` when the index holds nothing under it.
async fn version_at(index: &VCSIndex, key: &Key) -> Result<Option<Version>, LayoutError> {
    match index.read(*key).await {
        Ok(object) => object
            .expect_version()
            .map(Some)
            .map_err(|_| LayoutError::Malformed),
        Err(VCSIndexReadingError::NotFound { .. }) => Ok(None),
        Err(error) => Err(LayoutError::Index(error.reason())),
    }
}

/// The variant stored under `key`, or `None` when the index holds nothing under it.
async fn variant_at(index: &VCSIndex, key: &Key) -> Result<Option<Variant>, LayoutError> {
    match index.read(*key).await {
        Ok(object) => object
            .expect_variant()
            .map(Some)
            .map_err(|_| LayoutError::Malformed),
        Err(VCSIndexReadingError::NotFound { .. }) => Ok(None),
        Err(error) => Err(LayoutError::Index(error.reason())),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::{Blake3Hash, Key, RorolalaStorage, StorageBackend as _, store_file};
    use rorolala_vcs::{VCSIndex, Version};
    use uuid::Uuid;

    use super::{LAYOUT_FILE_MAGIC, LAYOUT_FILE_VERSION, LayoutFile};
    use crate::data::MutableData;
    use crate::error::LayoutError;
    use crate::layout::Layout;
    use crate::path::LayoutPath;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-layout-file-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A path, from text a test can read.
    fn path(text: &str) -> LayoutPath {
        LayoutPath::new(text).unwrap()
    }

    /// What a checkout of a version needs, so a test can say what a pack should name.
    struct Held {
        /// The version in the index.
        version: Key,
        /// The variant the version points at.
        variant: Key,
        /// The content that variant names, in the store.
        content: Key,
    }

    /// Writes content into `store` and a version pointing at it into `index`.
    async fn hold(dir: &Path, index: &VCSIndex, store: &RorolalaStorage) -> Held {
        let file = dir.join("content.bin");
        std::fs::write(&file, b"the content a version stands on").unwrap();
        let content = store_file(store, &file).await.unwrap();

        let variant = Version::root().new_variant(*content.digest(), [1; 32], [2; 32]);
        let variant_key = index.write(variant.clone()).await.unwrap();
        let version = variant.new_version();
        let version_key = index.write(version.clone()).await.unwrap();

        Held {
            version: version_key,
            variant: variant_key,
            content,
        }
    }

    /// A layout in `dir` holding one path at a version.
    fn layout_over(dir: &Path, version: &Blake3Hash) -> Layout {
        let layout = Layout::open(dir.join("layout")).unwrap();
        let id = Uuid::from_u128(5);

        layout
            .create_entry(
                id,
                MutableData::new(Some("alice".to_owned()), *version, "first".to_owned()),
            )
            .unwrap();
        layout.create_path(&path("a/b.txt"), id).unwrap();

        layout
    }

    #[test]
    fn a_layout_written_down_alone_reads_back_as_it_went_in() {
        let layout = Layout::open(scratch("plain")).unwrap();
        let id = Uuid::from_u128(1);

        layout
            .create_entry(
                id,
                MutableData::new(Some("alice".to_owned()), [7; 32], "first".to_owned()),
            )
            .unwrap();
        layout.create_path(&path("a/b.txt"), id).unwrap();

        let file = LayoutFile::of(&layout);
        let read = LayoutFile::decode(&file.encode()).unwrap();

        assert!(!read.is_packed());
        assert!(read.requires().is_none());
        assert_eq!(read.entries().len(), 1);
        assert_eq!(read.entries()[0].path(), &path("a/b.txt"));
        assert_eq!(read.entries()[0].id(), id);
        assert_eq!(read.entries()[0].version(), [7; 32]);
    }

    #[test]
    fn what_a_path_was_makes_a_file_the_same_however_it_was_walked() {
        let layout = Layout::open(scratch("order")).unwrap();
        let a = Uuid::from_u128(21);
        let b = Uuid::from_u128(22);

        layout
            .create_entry(a, MutableData::new(None, [1; 32], "a".to_owned()))
            .unwrap();
        layout
            .create_entry(b, MutableData::new(None, [2; 32], "b".to_owned()))
            .unwrap();
        layout.create_path(&path("b.txt"), a).unwrap();
        layout.create_path(&path("a.txt"), b).unwrap();
        // A path that names an entry nothing was made for: a place with nothing in it.
        layout
            .create_path(&path("c.txt"), Uuid::from_u128(23))
            .unwrap();

        let file = LayoutFile::of(&layout);
        let paths: Vec<&str> = file
            .entries()
            .iter()
            .map(|entry| entry.path().as_str())
            .collect();

        assert_eq!(paths, vec!["a.txt", "b.txt"]);
        assert_eq!(file.encode(), LayoutFile::of(&layout).encode());
    }

    #[test]
    fn a_file_carries_the_vault_its_layout_tracked() {
        let layout = Layout::open(scratch("track")).unwrap();
        let mut file = LayoutFile::of(&layout);

        // A layout that tracks nothing is written with nothing.
        assert_eq!(file.track(), None);

        file.set_track(Some("the-vault".to_owned()));
        assert_eq!(file.track(), Some("the-vault"));
        assert_eq!(
            LayoutFile::decode(&file.encode()).unwrap().track(),
            Some("the-vault")
        );

        // Naming nothing again leaves it naming nothing.
        file.set_track(Some("   ".to_owned()));
        assert_eq!(file.track(), None);
        assert_eq!(LayoutFile::decode(&file.encode()).unwrap().track(), None);
    }

    #[test]
    fn what_does_not_read_as_a_layout_file_is_refused() {
        let bytes = LayoutFile {
            entries: Vec::new(),
            requires: None,
            track: None,
        }
        .encode();

        // The magic is what says a file is a layout file at all.
        assert!(LayoutFile::decode(b"not a layout file").is_err());

        // A file shorter than it says it is is cut short.
        assert!(LayoutFile::decode(&bytes[..bytes.len() - 1]).is_err());

        // And one longer is not read past.
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(LayoutFile::decode(&extra).is_err());

        // A checksum that does not hold is a payload that did not survive.
        let mut broken = bytes.clone();
        let last = broken.len() - 1;
        broken[last] ^= 0xff;
        assert!(LayoutFile::decode(&broken).is_err());

        // A flag this build does not know is not one it will read past.
        let mut unknown = bytes.clone();
        unknown[LAYOUT_FILE_MAGIC.len() + 1] |= 0b1000_0000;
        assert!(LayoutFile::decode(&unknown).is_err());

        // The version the header names has to be the one this build writes.
        let mut other = bytes;
        other[LAYOUT_FILE_MAGIC.len()] = LAYOUT_FILE_VERSION.wrapping_add(1);
        assert!(LayoutFile::decode(&other).is_err());
    }

    #[tokio::test]
    async fn a_packed_layout_names_everything_a_checkout_needs() {
        let dir = scratch("packed");
        let store = RorolalaStorage::create(dir.join("store"));
        let index = VCSIndex::create(dir.join("index"));
        let held = hold(&dir, &index, &store).await;
        let layout = layout_over(&dir, held.version.digest());

        let file = LayoutFile::pack(&layout, &index, &store).await.unwrap();
        assert!(file.is_packed());

        let requires = file
            .requires()
            .expect("a packed layout names what it needs");
        assert!(requires.contains(&held.version));
        assert!(requires.contains(&held.variant));
        assert!(requires.contains(&held.content));

        // And what it names survives being written down and read back.
        assert_eq!(LayoutFile::decode(&file.encode()).unwrap(), file);
    }

    #[tokio::test]
    async fn a_layout_that_cannot_be_packed_says_what_is_missing() {
        let dir = scratch("missing");
        let store = RorolalaStorage::create(dir.join("store"));
        let index = VCSIndex::create(dir.join("index"));
        let held = hold(&dir, &index, &store).await;
        let layout = layout_over(&dir, held.version.digest());

        store.remove(&held.content).await.unwrap();

        match LayoutFile::pack(&layout, &index, &store).await {
            Err(LayoutError::MissingContent(keys)) => assert!(keys.contains(&held.content)),
            other => panic!("expected the missing content to be named, got {other:?}"),
        }
    }
}

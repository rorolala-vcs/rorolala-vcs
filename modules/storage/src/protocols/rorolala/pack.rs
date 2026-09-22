//! Packs: many objects in one file, and the index that says where each one sits.
//!
//! A pack is a place objects live rather than a thing a caller knows about, so it holds entries
//! exactly as they sat loose and is found — and committed — by its index. The work here is keeping
//! that true when packs are written, read, and taken apart again.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::SystemTime;

use tokio::sync::Mutex as AsyncMutex;

use super::RorolalaStorage;
use super::consts::PACKED_DIR;
use super::entry::{exists, read_range, sync_directory, write_whole_durably};
use super::paths::index_of_pack;
use crate::{Error, Key, PackEntry, PackIndex};

impl RorolalaStorage {
    /// Packs the loose objects stored under `keys` into a pack of their own.
    ///
    /// What the pack holds is each entry exactly as it sat loose — the frame and all — so an object
    /// that moves into a pack is the same bytes in a different place, and every read that worked
    /// before it moved works after. This is the whole of why packing is safe to do at any time and
    /// safe to leave undone: nothing about an object depends on how many of them share a file.
    ///
    /// Keys already named by a pack are passed over, so packing the loose objects of a store does not
    /// make a second copy of one that already has a home. Packs are changed under one lock in this
    /// process — see `root_state` — so two packings here cannot both take one key; two *processes*
    /// working one store still can, and what holds the store together then is
    /// [`remove`](crate::StorageBackend::remove), which asks every pack that names the key rather
    /// than the first. A batch with nothing loose among it makes no pack at all.
    ///
    /// ```text
    /// packed_<n>.pack      the entries one after another, byte for byte
    /// packed_<n>.idx       where each entry starts and how long it is
    /// ```
    ///
    /// That is how a pack is **written**. Once an entry is dropped from an index, the bytes it
    /// occupied stay where they are, so a pack that has had something removed from it is no longer
    /// one run of entries — the index is what says where each of them is, and it is read before
    /// anything is.
    ///
    /// The index is written after the pack it describes, so a pack that is still being written is a
    /// pack no reader can find yet: a pack is known by its index, not by its name. The pack's number
    /// is claimed by creating its file before a byte is written, so two ends packing at once take
    /// two numbers rather than one.
    ///
    /// Whether a pack was made is all this answers: the number a pack was given is the store's own
    /// business, and what a caller wanted to know is whether the objects it handed over changed
    /// anything.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the pack or its index cannot be written, and [`Error::Malformed`]
    /// if a path it needs cannot be named.
    pub async fn pack(&self, keys: &[Key]) -> Result<bool, Error> {
        use tokio::io::AsyncWriteExt as _;

        // Changing a store's packs is one at a time within this process: what a pack holds is read,
        // changed and written back, so two changes at once would each write one that has forgotten
        // the other. See `root_state`.
        let state = self.root_state();
        let _packing = state.packing.lock().await;

        // The keys are taken in order so that two packs of the same objects are the same file down
        // to the byte, which is what makes a pack something a transfer can compare rather than
        // merely read. A key named twice is one object, so it is taken once. What is held here is
        // the keys rather than the objects: the bytes go into the pack as they are read, one object
        // at a time, so a batch is not measured by what it holds.
        let mut ordered = keys.to_vec();
        ordered.sort_unstable();
        ordered.dedup();

        let packed = self.packed_keys().await?;
        let mut loose = Vec::new();

        for key in ordered {
            // A key a pack already names is not one to pack again: an object in two packs would be
            // one `remove` could not take away, since it would be found in the first and left in the
            // second. A key nothing is loose under — packed already, or never written — is passed
            // over, so a batch with nothing loose among it makes no pack at all.
            if packed.contains(&key) {
                continue;
            }

            if exists(&self.object_path(&key)).await? {
                loose.push(key);
            }
        }

        if loose.is_empty() {
            return Ok(false);
        }

        // The number is claimed and the pack file created before the index exists, so nothing else
        // can take this number while this pack is being written, and no reader can reach a pack that
        // is not finished.
        let (index, mut pack) = self.claim_pack().await?;
        let mut directory = PackIndex::default();
        let mut moved = Vec::new();
        let mut offset = 0_u64;

        for key in loose {
            // A loose object the store stopped holding between the check above and here is left out:
            // it is gone, so there is nothing to move and no entry to name.
            let bytes = match tokio::fs::read(self.object_path(&key)).await {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };

            pack.write_all(&bytes).await?;
            directory.push(PackEntry::new(key, offset, bytes.len() as u64));
            moved.push(key);
            offset += bytes.len() as u64;
        }

        // Every key vanished between the check and the read, so the number is given back rather than
        // a pack made of nothing.
        if moved.is_empty() {
            drop(pack);
            self.drop_entry(&self.pack_paths(index).0).await?;
            sync_directory(&self.root.join(PACKED_DIR)).await?;

            return Ok(false);
        }

        // The pack's bytes are on the disk before its name is made durable, and its name before the
        // index is written: a reader that finds this index after a crash has to find a pack that is
        // there and all there, and bytes on the disk are not the same as a name in the directory.
        pack.sync_all().await?;
        drop(pack);
        sync_directory(&self.root.join(PACKED_DIR)).await?;

        let (_, index_path) = self.pack_paths(index);
        // The index is written to outlive a crash, and only then are the loose copies dropped —
        // see `write_whole_durably`.
        write_whole_durably(&index_path, &directory.encode()).await?;

        // The objects live in the pack now, so the loose copies go. This is a move rather than a
        // drop: what the key promises is still held, in another place, and a read finds it either
        // way.
        for key in &moved {
            self.drop_entry(&self.object_path(key)).await?;
        }

        Ok(true)
    }

    /// Claims the next pack's number by creating its file, and answers the number and that file.
    ///
    /// The number is taken by **creating the pack file**, which succeeds for one writer and fails
    /// with [`AlreadyExists`](io::ErrorKind::AlreadyExists) for the rest, who move on to the next
    /// number. A number reached by a directory scan alone would be a number two ends could both
    /// reach, and the second pack would write over the first — losing every object the first one
    /// moved, since by then its loose copies are gone.
    ///
    /// The file is handed back open for writing and as empty as it was created: the caller streams
    /// the pack into it rather than holding the whole of it to write in one go. Nothing is written
    /// beside it and moved into place, because a pack no index names is a pack no reader can reach —
    /// a write cut short leaves an unreachable file rather than a reachable one that is half there,
    /// and the index the caller writes afterwards is what makes it a pack.
    async fn claim_pack(&self) -> Result<(u64, tokio::fs::File), Error> {
        tokio::fs::create_dir_all(self.root.join(PACKED_DIR)).await?;

        let mut candidate = self.next_pack_index().await?;
        loop {
            let (pack_path, _) = self.pack_paths(candidate);
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&pack_path)
                .await
            {
                Ok(file) => return Ok((candidate, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    candidate = candidate.saturating_add(1);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// The packs this store holds, lowest index first.
    ///
    /// A pack counts as held once its index is there, which is what the pack is found by.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the pack directory cannot be read.
    pub(crate) async fn packs(&self) -> Result<Vec<u64>, Error> {
        let mut indices = Vec::new();

        for path in super::entry::entries(&self.root.join(PACKED_DIR)).await? {
            if let Some(index) = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(index_of_pack)
            {
                indices.push(index);
            }
        }

        indices.sort_unstable();
        indices.dedup();

        Ok(indices)
    }

    /// The directory of the pack of `index`, saying what it holds and where.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the index cannot be read and [`Error::Malformed`] if it does not
    /// read as an index.
    pub(crate) async fn read_pack_index(&self, index: u64) -> Result<PackIndex, Error> {
        let (_, index_path) = self.pack_paths(index);
        let bytes = tokio::fs::read(&index_path).await?;

        PackIndex::decode(&bytes)
    }

    /// The packs whose index does not read.
    ///
    /// A lookup answers "not found" for a pack whose index does not read, so that one index gone
    /// wrong does not hide every other pack — see `pack_index`. That is the right answer for a read
    /// and a poor one for anyone looking after the store: nothing on the ordinary path says the pack
    /// is there at all. This is where that is asked, and it is asked of every pack in the store
    /// rather than of one lookup's — and it reads each index for itself rather than being answered
    /// from what a lookup remembered, since a check that can be answered from a memory is not one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the pack directory or a pack's file cannot be read at all, which is a
    /// store that is failing rather than one holding a bad file.
    pub(crate) async fn broken_packs(&self) -> Result<Vec<u64>, Error> {
        let state = self.root_state();
        let mut broken = Vec::new();

        for index in self.packs().await? {
            let (_, index_path) = self.pack_paths(index);
            let bytes = match tokio::fs::read(&index_path).await {
                Ok(bytes) => bytes,
                // A pack taken away between the scan and the read is one that is simply not there.
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    forget_pack_index(&state, index);

                    continue;
                }
                Err(error) => return Err(error.into()),
            };

            if PackIndex::decode(&bytes).is_err() {
                broken.push(index);
            }
        }

        Ok(broken)
    }

    /// Takes the object stored under `key` out of the pack of `index`.
    ///
    /// Only the pack's index is rewritten: the entry is dropped from it, and the bytes it occupied
    /// are left where they are. A pack and its index are two files, and a reader reads the index to
    /// find out where the pack's bytes are — so rewriting the pack as well would leave a window in
    /// which the index describes a pack that is already another one, and every object in it would be
    /// read wrong. Dropping the entry from the index is the whole removal: nothing reaches the bytes
    /// afterwards.
    ///
    /// What the bytes keep costing is not reclaimed here. A pack that had something removed from it
    /// is still as large as it was, and nothing in this store rewrites a pack to win that back yet;
    /// writing packs afresh, and retiring the old ones, is what would.
    ///
    /// A pack left holding nothing is dropped whole, the index first — a pack is found by its index,
    /// so a pack without one is a pack no reader reaches, and a `.pack` left behind by a write cut
    /// short is garbage rather than a store that does not hold together.
    pub(super) async fn remove_from_pack(
        &self,
        state: &RootState,
        index: u64,
        key: &Key,
    ) -> Result<(), Error> {
        let (pack_path, index_path) = self.pack_paths(index);
        let Some(directory) = self.pack_index(state, index).await? else {
            return Ok(());
        };

        let kept = PackIndex::new(
            directory
                .entries()
                .iter()
                .copied()
                .filter(|entry| entry.key() != *key)
                .collect(),
        );

        if kept.is_empty() {
            // The index goes first, since it is what a pack is found by — and what was read of it
            // goes with it, so that the number it frees is not answered with it. See
            // `forget_pack_index`.
            forget_pack_index(state, index);
            self.drop_entry(&index_path).await?;
            // The index going is made durable before the pack itself goes, so that a crash cannot
            // leave an index naming a pack that is not there.
            sync_directory(&self.root.join(PACKED_DIR)).await?;
            self.drop_entry(&pack_path).await?;

            // The pack is gone once the directory says so.
            return sync_directory(&self.root.join(PACKED_DIR)).await;
        }

        write_whole_durably(&index_path, &kept.encode()).await
    }

    /// The framed entry stored under `key`, read loose where it is loose and out of a pack where it
    /// was packed.
    ///
    /// Nothing above this has to know which of the two it is: what a key promises is the content,
    /// and where storage keeps it is storage's own business.
    pub(super) async fn object_entry(&self, key: &Key) -> Result<Option<Vec<u8>>, Error> {
        match tokio::fs::read(self.object_path(key)).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.packed_entry(key).await,
            Err(error) => Err(error.into()),
        }
    }

    /// The framed entry stored under `key` in whichever pack holds it, if any does.
    async fn packed_entry(&self, key: &Key) -> Result<Option<Vec<u8>>, Error> {
        let state = self.root_state();

        for index in self.packs().await? {
            let Some(directory) = self.pack_index(&state, index).await? else {
                continue;
            };

            if let Some(entry) = directory.find(key) {
                let (pack_path, _) = self.pack_paths(index);

                match read_range(&pack_path, entry.offset(), entry.len()).await {
                    Ok(bytes) => return Ok(Some(bytes)),
                    // The index was read and the pack taken away before it was opened — what
                    // dropping a pack's last object does. A pack that is not there holds nothing,
                    // which is the same answer as an index that is not there.
                    Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }

        Ok(None)
    }

    /// Every key the packs name.
    ///
    /// This is what tells a packing from a batch of keys already packed: a key a pack names is not
    /// one to pack a second time.
    async fn packed_keys(&self) -> Result<BTreeSet<Key>, Error> {
        let state = self.root_state();
        let mut keys = BTreeSet::new();

        for index in self.packs().await? {
            let Some(directory) = self.pack_index(&state, index).await? else {
                continue;
            };

            for entry in directory.entries() {
                keys.insert(entry.key());
            }
        }

        Ok(keys)
    }

    /// The number of the pack holding `key`, if one does.
    pub(super) async fn find_packed(&self, key: &Key) -> Result<Option<u64>, Error> {
        let state = self.root_state();

        for index in self.packs().await? {
            if self
                .pack_index(&state, index)
                .await?
                .is_some_and(|directory| directory.find(key).is_some())
            {
                return Ok(Some(index));
            }
        }

        Ok(None)
    }

    /// The lock this store's mutating operations are held under.
    ///
    /// A pack's index is read, changed and written back rather than replaced in one step, so two
    /// changes to one pack have to take turns: without that, two `remove`s could each write an index
    /// that has forgotten the other's, and the pack would go on naming an object the store said it
    /// had dropped. The lock is shared by every store *in this process* that names the same root,
    /// since `at` and `create` each hand back a store of their own for one directory.
    ///
    /// It reaches no further than this process. Two processes working one store are not made to take
    /// turns — the design keeps no cross-process lock — and what holds there is the pack *number*
    /// being claimed by creating its file, which is what stops two packings from writing one pack.
    pub(super) fn root_state(&self) -> Arc<RootState> {
        let root = self.root_key();
        let mut states = root_states()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        Arc::clone(states.entry(root).or_default())
    }

    /// The key a root is known by when this process is deciding which stores describe one directory.
    ///
    /// `canonicalize` makes two spellings of one directory one key, and needs the directory to be
    /// there to do it. A store that is not on disk yet — or one rooted at `""` — is keyed by where
    /// it would be instead, which is lexical and so never needs the disk to be readable for two
    /// stores of one directory to agree.
    fn root_key(&self) -> PathBuf {
        fs::canonicalize(&self.root)
            .or_else(|_| std::path::absolute(&self.root))
            .unwrap_or_else(|_| self.root.clone())
    }

    /// The pack of `index` as it was last read, if that reading still describes the file.
    ///
    /// A pack's index is read again for every object a lookup walks past, which for a content of
    /// many chunks is a great many decodings of the same file. What is kept is the decoded index
    /// together with how the file looked when it was read, so a file that has not changed is not read
    /// again, and one that has is. The stamp is the file's length and modification time: an index is
    /// only ever rewritten to name fewer objects, so a change shortens it, and a stamp that is the
    /// same is a file that is the same.
    ///
    /// `None` is a pack that holds nothing: there is no index, or what is there does not read. Both
    /// are the same answer to a lookup, and neither is kept — a pack that comes back is read afresh.
    ///
    /// The state is passed in rather than looked up here, so that a lookup walking over many packs
    /// asks for it once rather than once a pack.
    pub(super) async fn pack_index(
        &self,
        state: &RootState,
        index: u64,
    ) -> Result<Option<Arc<PackIndex>>, Error> {
        let (_, index_path) = self.pack_paths(index);
        let metadata = match tokio::fs::metadata(&index_path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                forget_pack_index(state, index);

                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let stamp = Stamp {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        };

        let kept = state
            .indices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&index)
            .filter(|kept| kept.stamp == stamp)
            .map(|kept| Arc::clone(&kept.index));

        if let Some(kept) = kept {
            return Ok(Some(kept));
        }

        let bytes = match tokio::fs::read(&index_path).await {
            Ok(bytes) => bytes,
            // The index was statted and then taken away before it was read — what dropping a pack's
            // last object does. A pack that is not there is a pack that holds nothing, which is the
            // same answer as an index that is not there.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                forget_pack_index(state, index);

                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };

        let Ok(decoded) = PackIndex::decode(&bytes) else {
            return Ok(None);
        };
        let decoded = Arc::new(decoded);

        state
            .indices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                index,
                KeptIndex {
                    stamp,
                    index: Arc::clone(&decoded),
                },
            );

        Ok(Some(decoded))
    }

    /// The number the next pack is given: one past the highest there is.
    async fn next_pack_index(&self) -> Result<u64, Error> {
        Ok(self
            .packs()
            .await?
            .last()
            .map_or(0, |last| last.saturating_add(1)))
    }
}

/// What this process keeps for one store root, shared by every store of it.
#[derive(Default)]
pub(super) struct RootState {
    /// The lock the packs' mutations are held under — see `root_state`.
    pub(super) packing: AsyncMutex<()>,
    /// The pack indexes that have been read, by pack number.
    pub(super) indices: StdMutex<HashMap<u64, KeptIndex>>,
}

/// A pack index that has been read, and what its file looked like when it was.
pub(super) struct KeptIndex {
    /// The file's length and modification time, which is how a changed one is told.
    stamp: Stamp,
    /// The index itself, shared so that handing it out copies a pointer rather than the list.
    index: Arc<PackIndex>,
}

/// What says one version of a file from another without reading it.
///
/// It is a length and a modification time rather than either alone: rewrites of an index in this
/// store always shorten it, so a length that is the same is a file that did not change, and the time
/// agrees with that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    /// When the file was last written, where the filesystem tells it.
    modified: Option<SystemTime>,
    /// How long the file is.
    len: u64,
}

/// Drops what was read of the pack of `index`, since there is no pack there to have read.
///
/// What was read is dropped whenever the index is gone, so that a pack number later given to
/// another pack is not answered with what the last one held. A number is given out as one past the
/// highest there is, so emptying a pack frees its number for the next packing — and an index read as
/// a stale one would name objects at offsets that no longer mean them.
fn forget_pack_index(state: &RootState, index: u64) {
    state
        .indices
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&index);
}

/// The state this process keeps for each store root, by the root it is for.
///
/// It is by root rather than by store because a store is handed out freely — `at`, `create` and
/// `locate` each make a new one for one directory — and two of them still describe one set of packs,
/// and so have to share one lock and one view of what has been read.
///
/// An entry is kept for the life of the process once it is handed out, so the map grows with the
/// number of distinct stores this process has changed, not with the number of changes. That is a
/// handful of workspaces and vaults for a daemon, which is why nothing drops them: state that could
/// be dropped under a running operation is worse than state that stays.
fn root_states() -> &'static StdMutex<HashMap<PathBuf, Arc<RootState>>> {
    /// Every root this process has kept state for.
    static STATES: OnceLock<StdMutex<HashMap<PathBuf, Arc<RootState>>>> = OnceLock::new();

    STATES.get_or_init(|| StdMutex::new(HashMap::new()))
}

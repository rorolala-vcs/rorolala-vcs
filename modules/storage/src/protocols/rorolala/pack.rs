//! Packs: many entries in one file, and the index that says where each one sits.
//!
//! A pack is a place entries live rather than a thing a caller knows about, so it holds them exactly
//! as they sat loose and is found — and committed — by its index. The work here is keeping that true
//! when packs are written, read, and taken apart again.
//!
//! A pack holds one [kind](PackKind) of entry: objects, which are the content itself, or manifests,
//! which say how a chunked content was cut. A pack number names one pack of one kind — the kinds are
//! told apart by where their indexes sit, and by nothing else — so an object and a manifest under one
//! key are two entries in two packs, and neither is mistaken for the other.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::SystemTime;

use tokio::sync::Mutex as AsyncMutex;

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, OBJECTS_DIR, PACKED_DIR, PackKind};
use super::entry::{collect, is_directory, read_range, sync_directory, write_whole_durably};
use super::paths::index_of_pack;
use crate::{Error, Key, PackEntry, PackIndex};

impl RorolalaStorage {
    /// Packs the loose entries stored under `keys`, of either kind, into packs of their own.
    ///
    /// What a pack holds is each entry exactly as it sat loose — the frame and all — so an entry that
    /// moves into a pack is the same bytes in a different place, and every read that worked before it
    /// moved works after. This is the whole of why packing is safe to do at any time and safe to leave
    /// undone: nothing about an entry depends on how many of them share a file.
    ///
    /// A key may be loose as an object, as a manifest, or as both, and the two are different entries:
    /// each is packed unless a pack of its own kind already names it. What is written goes into as many
    /// packs as the store's [`max_pack_size`](Self::max_pack_size) allows — one kind of entry to a pack
    /// — so a batch larger than that limit is laid down as several packs rather than one file no bound
    /// holds. A batch with nothing loose among it makes no pack at all.
    ///
    /// Packs are changed under one lock in this process — see `root_state` — so two packings here
    /// cannot both take one key; two *processes* working one store still can, and what holds the store
    /// together then is [`remove`](crate::StorageBackend::remove), which asks every pack that names the
    /// key rather than the first.
    ///
    /// Whether a pack was made is all this answers: the number a pack was given is the store's own
    /// business, and what a caller wanted to know is whether the entries it handed over changed
    /// anything.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the pack or its index cannot be written, and [`Error::Malformed`]
    /// if a path it needs cannot be named.
    pub async fn pack(&self, keys: &[Key]) -> Result<bool, Error> {
        // Changing a store's packs is one at a time within this process: what a pack holds is read,
        // changed and written back, so two changes at once would each write one that has forgotten
        // the other. See `root_state`.
        let state = self.root_state();
        let _packing = state.packing.lock().await;

        // The keys are taken in order so that two packs of the same entries are the same file down to
        // the byte, which is what makes a pack something a transfer can compare rather than merely
        // read. A key named twice is one entry of its kind, so it is taken once.
        let mut ordered = keys.to_vec();
        ordered.sort_unstable();
        ordered.dedup();

        let packed: [(PackKind, BTreeSet<Key>); 2] = [
            (PackKind::Object, self.packed_keys(PackKind::Object).await?),
            (
                PackKind::Manifest,
                self.packed_keys(PackKind::Manifest).await?,
            ),
        ];
        let mut entries = Vec::new();

        for key in ordered {
            for (kind, held) in &packed {
                // An entry a pack of its kind already names is not one to pack again: one in two packs
                // would be one `remove` could not take away, since it would be found in the first and
                // left in the second.
                if held.contains(&key) {
                    continue;
                }

                let path = self.entry_path(*kind, &key);
                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => entries.push((*kind, key, metadata.len(), Source::Loose(path))),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }

        // A pack holds one kind, so the entries are taken kind by kind.
        entries.sort_unstable_by_key(|(kind, key, _, _)| (*kind, *key));

        let written = self.write_packs(&entries).await?;

        // What moved into a pack is not loose any more. This is a move rather than a drop: what the key
        // promises is still held, in another place, and a read finds it either way.
        for (_, kind, keys) in &written {
            for key in keys {
                self.drop_entry(&self.entry_path(*kind, key)).await?;
            }
        }

        if !written.is_empty() {
            // The directories the loose entries sat in are empty now, so they go with them.
            self.prune_empty_directories().await?;
        }

        Ok(!written.is_empty())
    }

    /// Lays the store's entries out afresh, so that as few packs hold them as the size limit allows.
    ///
    /// Where [`pack`](Self::pack) puts the loose entries of a batch into packs, this takes the whole of
    /// what the store holds — the entries of every pack, and every loose entry, of either kind — and
    /// writes it again, merging the packs there are into one wherever they fit under
    /// [`max_pack_size`](Self::max_pack_size) and starting another where they do not. What each entry
    /// *is* does not change: an entry is written byte for byte as it was, so every read that worked
    /// before works after, whatever packs there were.
    ///
    /// A store already laid out the way this would lay it out is left alone and answered with `false`,
    /// so running this twice in a row rewrites nothing the second time. A pack whose index does not
    /// read is left where it is: what it holds cannot be read here to be written again, and dropping it
    /// would drop the only copy.
    ///
    /// The new packs are written and made durable before any old one is dropped, so a reader that
    /// arrives in the middle of this finds every entry in the packs it already knew; the old packs go
    /// last, their indexes first. A store interrupted here is a store holding a pack too many rather
    /// than one holding an entry too few.
    ///
    /// What the store is left with is numbered from nothing and without a gap — see `renumber` — so
    /// packing a store over and over does not climb a number that says how often it was packed. The
    /// directories the loose entries sat in, empty once they moved, go as well.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if an entry cannot be read or a pack or index cannot be written, and
    /// [`Error::Malformed`] if a path it needs cannot be named.
    pub async fn repack(&self) -> Result<bool, Error> {
        // Packs are changed one at a time within this process, the same as any other change to them —
        // see `root_state`.
        let state = self.root_state();
        let _packing = state.packing.lock().await;

        // Everything the store holds, wherever it holds it, under the kind and key it is held by: an
        // entry named by two packs, or one both packed and loose, is one entry here and is written once.
        let mut existing: Vec<(PackKind, u64, Arc<PackIndex>)> = Vec::new();
        let mut entries: BTreeMap<(PackKind, Key), (u64, Source)> = BTreeMap::new();

        for kind in [PackKind::Object, PackKind::Manifest] {
            for index in self.packs_of(kind).await? {
                let Some(directory) = self.pack_index(&state, kind, index).await? else {
                    continue;
                };

                for entry in directory.entries() {
                    entries.insert(
                        (kind, entry.key()),
                        (
                            entry.len(),
                            Source::Packed {
                                index,
                                offset: entry.offset(),
                                len: entry.len(),
                            },
                        ),
                    );
                }

                existing.push((kind, index, directory));
            }
        }

        // A loose entry is read from the file it is in, and it is written in place of a pack entry that
        // names the same kind and key: the two are the same bytes, so this decides only which copy is
        // dropped afterwards.
        for kind in [PackKind::Object, PackKind::Manifest] {
            for key in self.loose_keys(kind).await? {
                let path = self.entry_path(kind, &key);

                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => {
                        entries.insert((kind, key), (metadata.len(), Source::Loose(path)));
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }

        let ordered: Vec<Planned> = entries
            .into_iter()
            .map(|((kind, key), (len, source))| (kind, key, len, source))
            .collect();

        // What the store would look like written afresh, against what it looks like now: a store that is
        // already that — every pack a group of this plan, and nothing loose — is left alone rather than
        // written over as it stands.
        let mut planned = planned_groups(&ordered, self.max_pack_size().await);
        let mut current: Vec<Vec<Key>> = existing
            .iter()
            .map(|(_, _, directory)| directory.entries().iter().map(PackEntry::key).collect())
            .collect();
        planned.sort();
        current.sort();

        let loose = ordered
            .iter()
            .any(|(_, _, _, source)| matches!(source, Source::Loose(_)));
        if !loose && planned == current {
            return Ok(false);
        }

        let written = self.write_packs(&ordered).await?;

        // Nothing could be read to write again — every source was gone between being listed and being
        // read. The store is left as it is rather than having packs dropped that what is here could not
        // rebuild.
        if written.is_empty() {
            return Ok(false);
        }

        // What is in a pack now is not loose any more; a key that was already packed is not loose at
        // all, so the drop is a no-op for it.
        for (_, kind, keys) in &written {
            for key in keys {
                self.drop_entry(&self.entry_path(*kind, key)).await?;
            }
        }

        // The old packs go last, and only once the new ones are there. Their indexes go first, since a
        // pack is found by its index, and a `.pack` left behind by a write cut short is garbage rather
        // than a store that does not hold together.
        for (kind, index, _) in &existing {
            self.drop_pack(&state, *kind, *index).await?;
        }

        // The packs the store is left with are numbered from nothing, without a gap: what packs there
        // are is the store's own business, and a number that only ever grows says how many times a
        // store has been packed rather than what it holds.
        self.renumber(&state, &written).await?;

        // What the packing left behind is the shape of what the store used to hold — directories with
        // nothing left in them — so those go with it.
        self.prune_empty_directories().await?;

        Ok(true)
    }

    /// Renames the packs that were just written to the numbers they belong at, from nothing up.
    ///
    /// What a pack is found by is its index, and an index names the pack of its number, so the index
    /// has to be at its final name before the pack is: an index whose pack is not there yet reads as a
    /// pack holding nothing, and every entry is still found in the pack it is named by until then. The
    /// other way round would leave an index naming a pack that is no longer there, which is every entry
    /// in it unreachable.
    ///
    /// The packs are taken in the order they were written and given the numbers from nothing, so the
    /// renaming never steps on a pack still to be moved: a pack's final number is always one that an
    /// earlier pack has already left.
    async fn renumber(
        &self,
        state: &RootState,
        written: &[(u64, PackKind, Vec<Key>)],
    ) -> Result<(), Error> {
        for (at, (index, kind, _)) in written.iter().enumerate() {
            let wanted = at as u64;
            if wanted == *index {
                continue;
            }

            let from_index = self.index_path(*kind, *index);
            let to_index = self.index_path(*kind, wanted);
            let bytes = tokio::fs::read(&from_index).await?;
            write_whole_durably(&to_index, &bytes).await?;

            tokio::fs::rename(self.pack_path(*index), self.pack_path(wanted)).await?;
            sync_directory(&self.root.join(PACKED_DIR)).await?;

            forget_pack_index(state, *kind, *index);
            self.drop_entry(&from_index).await?;
            sync_directory(&self.index_directory(*kind)).await?;
        }

        Ok(())
    }

    /// Removes the directories the store left empty, so that a store that was packed is not one still
    /// shaped like what it used to hold.
    ///
    /// Only what is under the layout's own directories is taken: those stay, since a store is one with
    /// somewhere to put everything — see `create`.
    async fn prune_empty_directories(&self) -> Result<(), Error> {
        for directory in [OBJECTS_DIR, MANIFEST_DIR, PACKED_DIR] {
            self.prune_empty_under(&self.root.join(directory)).await?;
        }

        Ok(())
    }

    /// Removes the empty directories under `directory`, leaving `directory` itself.
    async fn prune_empty_under(&self, directory: &Path) -> Result<(), Error> {
        for child in super::entry::entries(directory).await? {
            if !is_directory(&child).await? {
                continue;
            }

            Box::pin(self.prune_empty_under(&child)).await?;

            // Nothing is left under it, so it is nothing the store has a use for.
            if super::entry::entries(&child).await?.is_empty() {
                tokio::fs::remove_dir(&child).await?;
            }
        }

        Ok(())
    }

    /// Writes `entries` into packs, each of at most the size the store allows.
    ///
    /// A pack holds one kind of entry, so the entries of one kind are written to a pack of their own;
    /// `entries` is in kind then key order, which is what makes each kind one run.
    ///
    /// A pack's number is claimed by creating its file before a byte is written, so nothing else can
    /// take it while this pack is being written, and no reader can reach a pack that is not finished —
    /// a pack is found by its index, which is written last.
    ///
    /// What is answered is each pack that was written, its kind, and the keys it holds, in the order
    /// they were written. A pack whose entries all turned out to be gone is given back rather than left
    /// empty: a pack nothing names is not a pack.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if an entry cannot be read or a pack or index cannot be written.
    async fn write_packs(
        &self,
        entries: &[Planned],
    ) -> Result<Vec<(u64, PackKind, Vec<Key>)>, Error> {
        use tokio::io::AsyncWriteExt as _;

        let max = self.max_pack_size().await;
        let mut written = Vec::new();

        for run in kind_runs(entries) {
            let part = &entries[run];
            let kind = part[0].0;

            for range in pack_ranges(part, max) {
                let mut packing = self.begin_pack(kind).await?;
                let index = packing.index;
                let mut keys = Vec::new();

                for (_, key, _, source) in &part[range] {
                    // An entry the store stopped holding between being listed and being read is left
                    // out: it is gone, so there is nothing to move and no entry to name.
                    let Some(bytes) = self.read_source(source).await? else {
                        continue;
                    };

                    packing.file.write_all(&bytes).await?;
                    packing
                        .directory
                        .push(PackEntry::new(*key, packing.size, bytes.len() as u64));
                    packing.size += bytes.len() as u64;
                    keys.push(*key);
                }

                // Every entry vanished, so the number is given back rather than a pack made of nothing.
                if keys.is_empty() {
                    drop(packing);
                    self.drop_entry(&self.pack_path(index)).await?;
                    sync_directory(&self.root.join(PACKED_DIR)).await?;

                    continue;
                }

                self.finish_pack(packing).await?;
                written.push((index, kind, keys));
            }
        }

        Ok(written)
    }

    /// Claims a pack's number and opens it for writing.
    async fn begin_pack(&self, kind: PackKind) -> Result<Packing, Error> {
        let (index, file) = self.claim_pack().await?;

        Ok(Packing {
            kind,
            index,
            file,
            directory: PackIndex::default(),
            size: 0,
        })
    }

    /// Makes a written pack durable and gives it its index.
    ///
    /// The pack's bytes are on the disk before its name is made durable, and its name before the index
    /// is written: a reader that finds this index after a crash has to find a pack that is there and
    /// all there, and bytes on the disk are not the same as a name in the directory.
    async fn finish_pack(&self, packing: Packing) -> Result<(), Error> {
        packing.file.sync_all().await?;
        drop(packing.file);
        sync_directory(&self.root.join(PACKED_DIR)).await?;

        let index_path = self.index_path(packing.kind, packing.index);

        write_whole_durably(&index_path, &packing.directory.encode()).await
    }

    /// The bytes of one entry, wherever it is being read from.
    ///
    /// `None` is an entry that is not there any more, which is not a failure: what is gone cannot be
    /// written again, and a pack is not made to name it.
    async fn read_source(&self, source: &Source) -> Result<Option<Vec<u8>>, Error> {
        match source {
            Source::Loose(path) => match tokio::fs::read(path).await {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error.into()),
            },
            Source::Packed { index, offset, len } => {
                let pack_path = self.pack_path(*index);

                match read_range(&pack_path, *offset, *len).await {
                    Ok(bytes) => Ok(Some(bytes)),
                    Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(error),
                }
            }
        }
    }

    /// The keys of the loose entries of `kind` the store holds.
    async fn loose_keys(&self, kind: PackKind) -> Result<Vec<Key>, Error> {
        let mut keys = BTreeSet::new();
        collect(&self.loose_directory(kind), &mut keys).await?;

        Ok(keys.into_iter().collect())
    }

    /// Drops the pack of `index`: its index first, then the pack.
    ///
    /// The index is what a pack is found by, so it goes before the pack does — a crash between the two
    /// leaves a `.pack` no reader reaches rather than an index naming a pack that is not there.
    async fn drop_pack(&self, state: &RootState, kind: PackKind, index: u64) -> Result<(), Error> {
        let pack_path = self.pack_path(index);
        let index_path = self.index_path(kind, index);

        forget_pack_index(state, kind, index);
        self.drop_entry(&index_path).await?;
        sync_directory(&self.index_directory(kind)).await?;
        self.drop_entry(&pack_path).await?;

        sync_directory(&self.root.join(PACKED_DIR)).await
    }

    /// Claims the next pack's number by creating its file, and answers the number and that file.
    ///
    /// The number is taken by **creating the pack file**, which succeeds for one writer and fails with
    /// [`AlreadyExists`](io::ErrorKind::AlreadyExists) for the rest, who move on to the next number. A
    /// number reached by a directory scan alone would be a number two ends could both reach, and the
    /// second pack would write over the first — losing every entry the first one moved, since by then
    /// its loose copies are gone.
    ///
    /// The file is handed back open for writing and as empty as it was created: the caller streams the
    /// pack into it rather than holding the whole of it to write in one go. Nothing is written beside it
    /// and moved into place, because a pack no index names is a pack no reader can reach — a write cut
    /// short leaves an unreachable file rather than a reachable one that is half there, and the index
    /// the caller writes afterwards is what makes it a pack.
    async fn claim_pack(&self) -> Result<(u64, tokio::fs::File), Error> {
        tokio::fs::create_dir_all(self.root.join(PACKED_DIR)).await?;

        let mut candidate = self.next_pack_index().await?;
        loop {
            let pack_path = self.pack_path(candidate);
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

    /// The packs of `kind` this store holds, lowest index first.
    ///
    /// A pack counts as held once its index is there, which is what the pack is found by.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the directory the indexes are kept in cannot be read.
    pub(super) async fn packs_of(&self, kind: PackKind) -> Result<Vec<u64>, Error> {
        let mut indices = Vec::new();

        for path in super::entry::entries(&self.index_directory(kind)).await? {
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

    /// Every pack this store holds, of either kind, lowest index first.
    ///
    /// This is the numbering a new pack is given a number past — see `next_pack_index` — so it is one
    /// sequence over both kinds: a number names one pack of one kind, and a pack file is one file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if a directory an index is kept in cannot be read.
    pub(crate) async fn packs(&self) -> Result<Vec<u64>, Error> {
        let mut indices = self.packs_of(PackKind::Object).await?;
        indices.extend(self.packs_of(PackKind::Manifest).await?);
        indices.sort_unstable();
        indices.dedup();

        Ok(indices)
    }

    /// The directory of the pack of `index`, saying what it holds and where.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the index cannot be read and [`Error::Malformed`] if it does not read
    /// as an index.
    pub(crate) async fn read_pack_index(&self, index: u64) -> Result<PackIndex, Error> {
        let bytes = tokio::fs::read(self.index_path(PackKind::Object, index)).await?;

        PackIndex::decode(&bytes)
    }

    /// The packs whose index does not read, of either kind.
    ///
    /// A lookup answers "not found" for a pack whose index does not read, so that one index gone wrong
    /// does not hide every other pack — see `pack_index`. That is the right answer for a read and a poor
    /// one for anyone looking after the store: nothing on the ordinary path says the pack is there at
    /// all. This is where that is asked, and it is asked of every pack in the store rather than of one
    /// lookup's — and it reads each index for itself rather than being answered from what a lookup
    /// remembered, since a check that can be answered from a memory is not one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if a directory an index is kept in or a pack's file cannot be read at all,
    /// which is a store that is failing rather than one holding a bad file.
    pub(crate) async fn broken_packs(&self) -> Result<Vec<u64>, Error> {
        let state = self.root_state();
        let mut broken = Vec::new();

        for kind in [PackKind::Object, PackKind::Manifest] {
            for index in self.packs_of(kind).await? {
                let index_path = self.index_path(kind, index);
                let bytes = match tokio::fs::read(&index_path).await {
                    Ok(bytes) => bytes,
                    // A pack taken away between the scan and the read is one that is simply not there.
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        forget_pack_index(&state, kind, index);

                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };

                if PackIndex::decode(&bytes).is_err() {
                    broken.push(index);
                }
            }
        }

        broken.sort_unstable();
        broken.dedup();

        Ok(broken)
    }

    /// Takes the entry stored under `key` out of the pack of `kind` and `index`.
    ///
    /// Only the pack's index is rewritten: the entry is dropped from it, and the bytes it occupied are
    /// left where they are. A pack and its index are two files, and a reader reads the index to find out
    /// where the pack's bytes are — so rewriting the pack as well would leave a window in which the
    /// index describes a pack that is already another one, and every entry in it would be read wrong.
    /// Dropping the entry from the index is the whole removal: nothing reaches the bytes afterwards.
    ///
    /// What the bytes keep costing is not reclaimed here. A pack that had something removed from it is
    /// still as large as it was; [`repack`](RorolalaStorage::repack) is what writes packs afresh and
    /// retires the old ones.
    ///
    /// A pack left holding nothing is dropped whole, the index first — a pack is found by its index, so
    /// a pack without one is a pack no reader reaches, and a `.pack` left behind by a write cut short is
    /// garbage rather than a store that does not hold together.
    pub(super) async fn remove_from_pack(
        &self,
        state: &RootState,
        kind: PackKind,
        index: u64,
        key: &Key,
    ) -> Result<(), Error> {
        let pack_path = self.pack_path(index);
        let index_path = self.index_path(kind, index);
        let Some(directory) = self.pack_index(state, kind, index).await? else {
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
            // The index goes first, since it is what a pack is found by — and what was read of it goes
            // with it, so that the number it frees is not answered with it. See `forget_pack_index`.
            forget_pack_index(state, kind, index);
            self.drop_entry(&index_path).await?;
            // The index going is made durable before the pack itself goes, so that a crash cannot leave
            // an index naming a pack that is not there.
            sync_directory(&self.index_directory(kind)).await?;
            self.drop_entry(&pack_path).await?;

            // The pack is gone once the directory says so.
            return sync_directory(&self.root.join(PACKED_DIR)).await;
        }

        write_whole_durably(&index_path, &kept.encode()).await
    }

    /// The framed entry stored under `key`, read loose where it is loose and out of a pack where it was
    /// packed.
    ///
    /// Nothing above this has to know which of the two it is: what a key promises is the content, and
    /// where storage keeps it is storage's own business.
    pub(super) async fn object_entry(&self, key: &Key) -> Result<Option<Vec<u8>>, Error> {
        match tokio::fs::read(self.object_path(key)).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.packed_entry(PackKind::Object, key).await
            }
            Err(error) => Err(error.into()),
        }
    }

    /// The framed manifest stored under `key`, read loose where it is loose and out of a manifest pack
    /// where it was packed.
    ///
    /// A manifest is read the same way an object is, and out of a pack of its own kind: a key may name
    /// both, and the manifest of a content is not the content.
    pub(super) async fn manifest_entry(&self, key: &Key) -> Result<Option<Vec<u8>>, Error> {
        match tokio::fs::read(self.manifest_path(key)).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.packed_entry(PackKind::Manifest, key).await
            }
            Err(error) => Err(error.into()),
        }
    }

    /// The framed entry stored under `key` in whichever pack of `kind` holds it, if any does.
    async fn packed_entry(&self, kind: PackKind, key: &Key) -> Result<Option<Vec<u8>>, Error> {
        let state = self.root_state();

        for index in self.packs_of(kind).await? {
            let Some(directory) = self.pack_index(&state, kind, index).await? else {
                continue;
            };

            if let Some(entry) = directory.find(key) {
                let pack_path = self.pack_path(index);

                match read_range(&pack_path, entry.offset(), entry.len()).await {
                    Ok(bytes) => return Ok(Some(bytes)),
                    // The index was read and the pack taken away before it was opened — what dropping a
                    // pack's last entry does. A pack that is not there holds nothing, which is the same
                    // answer as an index that is not there.
                    Err(Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }

        Ok(None)
    }

    /// Every key the packs of `kind` name.
    ///
    /// This is what tells a packing from a batch of keys already packed: a key a pack of its kind names
    /// is not one to pack a second time.
    async fn packed_keys(&self, kind: PackKind) -> Result<BTreeSet<Key>, Error> {
        let state = self.root_state();
        let mut keys = BTreeSet::new();

        for index in self.packs_of(kind).await? {
            let Some(directory) = self.pack_index(&state, kind, index).await? else {
                continue;
            };

            for entry in directory.entries() {
                keys.insert(entry.key());
            }
        }

        Ok(keys)
    }

    /// The number of the object pack holding `key`, if one does.
    pub(super) async fn find_packed(&self, key: &Key) -> Result<Option<u64>, Error> {
        let state = self.root_state();

        for index in self.packs_of(PackKind::Object).await? {
            if self
                .pack_index(&state, PackKind::Object, index)
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
    /// A pack's index is read, changed and written back rather than replaced in one step, so two changes
    /// to one pack have to take turns: without that, two `remove`s could each write an index that has
    /// forgotten the other's, and the pack would go on naming an entry the store said it had dropped.
    /// The lock is shared by every store *in this process* that names the same root, since `at` and
    /// `create` each hand back a store of their own for one directory.
    ///
    /// It reaches no further than this process. Two processes working one store are not made to take
    /// turns — the design keeps no cross-process lock — and what holds there is the pack *number* being
    /// claimed by creating its file, which is what stops two packings from writing one pack.
    pub(super) fn root_state(&self) -> Arc<RootState> {
        let root = self.root_key();
        let mut states = root_states()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        Arc::clone(states.entry(root).or_default())
    }

    /// The key a root is known by when this process is deciding which stores describe one directory.
    ///
    /// `canonicalize` makes two spellings of one directory one key, and needs the directory to be there
    /// to do it. A store that is not on disk yet — or one rooted at `""` — is keyed by where it would be
    /// instead, which is lexical and so never needs the disk to be readable for two stores of one
    /// directory to agree.
    fn root_key(&self) -> PathBuf {
        fs::canonicalize(&self.root)
            .or_else(|_| std::path::absolute(&self.root))
            .unwrap_or_else(|_| self.root.clone())
    }

    /// The pack of `kind` and `index` as it was last read, if that reading still describes the file.
    ///
    /// A pack's index is read again for every entry a lookup walks past, which for a content of many
    /// chunks is a great many decodings of the same file. What is kept is the decoded index together
    /// with how the file looked when it was read, so a file that has not changed is not read again, and
    /// one that has is. The stamp is the file's length and modification time: an index is only ever
    /// rewritten to name fewer entries, so a change shortens it, and a stamp that is the same is a file
    /// that is the same.
    ///
    /// `None` is a pack that holds nothing: there is no index, or what is there does not read. Both are
    /// the same answer to a lookup, and neither is kept — a pack that comes back is read afresh.
    ///
    /// The state is passed in rather than looked up here, so that a lookup walking over many packs asks
    /// for it once rather than once a pack.
    pub(super) async fn pack_index(
        &self,
        state: &RootState,
        kind: PackKind,
        index: u64,
    ) -> Result<Option<Arc<PackIndex>>, Error> {
        let index_path = self.index_path(kind, index);
        let metadata = match tokio::fs::metadata(&index_path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                forget_pack_index(state, kind, index);

                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let stamp = Stamp {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        };
        let known = (kind, index);

        let kept = state
            .indices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&known)
            .filter(|kept| kept.stamp == stamp)
            .map(|kept| Arc::clone(&kept.index));

        if let Some(kept) = kept {
            return Ok(Some(kept));
        }

        let bytes = match tokio::fs::read(&index_path).await {
            Ok(bytes) => bytes,
            // The index was statted and then taken away before it was read — what dropping a pack's last
            // entry does. A pack that is not there is a pack that holds nothing, which is the same answer
            // as an index that is not there.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                forget_pack_index(state, kind, index);

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
                known,
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

    /// The directory the loose entries of `kind` are kept in.
    ///
    /// A manifest pack keeps its index beside the loose manifests rather than beside the packs — see
    /// [`index_path`](RorolalaStorage::index_path) — so for a manifest this and
    /// [`index_directory`](RorolalaStorage::index_directory) are one directory, and for an object they
    /// are two.
    fn loose_directory(&self, kind: PackKind) -> PathBuf {
        self.root.join(match kind {
            PackKind::Object => OBJECTS_DIR,
            PackKind::Manifest => MANIFEST_DIR,
        })
    }

    /// The directory the indexes of the packs of `kind` are kept in.
    fn index_directory(&self, kind: PackKind) -> PathBuf {
        self.root.join(match kind {
            PackKind::Object => PACKED_DIR,
            PackKind::Manifest => MANIFEST_DIR,
        })
    }
}

/// One entry to be written into a pack: which kind it is, its key, how long it is, and where its bytes
/// are read from.
type Planned = (PackKind, Key, u64, Source);

/// Where an entry being written into a pack is read from.
enum Source {
    /// The loose entry file it sits in.
    Loose(PathBuf),
    /// Inside the pack of a number, at an offset and of a length.
    Packed {
        /// The number of the pack the entry sits in.
        index: u64,
        /// How many bytes into that pack the entry starts.
        offset: u64,
        /// How many bytes of that pack the entry takes up.
        len: u64,
    },
}

/// A pack being written: what it holds, the file, and how big it is.
struct Packing {
    /// Which kind of entry the pack holds.
    kind: PackKind,
    /// The number claimed for this pack.
    index: u64,
    /// The pack file, open for writing.
    file: tokio::fs::File,
    /// What the pack holds so far, in key order.
    directory: PackIndex,
    /// How many bytes of the pack have been written.
    size: u64,
}

/// Where the entries of one kind begin and end.
///
/// A pack holds one kind — see [`write_packs`](RorolalaStorage::write_packs) — so what is written as
/// one kind's packs is one run of `entries`. This is those runs, which `entries` being in kind then key
/// order is what makes them.
fn kind_runs(entries: &[Planned]) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start = 0;

    for at in 1..entries.len() {
        if entries[at].0 != entries[start].0 {
            runs.push(start..at);
            start = at;
        }
    }

    if start < entries.len() {
        runs.push(start..entries.len());
    }

    runs
}

/// Where the packs of `entries` begin and end, one pack each.
///
/// The entries are taken in the order they are given — which is key order — and a pack is closed once
/// the next entry would take it over `max`. An entry larger than `max` is a pack of its own, since a
/// limit cannot be met by a single entry that is over it.
fn pack_ranges(entries: &[Planned], max: u64) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut size = 0_u64;

    for (at, (_, _, len, _)) in entries.iter().enumerate() {
        if at > start && size.saturating_add(*len) > max {
            ranges.push(start..at);
            start = at;
            size = 0;
        }

        size = size.saturating_add(*len);
    }

    if start < entries.len() {
        ranges.push(start..entries.len());
    }

    ranges
}

/// The groups of keys `entries` are written into, one group per pack.
///
/// This is the shape `write_packs` gives, worked out without reading a byte: it is what tells a store
/// already laid out that way from one that is not.
fn planned_groups(entries: &[Planned], max: u64) -> Vec<Vec<Key>> {
    let mut groups = Vec::new();

    for run in kind_runs(entries) {
        let part = &entries[run];

        for range in pack_ranges(part, max) {
            groups.push(part[range].iter().map(|(_, key, _, _)| *key).collect());
        }
    }

    groups
}

/// What this process keeps for one store root, shared by every store of it.
#[derive(Default)]
pub(super) struct RootState {
    /// The lock the packs' mutations are held under — see `root_state`.
    pub(super) packing: AsyncMutex<()>,
    /// The pack indexes that have been read, by kind and pack number.
    pub(super) indices: StdMutex<HashMap<(PackKind, u64), KeptIndex>>,
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
/// It is a length and a modification time rather than either alone: rewrites of an index in this store
/// always shorten it, so a length that is the same is a file that did not change, and the time agrees
/// with that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    /// When the file was last written, where the filesystem tells it.
    modified: Option<SystemTime>,
    /// How long the file is.
    len: u64,
}

/// Drops what was read of the pack of `kind` and `index`, since there is no pack there to have read.
///
/// What was read is dropped whenever the index is gone, so that a pack number later given to another
/// pack is not answered with what the last one held. A number is given out as one past the highest
/// there is, so emptying a pack frees its number for the next packing — and an index read as a stale
/// one would name entries at offsets that no longer mean them.
fn forget_pack_index(state: &RootState, kind: PackKind, index: u64) {
    state
        .indices
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&(kind, index));
}

/// The state this process keeps for each store root, by the root it is for.
///
/// It is by root rather than by store because a store is handed out freely — `at`, `create` and `locate`
/// each make a new one for one directory — and two of them still describe one set of packs, and so have
/// to share one lock and one view of what has been read.
///
/// An entry is kept for the life of the process once it is handed out, so the map grows with the number
/// of distinct stores this process has changed, not with the number of changes. That is a handful of
/// workspaces and vaults for a daemon, which is why nothing drops them: state that could be dropped
/// under a running operation is worse than state that stays.
fn root_states() -> &'static StdMutex<HashMap<PathBuf, Arc<RootState>>> {
    /// Every root this process has kept state for.
    static STATES: OnceLock<StdMutex<HashMap<PathBuf, Arc<RootState>>>> = OnceLock::new();

    STATES.get_or_init(|| StdMutex::new(HashMap::new()))
}

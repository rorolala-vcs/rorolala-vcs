//! Laying the index's objects out in a pack.
//!
//! A pack is many objects in one file, and the index beside it says where each one sits; the
//! bytes an entry holds are the very bytes it sat loose as, the frame and all, so packing
//! changes where an object lives and nothing else. The index keeps one pack rather than as many
//! as a size limit allows: what it holds are the small, fixed objects the version control logic
//! works in, so there is nothing to weigh one pack against another for.
//!
//! A pack is found by its index, so the index is written before the pack it names and dropped
//! after it; a `.pack` left behind by a write cut short is garbage rather than an index naming a
//! pack that is not there.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, SystemTime};

use rorolala_storage::{Codec, FRAME_VERSION, Frame, Layout, PackEntry, PackIndex};
use rorolala_utils_progress::Progress;

use super::{VCSIndex, VCSIndexObject, VCSWrite, entries};
use crate::Hash;
use crate::error::VCSIndexError;

/// The directory packs are kept under, inside an index.
const PACKED_DIR: &str = "packed";

/// Where an object is read from while the index is laid out afresh.
pub enum Source {
    /// A loose file, which holds the whole framed entry.
    Loose(PathBuf),
    /// An entry inside a pack, which is read by its place in that pack.
    Packed(u64, PackEntry),
}

impl VCSIndex {
    /// Where the packed file of `index` sits.
    fn pack_path(&self, index: u64) -> PathBuf {
        self.path
            .join(PACKED_DIR)
            .join(format!("packed_{index}.pack"))
    }

    /// Where the index of the pack of `index` sits, which is what says what the pack holds.
    fn pack_index_path(&self, index: u64) -> PathBuf {
        self.path
            .join(PACKED_DIR)
            .join(format!("packed_{index}.idx"))
    }

    /// The packs the index holds, lowest index first.
    ///
    /// A pack is found by its index rather than by its pack file, so a `.pack` whose index never
    /// arrived is one no reader could reach anyway, and is not counted.
    ///
    /// A lookup walks every pack of the index, and a lookup is what a read of every object is made
    /// of, so the list is kept with what the directory looked like when it was read: a directory
    /// that has not changed names the packs it named before. A pack is installed by renaming its
    /// index into this directory, so a packing begun is not a packing a reader sees.
    pub(crate) async fn packs(&self) -> Result<Vec<u64>, VCSIndexError> {
        let state = self.pack_state();
        let directory = self.path.join(PACKED_DIR);

        let stamp = match tokio::fs::metadata(&directory).await {
            Ok(metadata) => stamp_of(&metadata),
            // A directory that is not there holds no packs, and keeps nothing: one that appears is
            // listed afresh.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                forget_packs(&state);

                return Ok(Vec::new());
            }
            Err(error) => return Err(error.into()),
        };

        let kept = state
            .packs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|kept| kept.stamp == stamp)
            .map(|kept| Arc::clone(&kept.packs));

        if let Some(kept) = kept {
            return Ok((*kept).clone());
        }

        let mut packs = Vec::new();
        for path in entries(&directory).await? {
            if let Some(name) = path.file_name().and_then(|name| name.to_str())
                && let Some(index) = index_of_pack(name)
            {
                packs.push(index);
            }
        }

        packs.sort_unstable();

        let listed = Arc::new(packs);
        *state.packs.lock().unwrap_or_else(PoisonError::into_inner) = Some(KeptPacks {
            stamp,
            packs: Arc::clone(&listed),
        });

        Ok((*listed).clone())
    }

    /// The directory of the pack of `index`, as it was last read, when that reading still describes
    /// the file.
    ///
    /// A pack's index is read again for every lookup that walks past it, which for a presence check
    /// over a whole batch is the same file decoded again and again. What is kept is the decoded
    /// index together with how the file looked when it was read: a file that has not changed is not
    /// read again, and one that has is. The stamp is the file's length and modification time, an
    /// index is rewritten to name another set of entries rather than appended to, so a stamp that is
    /// the same is the file that was read.
    ///
    /// `None` is a pack whose index is not there: a pack taken away between the scan that named it
    /// and this read, which holds nothing and is the same answer as one that was never there. An
    /// index that is there and does not decode is [`VCSIndexError::Malformed`], which a lookup passes
    /// over and [`broken_packs`](Self::broken_packs) reports.
    pub(crate) async fn read_pack_index(
        &self,
        index: u64,
    ) -> Result<Option<Arc<PackIndex>>, VCSIndexError> {
        let state = self.pack_state();
        let path = self.pack_index_path(index);

        let metadata = match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                forget_pack_index(&state, index);

                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        let stamp = stamp_of(&metadata);

        let kept = state
            .indices
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&index)
            .filter(|kept| kept.stamp == stamp)
            .map(|kept| Arc::clone(&kept.index));

        if let Some(kept) = kept {
            return Ok(Some(kept));
        }

        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            // Statted and then taken away before it was read, which is what dropping a pack's index
            // does. A pack that is not there holds nothing, the same answer as an index never there.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                forget_pack_index(&state, index);

                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };

        let decoded = PackIndex::decode(&bytes)?;
        let decoded = Arc::new(decoded);

        state
            .indices
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                index,
                KeptIndex {
                    stamp,
                    index: Arc::clone(&decoded),
                },
            );

        Ok(Some(decoded))
    }

    /// The packs whose index does not read.
    ///
    /// A listing skips a pack it cannot read rather than failing whole, what `list_object_keys`
    /// does, which is the right answer for a reader and a poor one for whoever looks after the
    /// index: nothing on the ordinary path says the pack is there at all. This is where that is
    /// asked, and it reads each index for itself rather than being answered from a memory a lookup
    /// kept.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Io`] if an index cannot be read at all, which is an index that is
    /// failing rather than one holding a file that does not decode.
    pub async fn broken_packs(&self) -> Result<Vec<u64>, VCSIndexError> {
        let mut broken = Vec::new();

        for index in self.packs().await? {
            match self.read_pack_index(index).await {
                Ok(_) => {}
                Err(VCSIndexError::Malformed) => broken.push(index),
                // Gone between the scan and the read: a pack that is not there holds nothing, and
                // there is no index left to report.
                Err(VCSIndexError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }

        Ok(broken)
    }

    /// Where the object stored under `key` sits in a pack, if a pack holds it.
    ///
    /// A pack whose index does not read is passed over rather than failing the lookup: nothing can be
    /// found through it, and one bad index answering for every pack is what a store with a hundred of
    /// them must not do. It is not passed over in silence, [`broken_packs`](Self::broken_packs) names
    /// it, and a pack taken away between the scan and the read is the same answer as an index that
    /// was never there.
    pub(crate) async fn find_packed(&self, key: &Hash) -> Result<Option<Source>, VCSIndexError> {
        for index in self.packs().await? {
            match self.read_pack_index(index).await {
                Ok(Some(directory)) => {
                    if let Some(entry) = directory.find(key) {
                        return Ok(Some(Source::Packed(index, *entry)));
                    }
                }
                Ok(None) | Err(VCSIndexError::Malformed) => {}
                Err(VCSIndexError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }

        Ok(None)
    }

    /// The bytes of the entry `source` names, framed as it sits.
    pub(crate) async fn read_source(&self, source: &Source) -> Result<Vec<u8>, VCSIndexError> {
        match source {
            Source::Loose(path) => Ok(tokio::fs::read(path).await?),
            Source::Packed(index, entry) => {
                read_range(&self.pack_path(*index), entry.offset(), entry.len()).await
            }
        }
    }

    /// The first `head` bytes of the entry `source` names, or fewer when the entry is shorter.
    ///
    /// What one byte past a frame answers is which kind the object is, and an entry is not read whole
    /// for it: a packed entry costs this many bytes out of its pack rather than its own length, and a
    /// pack whose file is gone holds nothing, which is answered as no bytes at all.
    pub(crate) async fn read_source_head(
        &self,
        source: &Source,
        head: u64,
    ) -> Result<Vec<u8>, VCSIndexError> {
        match source {
            Source::Loose(path) => match tokio::fs::read(path).await {
                Ok(mut bytes) => {
                    bytes.truncate(usize::try_from(head).unwrap_or(usize::MAX));

                    Ok(bytes)
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
                Err(error) => Err(error.into()),
            },
            Source::Packed(index, entry) => {
                let head = entry.len().min(head);

                match read_range(&self.pack_path(*index), entry.offset(), head).await {
                    Ok(bytes) => Ok(bytes),
                    Err(VCSIndexError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                        Ok(Vec::new())
                    }
                    Err(error) => Err(error),
                }
            }
        }
    }

    /// Every key the index holds loosely.
    pub(crate) async fn list_loose_keys(&self) -> Result<Vec<Hash>, VCSIndexError> {
        let mut keys = BTreeSet::new();
        super::collect(&self.path.join(super::OBJECTS_DIR), &mut keys).await?;

        Ok(keys.into_iter().collect())
    }

    /// Lays every object the index holds out in one pack, answering whether anything changed.
    ///
    /// Everything is gathered up (the entries of every pack, and every loose object) and written
    /// again: what was many packs and a scattering of loose files becomes one pack. What each
    /// object *is* does not change, so every read that worked before works after. An index already
    /// holding one pack and nothing loose is left alone and answered with `false`.
    ///
    /// The new pack is written and the old ones dropped only once it is there, so a reader that
    /// arrives in the middle finds every object in the packs it already knew.
    ///
    /// The work is said to `progress` as one task the length of the objects being written, begun
    /// once the enumeration has said how many there are.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Io`] if an object cannot be read or a pack cannot be written, and
    /// [`VCSIndexError::Malformed`] if a path it needs cannot be named.
    pub async fn repack(&self, progress: &Progress) -> Result<bool, VCSIndexError> {
        let loose = self.list_loose_keys().await?;
        let packs = self.packs().await?;

        if loose.is_empty() && packs.len() <= 1 {
            return Ok(false);
        }

        // Everything the index holds, under the key it is held by: a loose file, or the place in
        // a pack it sits at. A key both loose and packed is one entry here, and the loose copy is
        // the one read, the two are the same bytes.
        let mut sources: BTreeMap<Hash, Source> = BTreeMap::new();
        for index in &packs {
            // A pack whose index is gone holds nothing that can be found, so nothing of it is
            // gathered up: the pack it would have been read from is not reachable either way.
            let Some(directory) = self.read_pack_index(*index).await? else {
                continue;
            };

            for entry in directory.entries() {
                sources.insert(entry.key(), Source::Packed(*index, *entry));
            }
        }
        for key in &loose {
            sources.insert(*key, Source::Loose(self.object_path(key)));
        }

        if sources.is_empty() {
            return Ok(false);
        }

        // The writing is the part worth watching, and its length is known only once the
        // enumeration has said how many objects there are, so the task is begun here rather than
        // by the caller, who cannot know the number either.
        let total = u64::try_from(sources.len()).unwrap_or(u64::MAX);
        let mut task = progress.begin("", Some(total));

        // The new pack is written at a number no pack holds, so the old ones are not written over
        // while they are still what a reader would find.
        let mut framed = Vec::new();
        let mut directory = PackIndex::default();
        for (key, source) in &sources {
            let bytes = self.read_source(source).await?;
            let offset = framed.len() as u64;
            let len = bytes.len() as u64;

            framed.extend_from_slice(&bytes);
            directory.push(PackEntry::new(*key, offset, len));
            task.advance_by(1);
        }
        task.finish();

        // The pack is claimed under a number nothing else is writing, and the one it replaces is
        // dropped only once it is whole: a reader that arrives in the middle finds every object in
        // the packs it already knew. The number is not moved back to nothing afterwards, a pack
        // number is a name, and moving one is renaming a file another writer may be claiming.
        let fresh = self.claim_pack().await?;
        tokio::fs::write(self.pack_path(fresh), &framed).await?;
        write_whole_durably(&self.pack_index_path(fresh), &directory.encode()).await?;

        // The objects the pack now holds are not loose any more, and the packs it replaced are
        // gone: what a read finds is the one pack it wrote either way.
        for key in &loose {
            let _ = tokio::fs::remove_file(self.object_path(key)).await;
        }
        for index in &packs {
            drop_pack(self, *index).await?;
        }

        let state = self.pack_state();
        forget_pack_index(&state, fresh);
        forget_packs(&state);

        prune_empty_directories(&self.path.join(super::OBJECTS_DIR)).await;

        Ok(true)
    }

    /// Works out what a collecting run that keeps `keep` would take away.
    ///
    /// Every object the index holds is asked whether the keys handed in name it. What none of them
    /// names is what the plan takes away, with one exception: an object written within the `grace`
    /// is kept all the same, and counted in [`kept_by_grace`](CollectPlan::kept_by_grace)
    /// instead. A loose object carries its own file's modification time, a packed one its pack
    /// file's. The grace is what keeps a run that wrote an object after the references were read
    /// from losing that object; a grace of zero suppresses the keeping, and how long the grace is
    /// is the caller's to say rather than this index's.
    ///
    /// This reads the index and changes nothing, so a run can show what a collecting would do and
    /// ask before it does it. The collecting that carries a plan out enumerates the index again,
    /// so what arrives or vanishes between this and the collecting is the collecting's to find: an
    /// object that arrives is kept, one already gone is skipped.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Io`] if a pack index cannot be read, and
    /// [`VCSIndexError::Malformed`] if it does not decode.
    pub async fn collect_plan(
        &self,
        keep: &BTreeSet<Hash>,
        grace: Duration,
    ) -> Result<CollectPlan, VCSIndexError> {
        let loose = self.list_loose_keys().await?;
        let packs = self.packs().await?;

        // The grace is measured from now, backwards: an object written after this moment less the
        // grace is one the collecting keeps whatever the references say. A clock that cannot
        // answer keeps everything on the grace, which is the safe side of the two mistakes: taking
        // an object that was just written costs the object, keeping one that is truly dead costs
        // only the space until the next collecting run.
        let fresh_since = SystemTime::now().checked_sub(grace);

        let mut plan = CollectPlan::default();
        let mut seen: BTreeSet<Hash> = BTreeSet::new();

        // The loose copy is the one a laying-out reads, so it is the one this plan measures: a key
        // both loose and packed is one entry, and its freshness and its length are the loose
        // file's.
        for key in &loose {
            seen.insert(*key);

            if keep.contains(key) {
                continue;
            }

            let path = self.object_path(key);
            let metadata = tokio::fs::metadata(&path).await;

            let fresh = fresh_since.is_some_and(|since| {
                metadata
                    .as_ref()
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .is_some_and(|modified| modified > since)
            });

            if fresh {
                plan.kept_by_grace += 1;
            } else {
                plan.removed
                    .insert(*key, metadata.map_or(0_u64, |metadata| metadata.len()));
            }
        }

        for index in &packs {
            let Some(directory) = self.read_pack_index(*index).await? else {
                continue;
            };

            for entry in directory.entries() {
                if !seen.insert(entry.key()) {
                    continue;
                }

                if keep.contains(&entry.key()) {
                    continue;
                }

                let fresh = match fresh_since {
                    Some(since) => {
                        self.written_since(&Source::Packed(*index, *entry), since)
                            .await
                    }
                    None => true,
                };

                if fresh {
                    plan.kept_by_grace += 1;
                } else {
                    plan.removed.insert(entry.key(), entry.len());
                }
            }
        }

        Ok(plan)
    }

    /// Whether the object `source` names was written after `since`.
    ///
    /// A loose object carries its own file's modification time, a packed one the pack file's: a
    /// pack written recently counts as recent all through, which keeps a whole pack of fresh
    /// objects rather than asking one file for each of them.
    async fn written_since(&self, source: &Source, since: SystemTime) -> bool {
        let path = match source {
            Source::Loose(path) => path.clone(),
            Source::Packed(index, _) => self.pack_path(*index),
        };

        tokio::fs::metadata(&path)
            .await
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .is_some_and(|modified| modified > since)
    }

    /// The collecting itself: takes away what `plan` says nothing wants, and lays the rest out in
    /// one pack.
    ///
    /// This is [`collect_plan`](Self::collect_plan) carried out, against the index as it stands
    /// when it runs: what was written since the plan was made is kept, what is already gone is
    /// skipped, and the entries the plan names go last. The pack written here is made whole before
    /// any old one is dropped, a reader that arrives in the middle finds every object in the packs
    /// it already knew, the same bargain the laying-out strikes.
    ///
    /// A plan that takes nothing away and an index already one pack and nothing loose is left
    /// alone; a plan that takes nothing away and an index scattered otherwise is laid out as a
    /// laying-out would, taking nothing away.
    ///
    /// The work is said to `progress` as one task the length of the objects being written, begun
    /// once the enumeration has said how many there are.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Io`] if an object cannot be read or a pack cannot be written, and
    /// [`VCSIndexError::Malformed`] if a path it needs cannot be named.
    pub async fn collect(
        &self,
        plan: &CollectPlan,
        progress: &Progress,
    ) -> Result<CollectReport, VCSIndexError> {
        let loose = self.list_loose_keys().await?;
        let packs = self.packs().await?;
        let dead = plan.dead();

        // Everything the index holds, under the key it is held by, kept and taken apart: the plan
        // is executed against the index as it stands, so what arrived since it was made is kept
        // and what is already gone is counted as nothing taken.
        let mut kept: BTreeMap<Hash, Source> = BTreeMap::new();
        let mut taken_len = 0_u64;
        let mut taken = 0_usize;

        for index in &packs {
            let Some(directory) = self.read_pack_index(*index).await? else {
                continue;
            };

            for entry in directory.entries() {
                if dead.contains(&entry.key()) {
                    taken += 1;
                    taken_len = taken_len.saturating_add(entry.len());
                } else {
                    kept.insert(entry.key(), Source::Packed(*index, *entry));
                }
            }
        }
        for key in &loose {
            if dead.contains(key) {
                // A loose object's length is its file's, which is the whole framed entry.
                let len = tokio::fs::metadata(self.object_path(key))
                    .await
                    .map_or(0_u64, |metadata| metadata.len());
                taken += 1;
                taken_len = taken_len.saturating_add(len);
            } else {
                kept.insert(*key, Source::Loose(self.object_path(key)));
            }
        }

        // A plan that takes nothing away and an index already one pack and nothing loose is left
        // alone rather than written over as it stands.
        if dead.is_empty() && loose.is_empty() && packs.len() <= 1 {
            return Ok(CollectReport {
                kept_by_grace: plan.kept_by_grace,
                ..CollectReport::default()
            });
        }

        // The writing is the part worth watching, and its length is known only once the
        // enumeration has said how many objects there are, so the task is begun here rather than
        // by the caller, who cannot know the number either.
        let total = u64::try_from(kept.len()).unwrap_or(u64::MAX);
        let mut task = progress.begin("", Some(total));

        // What is kept is laid down as one pack, at a number no pack holds, so the old ones are
        // not written over while they are still what a reader would find. An index keeping nothing
        // writes none: its packs are taken away whole below, which is the collecting it asked for.
        let fresh = if kept.is_empty() {
            None
        } else {
            let mut framed = Vec::new();
            let mut directory = PackIndex::default();
            for (key, source) in &kept {
                let bytes = self.read_source(source).await?;
                let offset = framed.len() as u64;
                let len = bytes.len() as u64;

                framed.extend_from_slice(&bytes);
                directory.push(PackEntry::new(*key, offset, len));
                task.advance_by(1);
            }

            let claimed = self.claim_pack().await?;
            tokio::fs::write(self.pack_path(claimed), &framed).await?;
            write_whole_durably(&self.pack_index_path(claimed), &directory.encode()).await?;

            Some(claimed)
        };
        task.finish();

        // What the pack now holds is not loose any more, and what the plan took away goes with the
        // packs that held it: a loose object has only this removal to go by, one already gone
        // drops as nothing.
        for key in &loose {
            let _ = tokio::fs::remove_file(self.object_path(key)).await;
        }
        for index in &packs {
            drop_pack(self, *index).await?;
        }

        if let Some(fresh) = fresh {
            // The number the fresh pack was claimed under is one nothing has read yet, and the
            // listing that named the old packs is no longer the truth of the directory.
            let state = self.pack_state();
            forget_pack_index(&state, fresh);
            forget_packs(&state);
        }

        prune_empty_directories(&self.path.join(super::OBJECTS_DIR)).await;

        Ok(CollectReport {
            removed: taken,
            removed_len: taken_len,
            kept_by_grace: plan.kept_by_grace,
            changed: true,
        })
    }

    /// Lays the loose objects, or the packs, out in one pack, when there are enough of them to be
    /// worth it.
    ///
    /// A run of this index writes its objects as one pack already, so what gathers between packings
    /// is a pack per run rather than a loose object per object: what that costs is a walk of one more
    /// pack for every lookup, which a handful is worth and a hundred is not. So a run packs when the
    /// loose objects it has left number `loose`, or when it has left more than `packs` packs behind,
    /// whichever comes first; until then what it wrote is valid where it is, and a later run or
    /// `rola pack` settles it.
    ///
    /// # Errors
    ///
    /// Returns what [`repack`](Self::repack) does.
    pub async fn repack_when_due(&self, loose: usize, packs: usize) -> Result<bool, VCSIndexError> {
        let gathered = self.list_loose_keys().await?.len() >= loose;
        let scattered = self.packs().await?.len() > packs;

        if !gathered && !scattered {
            return Ok(false);
        }

        // A laying-out that is a step of a write nobody asked to watch goes unsaid: the run doing
        // the writing is the one a reader is watching, and this work is part of that rather than a
        // show of its own.
        let progress = Progress::silent();

        self.repack(&progress).await
    }

    /// Drops the object stored under `key`, whether it is loose or in a pack.
    ///
    /// A packed entry is taken away by unpacking the pack it sits in without it, and dropping the
    /// pack: the kept entries are written loose first, so nothing is dropped before its bytes are
    /// somewhere else.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::NotFound`] if nothing is stored under `key`.
    pub(crate) async fn remove_object_packed(&self, key: &Hash) -> Result<(), VCSIndexError> {
        if let Some(Source::Packed(index, _)) = self.find_packed(key).await? {
            self.unpack_without(index, key).await?;
            return Ok(());
        }

        Err(VCSIndexError::NotFound(*key))
    }

    /// Writes every entry of the pack of `index` loose, but `removed`, and drops the pack.
    async fn unpack_without(&self, index: u64, removed: &Hash) -> Result<(), VCSIndexError> {
        // A pack whose index is gone has no entries to write out, and nothing is left to read them
        // from: the pack it belonged to is dropped either way.
        if let Some(directory) = self.read_pack_index(index).await? {
            for entry in directory.entries() {
                if entry.key() == *removed {
                    continue;
                }

                let bytes = read_range(&self.pack_path(index), entry.offset(), entry.len()).await?;
                write_whole_durably(&self.object_path(&entry.key()), &bytes).await?;
            }
        }

        drop_pack(self, index).await
    }
}

/// A run of objects being written as one pack.
///
/// A sync takes many objects at once, and writing each one loose is a file made, a rename and a
/// directory entry of its own, where one pack is one file and one index for the whole run. Nothing
/// the batch takes is visible until [`publish`](Self::publish): a pack is found by its index, and the
/// index is written after the pack it names.
///
/// What it takes is what a transfer takes, an object checked against the key it arrived under, or
/// an object this index made, which is keyed by itself already.
pub struct VCSIndexBatch {
    /// The index the pack is written into.
    index: VCSIndex,
    /// The framed entries, as they will sit in the pack.
    framed: Vec<u8>,
    /// Which objects it holds, and where each sits.
    directory: PackIndex,
    /// The keys it has taken, so one key arriving twice is one entry rather than two.
    written: BTreeSet<Hash>,
}

impl VCSIndex {
    /// Groups writes so that what they make is laid down as one pack.
    ///
    /// It is what a transfer takes its objects through: one file and one index for a whole run
    /// rather than a file and an index for each object.
    #[must_use]
    pub fn batch(&self) -> VCSIndexBatch {
        VCSIndexBatch {
            index: self.clone(),
            framed: Vec::new(),
            directory: PackIndex::default(),
            written: BTreeSet::new(),
        }
    }

    /// The number of the next pack nothing else is writing: claimed by creating its file.
    ///
    /// Claiming is creating, so two writers, in this process or in another, never take one number
    /// for two packs: the second to try is told the file is there and takes the one after. What is
    /// claimed is an empty file until the caller writes it, and nothing finds it until the index
    /// beside it is written, since a pack is found by its index.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Io`] if the pack directory cannot be made or a file cannot be
    /// created.
    async fn claim_pack(&self) -> Result<u64, VCSIndexError> {
        tokio::fs::create_dir_all(self.path.join(PACKED_DIR)).await?;

        let mut index = self
            .packs()
            .await?
            .last()
            .map_or(0, |last| last.saturating_add(1));

        loop {
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.pack_path(index))
                .await
            {
                Ok(_) => return Ok(index),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    index = index.saturating_add(1);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl VCSIndexBatch {
    /// How many objects it holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.directory.len()
    }

    /// Whether it holds no object.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.directory.is_empty()
    }

    /// Takes `content` as the object stored under `key`, which is the hash it has to come to.
    ///
    /// It is what [`accept`](rorolala_storage::TransferableBackend::accept) does with one object,
    /// done for a run of them: what arrived is decoded, held to the key it came under, and written
    /// as its canonical encoding, a key names one meaning, so one spelling of it is what is kept.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Malformed`] if `content` does not read as an index object, and
    /// [`VCSIndexError::Corrupt`] if it does not hash to `key`.
    pub fn accept(&mut self, key: &Hash, content: &[u8]) -> Result<(), VCSIndexError> {
        let object = VCSIndexObject::decode(content)?;
        if object.hash() != *key {
            return Err(VCSIndexError::Corrupt(*key));
        }

        self.put(key, &object.encode());

        Ok(())
    }

    /// Takes `object`, and answers with the key it is written under.
    ///
    /// It is the write a run that made the object wants: the key is the object's own hash, so the
    /// same object taken twice is one entry.
    pub fn add<W: VCSWrite + ?Sized>(&mut self, object: &W) -> Hash {
        let key = object.hash();

        self.put(&key, &object.encode());

        key
    }

    /// Writes the objects down as one pack, and answers how many were written.
    ///
    /// The pack is claimed under a number nothing else is writing, written whole, and only then
    /// given its index, so a reader finds the pack that was here before this run until the one this
    /// run wrote is whole.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::Io`] if the pack or its index cannot be written.
    pub async fn publish(self) -> Result<usize, VCSIndexError> {
        let written = self.directory.len();
        if written == 0 {
            return Ok(0);
        }

        let index = self.index.claim_pack().await?;
        tokio::fs::write(self.index.pack_path(index), &self.framed).await?;
        write_whole_durably(&self.index.pack_index_path(index), &self.directory.encode()).await?;

        // The directory has a pack it did not have a moment ago, and the number it was claimed under
        // is one nothing has read yet.
        let state = self.index.pack_state();
        forget_pack_index(&state, index);
        forget_packs(&state);

        Ok(written)
    }

    /// Frames `plain` as an entry of the pack and adds it, unless the key is already here.
    fn put(&mut self, key: &Hash, plain: &[u8]) {
        if !self.written.insert(*key) {
            return;
        }

        let frame = Frame {
            version: FRAME_VERSION,
            codec: Codec::Raw,
            layout: Layout::Single,
            plain_len: plain.len() as u64,
        };
        let offset = self.framed.len() as u64;

        self.framed.extend_from_slice(&frame.encode());
        self.framed.extend_from_slice(plain);

        self.directory.push(PackEntry::new(
            *key,
            offset,
            self.framed.len() as u64 - offset,
        ));
    }
}

/// The pack number an index's name stands for, if it is one this build writes.
fn index_of_pack(name: &str) -> Option<u64> {
    name.strip_prefix("packed_")
        .and_then(|rest| rest.strip_suffix(".idx"))
        .and_then(|number| number.parse().ok())
        .filter(|index| name == format!("packed_{index}.idx"))
}

/// What an index collecting run would take away, worked out before anything is asked of the caller.
///
/// The plan is read-only work: it enumerates the index, asks each object whether the keys the caller
/// handed in name it, and holds what is left over. Making one changes nothing, so a run can show
/// what a collecting would do and ask before it does it, which is what a taking-away that cannot be
/// undone owes the caller.
#[derive(Clone, Default)]
pub struct CollectPlan {
    /// The keys the plan takes away, each with how long its entry is, frames included.
    removed: BTreeMap<Hash, u64>,
    /// How many objects the plan keeps although no key the caller handed in names them, because
    /// they were written recently enough to sit inside the grace.
    kept_by_grace: usize,
}

impl CollectPlan {
    /// How many objects the plan takes away.
    #[must_use]
    pub fn removed(&self) -> usize {
        self.removed.len()
    }

    /// How many bytes the objects the plan takes away occupy, frames included.
    #[must_use]
    pub fn removed_len(&self) -> u64 {
        self.removed
            .values()
            .fold(0_u64, |all, len| all.saturating_add(*len))
    }

    /// How many objects the plan keeps on the grace alone.
    ///
    /// These are objects no key the caller handed in names, but which were written recently enough
    /// that taking them could take a write that is still on its way: they are kept, and the next
    /// collecting run decides them on the references it sees.
    #[must_use]
    pub const fn kept_by_grace(&self) -> usize {
        self.kept_by_grace
    }

    /// The keys the plan takes away, so the collecting that carries this plan out knows what to
    /// leave out of the pack it writes.
    fn dead(&self) -> BTreeSet<Hash> {
        self.removed.keys().copied().collect()
    }
}

/// What an index collecting run took away, and whether it changed anything.
#[derive(Clone, Copy, Default)]
pub struct CollectReport {
    /// How many objects were taken away, as the collecting found them: one already gone between
    /// the plan and the collecting is not counted, there is nothing left of it.
    removed: usize,
    /// How many bytes the objects taken away occupy, frames included.
    removed_len: u64,
    /// How many objects were kept on the grace alone, as the plan counted them.
    kept_by_grace: usize,
    /// Whether the index was changed: a pack written, old ones dropped, objects taken away.
    changed: bool,
}

impl CollectReport {
    /// How many objects were taken away.
    #[must_use]
    pub const fn removed(&self) -> usize {
        self.removed
    }

    /// How many bytes the objects taken away occupy, frames included.
    #[must_use]
    pub const fn removed_len(&self) -> u64 {
        self.removed_len
    }

    /// How many objects were kept on the grace alone.
    #[must_use]
    pub const fn kept_by_grace(&self) -> usize {
        self.kept_by_grace
    }

    /// Whether anything changed: a collecting that took nothing away and found the index already
    /// one pack and nothing loose changed nothing.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

/// Drops the pack of `index` and its index, the index first.
async fn drop_pack(index: &VCSIndex, number: u64) -> Result<(), VCSIndexError> {
    for path in [index.pack_index_path(number), index.pack_path(number)] {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    // What was read of the pack is dropped with it: a number is given out again once the pack that
    // had it is gone, and a stamp would not tell a fresh file from the one that had that number.
    let state = index.pack_state();
    forget_pack_index(&state, number);
    forget_packs(&state);

    Ok(())
}

/// The `len` bytes of the file at `path` starting `offset` in.
///
/// A packed entry is read straight out of its pack rather than by reading the pack whole: a pack
/// is as big as everything in it, and a read that wanted one object should not pay for the rest.
async fn read_range(
    path: &std::path::Path,
    offset: u64,
    len: u64,
) -> Result<Vec<u8>, VCSIndexError> {
    use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _};

    let len = usize::try_from(len).map_err(|_| VCSIndexError::Malformed)?;
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(io::SeekFrom::Start(offset)).await?;

    let mut bytes = vec![0_u8; len];
    file.read_exact(&mut bytes).await?;

    Ok(bytes)
}

/// Writes `bytes` to `path` whole, laid down beside it and moved into place.
///
/// The temporary is named so no two writes in this process share one, so two of them writing a
/// pack and its index never write through each other's file.
async fn write_whole_durably(path: &std::path::Path, bytes: &[u8]) -> Result<(), VCSIndexError> {
    let parent = path.parent().ok_or(VCSIndexError::Malformed)?;
    tokio::fs::create_dir_all(parent).await?;

    let temporary = super::temporary_beside(path)?;
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(&temporary, path).await?;

    Ok(())
}

/// Writes `bytes` to `path` whole, beside it and moved into place, making the directory it is in.
///
/// This is for a caller's own file, what `extract_file` hands back, rather than for an entry the
/// index reads back, so nothing is promised to outlive a crash. What is kept is that a failed write
/// leaves the target as it was rather than half written.
pub(super) async fn write_whole(path: &std::path::Path, bytes: &[u8]) -> Result<(), VCSIndexError> {
    let parent = path.parent().ok_or(VCSIndexError::Malformed)?;
    tokio::fs::create_dir_all(parent).await?;

    let temporary = super::temporary_beside(path)?;
    tokio::fs::write(&temporary, bytes).await?;

    // A move that fails (the target is a directory, or on another filesystem) leaves the written
    // temporary beside it, which nothing will ever read. Dropping it keeps a failed extract from
    // costing space every time it is tried.
    if let Err(error) = tokio::fs::rename(&temporary, path).await {
        let _ = tokio::fs::remove_file(&temporary).await;

        return Err(error.into());
    }

    Ok(())
}

/// Removes the shard directories under `directory` that no object is left in.
async fn prune_empty_directories(directory: &std::path::Path) {
    let Ok(firsts) = entries(directory).await else {
        return;
    };

    for first in firsts {
        let Ok(seconds) = entries(&first).await else {
            continue;
        };

        for second in seconds {
            let _ = tokio::fs::remove_dir(&second).await;
        }

        let _ = tokio::fs::remove_dir(&first).await;
    }
}

/// What this process keeps for one index root, shared by every index of it.
///
/// It is by root rather than by index because an index is handed out freely, `at`, `create` and
/// `locate` each make one for a directory, and two of them still describe one set of packs, so they
/// have to share one view of what has been read. A running daemon reaches the same Workspace's index
/// from one connection to the next, which is where a lookup that is answered from here stops paying
/// for the same file again.
#[derive(Default)]
struct PackState {
    /// The pack indexes that have been read, by pack number.
    indices: Mutex<HashMap<u64, KeptIndex>>,
    /// The packs as they were last listed, and what the directory looked like then.
    packs: Mutex<Option<KeptPacks>>,
}

/// The packs as they were last listed, and what the directory looked like then.
struct KeptPacks {
    /// How the directory looked when it was walked, which is how a changed one is told.
    stamp: Stamp,
    /// The pack numbers, lowest first, shared so that handing them out copies a pointer.
    packs: Arc<Vec<u64>>,
}

/// A pack index that has been read, and what its file looked like when it was.
struct KeptIndex {
    /// How the file looked when it was read, which is how a changed one is told.
    stamp: Stamp,
    /// The index itself, shared so that handing it out copies a pointer rather than the list.
    index: Arc<PackIndex>,
}

/// What says one version of a file from another without reading it.
///
/// A length and a modification time rather than either alone: an index is rewritten to name another
/// set of entries rather than appended to, so a rewrite changes how long it is, and the time agrees
/// with that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    /// When the file was last written, where the filesystem tells it.
    modified: Option<SystemTime>,
    /// How long the file is.
    len: u64,
}

/// The stamp of `metadata`.
fn stamp_of(metadata: &std::fs::Metadata) -> Stamp {
    Stamp {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    }
}

/// Drops what was read of the pack of `index`, since there is no pack there to have read.
///
/// It is dropped whenever the index is gone, so that a pack number later given to another pack is not
/// answered with what the last one held: a number is given out again once its pack is dropped, and an
/// index read as a stale one would name entries at offsets that no longer mean them.
fn forget_pack_index(state: &PackState, index: u64) {
    state
        .indices
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&index);
}

/// Drops the list of packs, since the directory it was read from has changed.
fn forget_packs(state: &PackState) {
    *state.packs.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

impl VCSIndex {
    /// What this process keeps of this index's packs, shared with every index of the same root.
    fn pack_state(&self) -> Arc<PackState> {
        let key = std::fs::canonicalize(&self.path)
            .or_else(|_| std::path::absolute(&self.path))
            .unwrap_or_else(|_| self.path.clone());

        let mut states = pack_states().lock().unwrap_or_else(PoisonError::into_inner);

        Arc::clone(states.entry(key).or_default())
    }
}

/// The state this process keeps for each index root, by the root it is for.
///
/// An entry is kept for the life of the process once it is handed out, so the map grows with the
/// number of distinct indexes this process has read, not with the number of objects it has read.
/// That is a handful of workspaces and vaults for a daemon, which is why nothing drops them: state
/// that could be dropped under a running lookup is worse than state that stays.
fn pack_states() -> &'static Mutex<HashMap<PathBuf, Arc<PackState>>> {
    /// Every root this process has kept state for.
    static STATES: OnceLock<Mutex<HashMap<PathBuf, Arc<PackState>>>> = OnceLock::new();

    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

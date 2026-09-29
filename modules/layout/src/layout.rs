//! The layout: which path names which `Uuid`, and what is kept for each.
//!
//! A layout is the one thing about a version control that is *not* immutable: everything else is
//! content-addressed and never changes, while a layout says what the work is made of right now.
//! So it is kept differently from everything else — as a log of changes appended to, replayed into
//! memory, and answered from there.
//!
//! # How it is kept
//!
//! Changes are spread over a fixed number of **shards**, and the shard a change belongs to is
//! decided by the `Uuid` it names. Everything about one `Uuid` — the path it is at and what it
//! holds — is in that one shard, which is what lets a shard be written down on its own. Each shard
//! is a log a change is appended to with a single write and then flushed, so a change is on disk by
//! the time the call returns, and two writers — in one process or in several — appending at once
//! cannot tear each other's record. Nothing is locked across a write, and no writer waits for
//! another except one appending to the same shard.
//!
//! Each shard also has a **snapshot**: everything it holds, written down whole, so the log that led
//! to it can be dropped. [`Layout::compact`] reads what a shard's log still holds into the snapshot
//! and empties the log, one shard at a time — a shard being compacted is the only one not being
//! written, so a compaction never stops the others. Emptying a log leaves no reader safe to read on
//! from where it had got to, so a compaction **ages** the log: a reader holding an offset into the
//! age before it starts again from the beginning.
//!
//! Reads are answered from memory. Opening the layout reads every shard's snapshot and replays its
//! log into memory, and [`Layout::refresh`] replays whatever has been appended since — so a reader
//! that wants what another process has just written calls it, and a reader that does not pays
//! nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use uuid::Uuid;

use crate::data::MutableData;
use crate::error::LayoutError;
use crate::log::ShardLog;
use crate::path::LayoutPath;
use crate::record::Record;
use crate::slot::Slot;

/// How many shards a layout's changes are spread over.
///
/// It is fixed, since it decides which file a change is written to; a shard per `Uuid` group keeps
/// two writers apart unless they are changing the same group.
const SHARD_COUNT: usize = 16;

/// What a shard's log and snapshot are named.
const LOG_PREFIX: &str = "log-";
const LOG_SUFFIX: &str = ".wal";
const SNAPSHOT_PREFIX: &str = "snap-";
const SNAPSHOT_SUFFIX: &str = ".bin";

/// A layout, kept in a directory and answered from memory.
pub struct Layout {
    /// The directory the layout is kept in.
    dir: PathBuf,

    /// A shard per group of `Uuid`s: its log, its snapshot in memory, and the `Uuid`s it holds.
    shards: Vec<RwLock<Shard>>,

    /// Which path names which `Uuid`, kept in step with the shards.
    index: RwLock<Index>,
}

/// One shard: the log of its changes, and everything about the `Uuid`s in it.
struct Shard {
    /// The log this shard's changes are appended to.
    log: ShardLog,

    /// What each `Uuid` in this shard is: its path and what it holds.
    slots: HashMap<Uuid, Slot>,
}

/// Which path names which `Uuid`, and the same backwards.
///
/// It is derived from the shards and kept in step with them rather than stored on its own: a `Uuid`
/// is at one path, and a path names one `Uuid`.
#[derive(Default)]
struct Index {
    /// The `Uuid` each path names.
    id_of: HashMap<LayoutPath, Uuid>,
    /// The path each `Uuid` is at.
    path_of: HashMap<Uuid, LayoutPath>,
}

/// What a layout holds that is half-made: entries no path names, and paths that name none.
///
/// Both are legitimate between the two changes a create is, so neither is an error. They are worth
/// seeing, though, because one that never resolves is a create that did not finish.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Orphans {
    /// Entries (`Uuid`s) no path names.
    pub without_path: Vec<Uuid>,
    /// Paths whose `Uuid` holds nothing.
    pub without_entry: Vec<LayoutPath>,
}

impl Layout {
    /// Opens the layout kept in `dir`, making the directory and its shards if they are not there.
    ///
    /// Every shard's snapshot is read and its log replayed into memory, so what is answered
    /// afterwards is the layout as the two describe it at the moment it was opened.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Io`] if a file could not be made or read, and [`LayoutError::Malformed`]
    /// if one holds bytes this build cannot read.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, LayoutError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let mut shards = Vec::with_capacity(SHARD_COUNT);
        for shard in 0..SHARD_COUNT {
            let log = ShardLog::open(&dir.join(format!("{LOG_PREFIX}{shard}{LOG_SUFFIX}")))?;
            let slots = crate::snapshot::read(&snapshot_path(&dir, shard))?.unwrap_or_default();
            shards.push(RwLock::new(Shard { log, slots }));
        }

        let index = derived_index(&shards);
        let layout = Self {
            dir,
            shards,
            index: RwLock::new(index),
        };
        layout.refresh()?;

        Ok(layout)
    }

    /// The directory the layout is kept in.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Reads and applies every change appended since the last read.
    ///
    /// It is what makes a reader see what another writer has just written, and what opening does to
    /// read the logs back. A record that is not all there — a write a crash cut short — stops the
    /// read where it began rather than failing, so what was written whole is what is applied.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Io`] if a log could not be read, and [`LayoutError::Malformed`] if a
    /// whole record on disk is not one this build wrote.
    pub fn refresh(&self) -> Result<(), LayoutError> {
        for shard in 0..SHARD_COUNT {
            let records = {
                let mut held = lock_write(&self.shards[shard]);
                let bytes = held.log.poll()?;

                let mut records = Vec::new();
                let read = replay(&bytes, &mut held.slots, &mut records)?;
                held.log.advance(read as u64);

                records
            };

            let mut index = lock_write(&self.index);
            for record in &records {
                apply_index(&mut index, record)?;
            }
        }

        Ok(())
    }

    /// Writes every shard down whole and empties its log.
    ///
    /// One shard is compacted at a time, each under its own lock, so a writer changing one shard is
    /// never held up by another shard being compacted. What a shard is compacted from is its own
    /// snapshot and log, so nothing another shard does reaches it.
    ///
    /// A shard's log is locked exclusively for the whole of its compaction — reading what it still
    /// holds, writing the snapshot down, raising the age and emptying it. Appending locks the log
    /// too, so a writer in this process or another is held up only while the shard it is writing to
    /// is being compacted, never globally; and because what is still in the log is folded into the
    /// snapshot before the log is emptied, a writer's change is never one the compaction did not see.
    ///
    /// # Errors
    ///
    /// Returns why a snapshot could not be written or a log could not be emptied.
    pub fn compact(&self) -> Result<(), LayoutError> {
        for shard in 0..SHARD_COUNT {
            let mut held = lock_write(&self.shards[shard]);

            let mut done = Vec::new();
            {
                let Shard { log, slots } = &mut *held;
                log.compact(|remaining| {
                    replay(remaining, slots, &mut done)?;
                    crate::snapshot::write(&snapshot_path(&self.dir, shard), slots)
                })?;
            }

            drop(held);
            let mut index = lock_write(&self.index);
            for record in &done {
                apply_index(&mut index, record)?;
            }
        }

        Ok(())
    }

    /// Binds `path` to `id`.
    ///
    /// A `Uuid` is at one path, so binding it somewhere else moves it rather than duplicating it:
    /// the path it was at stops naming it.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::AlreadyExists`] if `path` already names a `Uuid`, and whatever
    /// reading or writing the change fails with.
    pub fn create_path(&self, path: &LayoutPath, id: Uuid) -> Result<(), LayoutError> {
        self.refresh()?;
        if lock_read(&self.index).id_of.contains_key(path) {
            return Err(LayoutError::AlreadyExists);
        }

        self.change(&Record::CreatePath {
            id,
            path: path.as_str().to_owned(),
        })
    }

    /// Stops `path` naming whatever it names.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if `path` names nothing, and whatever reading or writing
    /// the change fails with.
    pub fn remove_path(&self, path: &LayoutPath) -> Result<(), LayoutError> {
        self.refresh()?;
        let id = lock_read(&self.index)
            .id_of
            .get(path)
            .copied()
            .ok_or(LayoutError::NotFound)?;

        self.change(&Record::RemovePath {
            id,
            path: path.as_str().to_owned(),
        })
    }

    /// Moves the `Uuid` at `from` to `to`.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if `from` names nothing, [`LayoutError::AlreadyExists`] if
    /// `to` already names a `Uuid`, and whatever reading or writing the change fails with.
    pub fn move_path(&self, from: &LayoutPath, to: &LayoutPath) -> Result<(), LayoutError> {
        self.refresh()?;
        let id = {
            let index = lock_read(&self.index);
            let id = index
                .id_of
                .get(from)
                .copied()
                .ok_or(LayoutError::NotFound)?;
            if index.id_of.contains_key(to) {
                return Err(LayoutError::AlreadyExists);
            }
            drop(index);
            id
        };

        self.change(&Record::MovePath {
            id,
            from: from.as_str().to_owned(),
            to: to.as_str().to_owned(),
        })
    }

    /// Creates the entry `id` with `data`.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::AlreadyExists`] if `id` already holds something, and whatever reading
    /// or writing the change fails with.
    pub fn create_entry(&self, id: Uuid, data: MutableData) -> Result<(), LayoutError> {
        self.refresh()?;
        if self.held_data(id).is_some() {
            return Err(LayoutError::AlreadyExists);
        }

        self.change(&Record::CreateEntry { id, data })
    }

    /// Changes the entry `id` to hold `data`.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if `id` holds nothing, and whatever reading or writing the
    /// change fails with.
    pub fn update_entry(&self, id: Uuid, data: MutableData) -> Result<(), LayoutError> {
        self.refresh()?;
        if self.held_data(id).is_none() {
            return Err(LayoutError::NotFound);
        }

        self.change(&Record::UpdateEntry { id, data })
    }

    /// Drops the entry `id`, and the path that named it.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if `id` holds nothing, and whatever reading or writing the
    /// change fails with.
    pub fn remove_entry(&self, id: Uuid) -> Result<(), LayoutError> {
        self.refresh()?;
        if self.held_data(id).is_none() {
            return Err(LayoutError::NotFound);
        }

        self.change(&Record::RemoveEntry { id })
    }

    /// Empties the layout: every entry is dropped, and every path stops naming anything.
    ///
    /// It is what leaving a layout with nothing in it means — a place that is still there to be
    /// worked in rather than one that is gone. What it costs is a change per entry, since a log of
    /// changes takes something back one change at a time.
    ///
    /// # Errors
    ///
    /// Returns whatever reading or writing a change fails with.
    pub fn clear(&self) -> Result<(), LayoutError> {
        for (id, _) in self.entries() {
            self.remove_entry(id)?;
        }

        // A path that named no entry is not taken by dropping entries — nothing named it — so it
        // is cleared on its own, leaving nothing at all pointing at a place.
        for (path, _) in self.paths() {
            self.remove_path(&path)?;
        }

        Ok(())
    }

    /// The `Uuid` `path` names, if it names one.
    #[must_use]
    pub fn id_of(&self, path: &LayoutPath) -> Option<Uuid> {
        lock_read(&self.index).id_of.get(path).copied()
    }

    /// The path `id` is at, if it is at one.
    #[must_use]
    pub fn path_of(&self, id: Uuid) -> Option<LayoutPath> {
        lock_read(&self.index).path_of.get(&id).cloned()
    }

    /// What `id` holds, if it holds anything.
    #[must_use]
    pub fn entry(&self, id: Uuid) -> Option<MutableData> {
        self.held_data(id)
    }

    /// What the `Uuid` `path` names holds, if `path` names one that holds anything.
    #[must_use]
    pub fn data_of(&self, path: &LayoutPath) -> Option<MutableData> {
        self.entry(self.id_of(path)?)
    }

    /// Every path, with the `Uuid` it names, in no particular order.
    #[must_use]
    pub fn paths(&self) -> Vec<(LayoutPath, Uuid)> {
        let mut all = Vec::new();
        for shard in &self.shards {
            for (id, slot) in &lock_read(shard).slots {
                if let Some(path) = &slot.path {
                    all.push((path.clone(), *id));
                }
            }
        }

        all
    }

    /// Every entry, with what it holds, in no particular order.
    #[must_use]
    pub fn entries(&self) -> Vec<(Uuid, MutableData)> {
        let mut all = Vec::new();
        for shard in &self.shards {
            for (id, slot) in &lock_read(shard).slots {
                if let Some(data) = &slot.data {
                    all.push((*id, data.clone()));
                }
            }
        }

        all
    }

    /// What is half-made: entries no path names, and paths that name no entry.
    #[must_use]
    pub fn orphans(&self) -> Orphans {
        let mut without_path = Vec::new();
        let mut without_entry = Vec::new();
        for shard in &self.shards {
            for (id, slot) in &lock_read(shard).slots {
                match (&slot.path, &slot.data) {
                    (None, Some(_)) => without_path.push(*id),
                    (Some(path), None) => without_entry.push(path.clone()),
                    _ => {}
                }
            }
        }

        Orphans {
            without_path,
            without_entry,
        }
    }

    /// What `id` holds, read from the shard it belongs to.
    fn held_data(&self, id: Uuid) -> Option<MutableData> {
        lock_read(&self.shards[shard_of(id)])
            .slots
            .get(&id)
            .and_then(|slot| slot.data.clone())
    }

    /// Appends `record` to the log of the shard its `Uuid` belongs to, and applies it.
    ///
    /// The append is what makes the change durable; applying it is what makes it visible here. A
    /// change another process appended is not this call's to apply — [`refresh`](Self::refresh) is.
    fn change(&self, record: &Record) -> Result<(), LayoutError> {
        lock_write(&self.shards[shard_of(record.id())])
            .log
            .append(&record.encode())?;

        self.apply(record)
    }

    /// Makes `record` true in memory.
    ///
    /// Every kind of change is idempotent, so a record read back a second time — which happens when
    /// a change this process wrote is met again on the next [`refresh`](Self::refresh) — does
    /// nothing the second time.
    fn apply(&self, record: &Record) -> Result<(), LayoutError> {
        {
            let mut held = lock_write(&self.shards[shard_of(record.id())]);
            apply_slot(&mut held.slots, record)?;
        }

        let mut index = lock_write(&self.index);
        apply_index(&mut index, record)?;

        Ok(())
    }
}

/// Applies every record `bytes` holds from its front, up to one that is not all there.
///
/// Stops where a record that is not whole begins rather than failing, since that is what a write a
/// crash cut short leaves, and returns how many bytes it read so the caller can count them consumed.
fn replay(
    bytes: &[u8],
    slots: &mut HashMap<Uuid, Slot>,
    done: &mut Vec<Record>,
) -> Result<usize, LayoutError> {
    let mut at = 0_usize;
    while at < bytes.len() {
        match Record::decode(&bytes[at..])? {
            Some((record, used)) => {
                apply_slot(slots, &record)?;
                done.push(record);
                at += used;
            }
            None => break,
        }
    }

    Ok(at)
}

/// Makes the slot part of `record` true.
fn apply_slot(slots: &mut HashMap<Uuid, Slot>, record: &Record) -> Result<(), LayoutError> {
    match record {
        Record::CreatePath { id, path } => {
            slots.entry(*id).or_default().path = Some(LayoutPath::new(path)?);
        }
        Record::RemovePath { id, .. } => {
            if let Some(slot) = slots.get_mut(id) {
                slot.path = None;
                if slot.is_empty() {
                    slots.remove(id);
                }
            }
        }
        Record::MovePath { id, to, .. } => {
            slots.entry(*id).or_default().path = Some(LayoutPath::new(to)?);
        }
        Record::CreateEntry { id, data } | Record::UpdateEntry { id, data } => {
            slots.entry(*id).or_default().data = Some(data.clone());
        }
        Record::RemoveEntry { id } => {
            slots.remove(id);
        }
    }

    Ok(())
}

/// Makes the index part of `record` true.
fn apply_index(index: &mut Index, record: &Record) -> Result<(), LayoutError> {
    match record {
        Record::CreatePath { id, path } => index.bind(LayoutPath::new(path)?, *id),
        Record::RemovePath { id, path } => index.unbind(&LayoutPath::new(path)?, *id),
        Record::MovePath { id, from, to } => {
            index.rename(*id, &LayoutPath::new(from)?, &LayoutPath::new(to)?);
        }
        Record::RemoveEntry { id } => index.forget(*id),
        Record::CreateEntry { .. } | Record::UpdateEntry { .. } => {}
    }

    Ok(())
}

/// The index the shards already describe, built by reading them.
fn derived_index(shards: &[RwLock<Shard>]) -> Index {
    let mut index = Index::default();
    for shard in shards {
        for (id, slot) in &lock_read(shard).slots {
            if let Some(path) = &slot.path {
                index.bind(path.clone(), *id);
            }
        }
    }

    index
}

impl Index {
    /// Makes `path` name `id`, and `id` be at `path`.
    fn bind(&mut self, path: LayoutPath, id: Uuid) {
        if let Some(previous) = self.path_of.insert(id, path.clone())
            && previous != path
        {
            self.id_of.remove(&previous);
        }
        self.id_of.insert(path, id);
    }

    /// Makes `path` stop naming `id`, when it is `id` it names.
    fn unbind(&mut self, path: &LayoutPath, id: Uuid) {
        self.id_of.remove(path);
        if self.path_of.get(&id) == Some(path) {
            self.path_of.remove(&id);
        }
    }

    /// Makes `id` be at `to` rather than at `from`.
    fn rename(&mut self, id: Uuid, from: &LayoutPath, to: &LayoutPath) {
        self.id_of.remove(from);
        if let Some(previous) = self.path_of.insert(id, to.clone())
            && previous != *to
        {
            self.id_of.remove(&previous);
        }
        self.id_of.insert(to.clone(), id);
    }

    /// Makes `id` be at no path.
    fn forget(&mut self, id: Uuid) {
        if let Some(path) = self.path_of.remove(&id) {
            self.id_of.remove(&path);
        }
    }
}

/// The shard a `Uuid`'s changes belong to.
fn shard_of(id: Uuid) -> usize {
    usize::from(id.as_bytes()[0]) % SHARD_COUNT
}

/// Where a shard's snapshot is kept.
fn snapshot_path(dir: &Path, shard: usize) -> PathBuf {
    dir.join(format!("{SNAPSHOT_PREFIX}{shard}{SNAPSHOT_SUFFIX}"))
}

/// Takes a read lock, recovering from a holder that panicked.
fn lock_read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

/// Takes a write lock, recovering from a holder that panicked.
fn lock_write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use uuid::Uuid;

    use super::Layout;
    use crate::data::MutableData;
    use crate::error::LayoutError;
    use crate::path::LayoutPath;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-layout-{}-{label}-{}",
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

    /// Data an entry is made with.
    fn data(seed: u8) -> MutableData {
        MutableData::new(Some("alice".to_owned()), [seed; 32], "first".to_owned())
    }

    /// The seed a `Uuid` is written down with, so a test can say what an entry should hold.
    fn seed_of(id: Uuid) -> u8 {
        u8::try_from(id.as_u128() % 256).unwrap_or_default()
    }

    #[test]
    fn a_path_and_an_entry_are_bound_and_read_back() {
        let layout = Layout::open(scratch("bind")).unwrap();
        let id = Uuid::from_u128(1);

        layout.create_entry(id, data(1)).unwrap();
        layout.create_path(&path("a/b.txt"), id).unwrap();

        assert_eq!(layout.id_of(&path("a/b.txt")), Some(id));
        assert_eq!(layout.path_of(id), Some(path("a/b.txt")));
        assert_eq!(layout.data_of(&path("a/b.txt")), Some(data(1)));
    }

    #[test]
    fn a_change_survives_being_read_back_by_another_open() {
        let dir = scratch("reopen");
        let id = Uuid::from_u128(2);

        {
            let layout = Layout::open(&dir).unwrap();
            layout.create_entry(id, data(2)).unwrap();
            layout.create_path(&path("a.txt"), id).unwrap();
            layout.move_path(&path("a.txt"), &path("b.txt")).unwrap();
        }

        let layout = Layout::open(&dir).unwrap();
        assert!(layout.id_of(&path("a.txt")).is_none());
        assert_eq!(layout.id_of(&path("b.txt")), Some(id));
        assert_eq!(layout.entry(id), Some(data(2)));
    }

    #[test]
    fn a_create_where_something_already_is_is_refused() {
        let layout = Layout::open(scratch("exists")).unwrap();
        let id = Uuid::from_u128(3);

        layout.create_path(&path("a.txt"), id).unwrap();
        assert!(matches!(
            layout.create_path(&path("a.txt"), Uuid::from_u128(4)),
            Err(LayoutError::AlreadyExists)
        ));

        // A `Uuid` is at one path, so binding it somewhere else moves it rather than duplicating.
        layout.create_path(&path("b.txt"), id).unwrap();
        assert!(layout.id_of(&path("a.txt")).is_none());
        assert_eq!(layout.id_of(&path("b.txt")), Some(id));
    }

    #[test]
    fn a_change_to_something_that_is_not_there_is_refused() {
        let layout = Layout::open(scratch("missing")).unwrap();

        assert!(matches!(
            layout.remove_path(&path("none.txt")),
            Err(LayoutError::NotFound)
        ));
        assert!(matches!(
            layout.move_path(&path("none.txt"), &path("other.txt")),
            Err(LayoutError::NotFound)
        ));
        assert!(matches!(
            layout.update_entry(Uuid::from_u128(9), data(9)),
            Err(LayoutError::NotFound)
        ));
        assert!(matches!(
            layout.remove_entry(Uuid::from_u128(9)),
            Err(LayoutError::NotFound)
        ));
    }

    #[test]
    fn changing_an_entry_changes_what_is_held_for_it() {
        let layout = Layout::open(scratch("update")).unwrap();
        let id = Uuid::from_u128(5);

        layout.create_entry(id, data(5)).unwrap();
        layout.update_entry(id, data(6)).unwrap();

        assert_eq!(layout.entry(id), Some(data(6)));
    }

    #[test]
    fn removing_an_entry_takes_the_path_that_named_it_with_it() {
        let layout = Layout::open(scratch("remove")).unwrap();
        let id = Uuid::from_u128(7);

        layout.create_entry(id, data(7)).unwrap();
        layout.create_path(&path("a.txt"), id).unwrap();
        layout.remove_entry(id).unwrap();

        assert!(layout.entry(id).is_none());
        assert!(layout.id_of(&path("a.txt")).is_none());
    }

    #[test]
    fn a_half_made_create_is_reported_rather_than_refused() {
        let layout = Layout::open(scratch("orphans")).unwrap();
        let kept = Uuid::from_u128(11);
        let alone = Uuid::from_u128(12);

        layout.create_entry(kept, data(11)).unwrap();
        layout.create_path(&path("a.txt"), kept).unwrap();
        // An entry no path names, and a path naming no entry: both are between the two changes.
        layout.create_entry(alone, data(12)).unwrap();
        layout
            .create_path(&path("b.txt"), Uuid::from_u128(13))
            .unwrap();

        let orphans = layout.orphans();
        assert_eq!(orphans.without_path, vec![alone]);
        assert_eq!(orphans.without_entry, vec![path("b.txt")]);
    }

    #[test]
    fn a_compaction_while_writing_keeps_every_change() {
        use std::sync::Arc;
        use std::thread;

        let dir = scratch("concurrent");
        let layout = Arc::new(Layout::open(&dir).unwrap());
        let ids: Vec<Uuid> = (0..128).map(|n| Uuid::from_u128(n + 1000)).collect();

        // One thread keeps writing while another keeps compacting; what the writers say must all be
        // there at the end, whichever shard each change went to.
        let compacting = {
            let layout = Arc::clone(&layout);
            thread::spawn(move || {
                for _ in 0..8 {
                    layout.compact().unwrap();
                }
            })
        };

        let writing = {
            let layout = Arc::clone(&layout);
            let ids = ids.clone();
            thread::spawn(move || {
                for id in ids {
                    layout.create_entry(id, data(seed_of(id))).unwrap();
                    let path = path(&format!("file-{}", id.as_u128()));
                    layout.create_path(&path, id).unwrap();
                }
            })
        };

        writing.join().unwrap();
        compacting.join().unwrap();

        for id in &ids {
            assert_eq!(layout.entry(*id), Some(data(seed_of(*id))), "{id}");
        }

        // And it is all still there after a compaction and a fresh open.
        layout.compact().unwrap();
        let reopened = Layout::open(&dir).unwrap();
        for id in &ids {
            assert_eq!(reopened.entry(*id), Some(data(seed_of(*id))), "{id}");
        }
    }

    #[test]
    fn a_reader_caught_by_another_compaction_starts_again() {
        let dir = scratch("aged");

        // Two handles on one layout, as two processes have: one reads while the other writes and
        // compacts, so the place the reader had read up to is in a log the other one emptied.
        let reader = Layout::open(&dir).unwrap();
        let writer = Layout::open(&dir).unwrap();
        reader.refresh().unwrap();

        let made: Vec<Uuid> = (0..8_u128).map(|n| Uuid::from_u128(500 + n)).collect();
        for id in &made {
            writer.create_entry(*id, data(seed_of(*id))).unwrap();
            writer
                .create_path(&path(&format!("f-{}", id.as_u128())), *id)
                .unwrap();
        }
        reader.refresh().unwrap();
        for id in &made {
            assert_eq!(reader.entry(*id), Some(data(seed_of(*id))), "{id}");
        }

        // The other handle writes it all down and empties the log, so the place the reader had read
        // up to counts bytes of a log that is no longer there.
        writer.compact().unwrap();

        let later = Uuid::from_u128(900);
        writer.create_entry(later, data(seed_of(later))).unwrap();
        writer.create_path(&path("later"), later).unwrap();

        // What was written after the compaction is read, though the reader had read far past it,
        // and what it had read before is still held.
        reader.refresh().unwrap();
        assert_eq!(reader.entry(later), Some(data(seed_of(later))));
        for id in &made {
            assert_eq!(reader.entry(*id), Some(data(seed_of(*id))), "{id}");
        }
    }

    #[test]
    fn a_compacted_layout_reads_back_whole() {
        let dir = scratch("compact");
        let bound = Uuid::from_u128(21);
        let alone = Uuid::from_u128(22);

        {
            let layout = Layout::open(&dir).unwrap();
            layout.create_entry(bound, data(21)).unwrap();
            layout.create_path(&path("a.txt"), bound).unwrap();
            layout.create_entry(alone, data(22)).unwrap();
            layout.compact().unwrap();

            // What is written after the snapshot is still read back with it.
            layout.move_path(&path("a.txt"), &path("b.txt")).unwrap();
        }

        let layout = Layout::open(&dir).unwrap();
        assert!(layout.id_of(&path("a.txt")).is_none());
        assert_eq!(layout.id_of(&path("b.txt")), Some(bound));
        assert_eq!(layout.entry(bound), Some(data(21)));
        assert_eq!(layout.entry(alone), Some(data(22)));
        assert_eq!(layout.orphans().without_path, vec![alone]);
    }
}

//! A shard's log: one file, appended to and read back.
//!
//! A log is written by appending and read by taking bytes off the front from where the reader last
//! stopped. A record is appended as one write to a file opened to append, so what a write puts in
//! the log is whole — a reader is never met part of the way through a record.
//!
//! A write — an append or a compaction — holds a lock on the file exclusively for as long as it
//! runs, and a read holds it shared, so a log is never emptied while a writer is part of the way
//! through appending to it, nor read while it is being rewritten. Appends are one at a time rather
//! than concurrent: a shared lock would keep several going, but a writer may not hold one and still
//! write — Windows denies a write to the range a shared lock covers, its own holder's included. The
//! lock is the operating system's, so it holds across processes as well as threads — which is what
//! makes a compaction safe while other processes are writing.
//!
//! A compaction empties the log, which leaves every reader that had read up to some point of the old
//! log holding an offset into a file that is no longer there — the bytes it counts are gone, and
//! what it would read next is the middle of a record rather than one. So a compaction also **names
//! the age it left behind**: a small file beside the log holds a number, raised by one every time the
//! log is emptied, and a reader that sees the number it holds is not the one it read under starts
//! again from the beginning. A reader's own offset into a log is therefore only ever good for the
//! age it was made in, and no reader is left reading someone else's compaction.
//!
//! Every append is followed by an `fsync`, so a change that has been made is one a crash cannot
//! take back. That is the write cost this layout chooses: a change is durable when the call
//! returns.

use std::fs::{File, OpenOptions};
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use crate::error::LayoutError;

/// One shard's log.
pub struct ShardLog {
    /// The handle writes are appended through, the file a write locks exclusively, and the handle a
    /// compaction shortens: one handle, because Windows denies a second one what the first has locked.
    write: File,
    /// The handle reads are taken through.
    read: File,
    /// Where the age of the log is written down.
    epoch_path: PathBuf,
    /// How many bytes of the age this reader is up to have been read and applied.
    consumed: u64,
    /// The age this reader last saw. An offset means nothing under another one.
    generation: u64,
}

impl ShardLog {
    /// Opens the log at `path`, making it if it is not there.
    ///
    /// # Errors
    ///
    /// Returns why the file could not be opened, or why the age beside it could not be read.
    pub fn open(path: &Path) -> io::Result<Self> {
        // One handle for writing, truncating and locking, opened for reading as well. Windows
        // refuses `LockFileEx` on a handle that has neither `GENERIC_READ` nor `GENERIC_WRITE`, and
        // refuses `SetEndOfFile` on a handle that only has append access — so a log written through
        // an append-only handle could be locked but never emptied, and a compaction is both. Appends
        // therefore seek to the end first, which they do with the lock held, so nothing can be
        // writing there at the same time.
        let write = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        let read = File::open(path)?;

        let epoch_path = path.with_extension(EPOCH_EXTENSION);
        let generation = read_generation(&epoch_path)?;

        Ok(Self {
            write,
            read,
            epoch_path,
            consumed: 0,
            generation,
        })
    }

    /// Opens the log at `path`, which has to be there already.
    ///
    /// A reader of a layout it did not make — a fetched copy, say — must not bring a log into being:
    /// an empty directory would then open as a layout holding nothing, and "nothing was fetched"
    /// would read as "the layout is empty". What is not there is therefore
    /// [`io::ErrorKind::NotFound`] rather than a new file.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::NotFound`] when the log is not there, and whatever else the open
    /// fails with.
    pub fn open_existing(path: &Path) -> io::Result<Self> {
        // Opened as `open` does and without `create`, because a reader still holds the handle a
        // compaction takes its lock through, and the two opens are one shape.
        let write = OpenOptions::new().read(true).write(true).open(path)?;
        let read = File::open(path)?;

        let epoch_path = path.with_extension(EPOCH_EXTENSION);
        let generation = read_generation(&epoch_path)?;

        Ok(Self {
            write,
            read,
            epoch_path,
            consumed: 0,
            generation,
        })
    }

    /// Appends `bytes`, and waits until they are on disk.
    ///
    /// The lock is held exclusively for as long as this runs, as a compaction's is, so the two
    /// cannot overlap — and on Windows an exclusive lock is also what lets the one holding it write,
    /// a shared lock there denying writes even to its own holder.
    ///
    /// The end of the file is asked for under that lock rather than trusted from the handle's mode:
    /// a handle opened to append would be one that cannot be shortened, and a compaction shortens.
    ///
    /// # Errors
    ///
    /// Returns why the write, the flush or the lock failed.
    pub fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.write.lock()?;
        let result = self
            .write
            .seek(SeekFrom::End(0))
            .and_then(|_| self.write.write_all(bytes))
            .and_then(|()| self.write.sync_data());
        self.write.unlock()?;

        result
    }

    /// The bytes appended since the last read, without moving past them.
    ///
    /// A shared lock is held for as long as this runs, so what is read is not written over under it
    /// — a compaction, which holds the lock exclusively, cannot empty the log mid-read — and, under
    /// that same lock, the age of the log is read: a reader that meets an age it has not read under
    /// has been compacted away from its offset, and starts again from the beginning rather than
    /// reading on into the middle of a record.
    ///
    /// A log that has not grown costs one length check and no copy.
    ///
    /// # Errors
    ///
    /// Returns why the age or the log could not be read, or the lock could not be taken.
    pub fn poll(&mut self) -> io::Result<Vec<u8>> {
        self.read.lock_shared()?;
        let result = self.poll_locked();
        self.read.unlock()?;

        result
    }

    /// Counts `count` bytes as read, so the next read starts after them.
    pub const fn advance(&mut self, count: u64) {
        self.consumed += count;
    }

    /// Reads a shard as one whole: the snapshot at `snapshot`, and every byte the log holds after it.
    ///
    /// The two are read under the log's shared lock, which is what makes them one whole rather than
    /// two reads a compaction can fall between: a compaction holds the log exclusively across writing
    /// the snapshot and emptying the log, so what is read here is all of the state before it or all
    /// of the state after it, never a snapshot from one and a log from the other.
    ///
    /// A first value of no bytes is no snapshot at all — a shard nothing has been written down for —
    /// rather than a malformed one; the second is what the log holds past what this handle had read.
    ///
    /// # Errors
    ///
    /// Returns why the snapshot or the log could not be read, or the lock could not be taken.
    pub fn read_whole(&mut self, snapshot: &Path) -> io::Result<(Vec<u8>, Vec<u8>)> {
        self.read.lock_shared()?;
        let result = self.read_whole_locked(snapshot);
        self.read.unlock()?;

        result
    }

    /// The whole of a shard, with the shared lock already held.
    fn read_whole_locked(&mut self, snapshot: &Path) -> io::Result<(Vec<u8>, Vec<u8>)> {
        let snapshot = match std::fs::read(snapshot) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        let appended = self.poll_locked()?;

        Ok((snapshot, appended))
    }

    /// Writes everything the log still holds down whole, and empties it.
    ///
    /// An exclusive lock is held for the whole of this, so no other process is part of the way
    /// through appending while the log is being emptied, and none reads it while it is being
    /// rewritten. `replay` is handed the bytes appended since this log was last read — which the
    /// caller is expected to fold into the whole it is about to write down — and the log is only
    /// emptied after the age has been raised, so a compaction a crash cut short leaves a log that
    /// is replayed rather than an offset that points past what is left.
    ///
    /// # Errors
    ///
    /// Returns why the log could not be read, emptied or aged, or what `replay` failed with.
    pub fn compact(
        &mut self,
        mut replay: impl FnMut(&[u8]) -> Result<(), LayoutError>,
    ) -> Result<(), LayoutError> {
        self.write.lock()?;
        let result = self.compact_locked(&mut replay);
        self.write.unlock()?;

        result
    }

    /// The whole of a compaction, with the exclusive lock already held.
    fn compact_locked(
        &mut self,
        replay: &mut impl FnMut(&[u8]) -> Result<(), LayoutError>,
    ) -> Result<(), LayoutError> {
        // A log with no bytes in it has nothing to fold into the snapshot and nothing to empty, so a
        // shard nothing has been appended to is left exactly as it is rather than spending three
        // flushes to say so. The bytes this handle has already read count as content: they are in
        // the caller's memory and only the flush of a snapshot takes them from the log.
        if self.write.metadata()?.len() == 0 {
            return Ok(());
        }

        let remaining = self.pending_locked()?;
        replay(&remaining)?;

        self.raise_generation()?;
        self.empty_locked()?;

        Ok(())
    }

    /// The bytes appended since the last read, with the caller already holding the lock.
    ///
    /// Read through the handle that holds the lock and never through the other one: Windows denies a
    /// read of a range another handle holds exclusively — the same process's other handle included —
    /// so a compaction that read through the reader's handle would be a compaction that could not
    /// read the log it was compacting. The writer's handle is opened for reading, so it can.
    fn pending_locked(&mut self) -> io::Result<Vec<u8>> {
        let length = self.write.metadata()?.len();
        if length <= self.consumed {
            return Ok(Vec::new());
        }

        self.write.seek(SeekFrom::Start(self.consumed))?;
        let mut bytes = vec![0_u8; usize::try_from(length - self.consumed).unwrap_or(0)];
        self.write.read_exact(&mut bytes)?;

        Ok(bytes)
    }

    /// The bytes appended since the last read, with the shared lock already held.
    fn poll_locked(&mut self) -> io::Result<Vec<u8>> {
        let generation = read_generation(&self.epoch_path)?;
        if generation != self.generation {
            self.generation = generation;
            self.consumed = 0;
        }

        self.pending_locked()
    }

    /// Raises the age, so a reader whose offset is in the old log knows to start again.
    ///
    /// It is written under a name of its own and moved into place, so a reader in another process
    /// never sees it half-written.
    fn raise_generation(&mut self) -> io::Result<()> {
        let raised = self.generation + 1;

        let temporary = self
            .epoch_path
            .with_extension(format!("{EPOCH_EXTENSION}-{}", std::process::id()));
        let mut file = File::create(&temporary)?;
        file.write_all(&raised.to_be_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, &self.epoch_path)?;

        self.generation = raised;

        Ok(())
    }

    /// Empties the log, so what comes next starts at the beginning.
    ///
    /// Through the handle that holds the lock, and no other. Windows refuses to shorten a file that
    /// another handle has a byte of locked — even when the lock is the caller's own, held through a
    /// different handle — and a compaction runs with the log locked, so a handle of its own to
    /// truncate through is a handle that cannot. Unix ignores the lock here and would let the third
    /// handle work, which is why this only ever failed on one of the two.
    fn empty_locked(&mut self) -> io::Result<()> {
        self.write.set_len(0)?;
        self.write.sync_all()?;

        self.consumed = 0;
        self.write.seek(SeekFrom::Start(0))?;

        Ok(())
    }
}

/// What a log's age is written down beside it as — `log-0.wal` being aged by `log-0.epoch`.
const EPOCH_EXTENSION: &str = "epoch";

/// The age written down at `path`, or none when nothing has aged it yet.
fn read_generation(path: &Path) -> io::Result<u64> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };

    let head = bytes
        .get(..8)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "an age too short to be one"))?;

    Ok(u64::from_be_bytes(head.try_into().unwrap_or_default()))
}

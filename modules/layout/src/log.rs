//! A shard's log: one file, appended to and read back.
//!
//! A log is written by appending and read by taking bytes off the front from where the reader last
//! stopped. Appending is a single write of a whole record to a file opened to append, which is what
//! lets several processes write the same log at once: the kernel puts each write at the end as one,
//! so two writers cannot interleave inside a record.
//!
//! An append and a compact are told apart by a lock on the file, taken for as long as each runs:
//! appending and reading hold it shared, so they are one another's company, and compacting holds it
//! exclusive, so a log is never emptied while a writer is part of the way through appending to it.
//! The lock is the operating system's, so it holds across processes as well as threads — which is
//! what makes a compaction safe while other processes are writing.
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
    /// The handle writes are appended through, and the file a compaction locks exclusively.
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
        let write = OpenOptions::new().create(true).append(true).open(path)?;
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
    /// A shared lock is held for as long as this runs, so a compaction — which holds the lock
    /// exclusively — cannot empty the log part of the way through the append.
    ///
    /// # Errors
    ///
    /// Returns why the write, the flush or the lock failed.
    pub fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.write.lock_shared()?;
        let result = self
            .write
            .write_all(bytes)
            .and_then(|()| self.write.sync_data());
        self.write.unlock()?;

        result
    }

    /// The bytes appended since the last read, without moving past them.
    ///
    /// A shared lock is held for as long as this runs, so what is read is not emptied under it by a
    /// compaction — and, under that same lock, the age of the log is read: a reader that meets an
    /// age it has not read under has been compacted away from its offset, and starts again from the
    /// beginning rather than reading on into the middle of a record.
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
        let remaining = self.pending_locked()?;
        replay(&remaining)?;

        self.raise_generation()?;
        self.empty_locked()?;

        Ok(())
    }

    /// The bytes appended since the last read, with the caller already holding the lock.
    fn pending_locked(&mut self) -> io::Result<Vec<u8>> {
        let length = self.read.metadata()?.len();
        if length <= self.consumed {
            return Ok(Vec::new());
        }

        self.read.seek(SeekFrom::Start(self.consumed))?;
        let mut bytes = vec![0_u8; usize::try_from(length - self.consumed).unwrap_or(0)];
        self.read.read_exact(&mut bytes)?;

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
    fn empty_locked(&mut self) -> io::Result<()> {
        self.write.set_len(0)?;
        self.write.sync_all()?;

        self.consumed = 0;
        self.read.seek(SeekFrom::Start(0))?;

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

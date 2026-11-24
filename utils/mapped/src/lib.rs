#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::fs::File;
use std::io::{self, Read as _};
use std::ops::{Deref, Range};
use std::path::Path;
use std::sync::Arc;

/// The size at or above which a file is mapped rather than read into a buffer.
///
/// A mapping costs a call to make and a call to undo, and it faults a page at a time: below a page
/// the read is the cheaper of the two, and a few pages of a file are not worth the bookkeeping. The
/// number is a page count rather than a guess at what a caller might want: sixteen pages is past
/// the point where the copies a read makes start to be the larger cost.
pub const MIN_MAPPED: u64 = 64 * 1024;

/// The size at or above which a slice is hashed on more than one thread.
///
/// `blake3` states that its multithreaded call is a loss on inputs under 128 KiB, so a smaller slice
/// is handed to the single-threaded one.
const RAYON_MIN: usize = 128 * 1024;

/// A file's bytes, mapped into the process where that is cheaper than reading them.
///
/// Which of the two it is, is decided when it is opened, see [`MIN_MAPPED`], and nothing a caller
/// asks of it depends on the answer: it reads as a slice either way.
pub struct Mapped(Held);

/// What a [`Mapped`] holds.
enum Held {
    /// The file's pages, mapped and faulted in as they are read.
    Mapped(memmap2::Mmap),
    /// The file's bytes, read into a buffer because mapping would have cost more.
    Heap(Vec<u8>),
}

impl Mapped {
    /// The bytes of the file at `path`.
    ///
    /// A file under [`MIN_MAPPED`] is read rather than mapped, and a bigger one is mapped. Neither
    /// answer is visible to a caller beyond how much memory the reading holds.
    ///
    /// # Errors
    ///
    /// Returns what opening, reading or mapping the file fails with.
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();

        if size < MIN_MAPPED {
            let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
            file.read_to_end(&mut bytes)?;

            return Ok(Self(Held::Heap(bytes)));
        }

        // SAFETY: the mapping is read for as long as this value lives, and what it reads is a file
        // this process does not truncate: a store writes an object beside its place and renames it
        // into it (`modules/storage/src/protocols/rorolala/entry.rs`), so an object's bytes are
        // never shortened in place. A file the user holds is another matter, and a working-tree file
        // cut short while it is read faults rather than reading something wrong, which is the price
        // of mapping a file this process does not own. On Windows a mapped file cannot be removed,
        // so a removal of one lands as a failure rather than as a dangling mapping, and no mapping
        // outlives the one read it was made for.
        let map = unsafe { memmap2::Mmap::map(&file)? };

        Ok(Self(Held::Mapped(map)))
    }

    /// The bytes, whichever of the two is holding them.
    fn bytes(&self) -> &[u8] {
        match &self.0 {
            Held::Mapped(map) => &map[..],
            Held::Heap(bytes) => bytes.as_slice(),
        }
    }

    /// How many bytes the file holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    /// Whether the file held nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes().is_empty()
    }

    /// The BLAKE3 hash of these bytes.
    ///
    /// A slice at or above `RAYON_MIN` is hashed on more than one thread, which is what makes a
    /// large material two or three times cheaper to name than a sequential pass is.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let bytes = self.bytes();
        let mut hasher = blake3::Hasher::new();

        if bytes.len() >= RAYON_MIN {
            hasher.update_rayon(bytes);
        } else {
            hasher.update(bytes);
        }

        *hasher.finalize().as_bytes()
    }
}

impl Deref for Mapped {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.bytes()
    }
}

impl From<Mapped> for Bytes {
    fn from(mapped: Mapped) -> Self {
        let range = 0..mapped.len();

        Self {
            parts: vec![Part::Mapped(Arc::new(mapped), range)],
        }
    }
}

/// A run of bytes, held as ranges of mapped files and as buffers, in the order it reads.
///
/// A read that finds its bytes in a file that is already mapped hands out a range of that mapping,
/// so what a caller gets is the page cache rather than a copy of it. A piece that has to be decoded,
/// a compressed chunk say, is a buffer of its own, and the two kinds sit in one run so that a caller
/// walking it does not have to know which is which.
#[derive(Clone, Default)]
pub struct Bytes {
    /// The pieces, in the order the run reads them.
    parts: Vec<Part>,
}

/// One piece of a [`Bytes`].
#[derive(Clone)]
enum Part {
    /// A range of a file mapped into memory.
    Mapped(Arc<Mapped>, Range<usize>),
    /// Bytes the program holds itself.
    Owned(Vec<u8>),
}

impl Bytes {
    /// A run of the bytes `bytes` holds.
    #[must_use]
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self {
            parts: vec![Part::Owned(bytes)],
        }
    }

    /// A run of the `range` of `map`, or nothing when the range is not inside it.
    #[must_use]
    pub fn mapped(map: Arc<Mapped>, range: Range<usize>) -> Option<Self> {
        // The range is refused rather than shortened: it comes from a frame read out of the same
        // mapping, and one that reaches past the end says the store does not hold together.
        if range.start > range.end || range.end > map.len() {
            return None;
        }

        Some(Self {
            parts: vec![Part::Mapped(map, range)],
        })
    }

    /// How many bytes the run holds, which is the sum of its pieces.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.parts
            .iter()
            .map(|part| match part {
                Part::Mapped(_, range) => range.end.saturating_sub(range.start),
                Part::Owned(bytes) => bytes.len(),
            })
            .fold(0_u64, |sum, length| {
                sum.saturating_add(u64::try_from(length).unwrap_or(u64::MAX))
            })
    }

    /// Whether the run held nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Adds `other` to the end of this run.
    pub fn push(&mut self, other: Self) {
        self.parts.extend(other.parts);
    }

    /// The pieces of the run, in order.
    pub fn parts(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.parts.iter().map(|part| match part {
            // The range was refused at construction when it was not inside the mapping, so a piece
            // already in the run always reads. A range that somehow did not would read as no bytes
            // rather than ending the process, and the content it makes would fail its own hash.
            Part::Mapped(map, range) => map.get(range.start..range.end).unwrap_or_default(),
            Part::Owned(bytes) => bytes.as_slice(),
        })
    }

    /// The whole run as one slice, when it is one piece, and nothing when it is more than one.
    #[must_use]
    pub fn as_slice(&self) -> Option<&[u8]> {
        match self.parts.as_slice() {
            [Part::Mapped(map, range)] => map.get(range.start..range.end),
            [Part::Owned(bytes)] => Some(bytes.as_slice()),
            _ => None,
        }
    }

    /// The run copied into one buffer.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(usize::try_from(self.len()).unwrap_or(0));

        for part in self.parts() {
            bytes.extend_from_slice(part);
        }

        bytes
    }

    /// The BLAKE3 hash of the run, taken over its pieces in order.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();

        for part in self.parts() {
            if part.len() >= RAYON_MIN {
                hasher.update_rayon(part);
            } else {
                hasher.update(part);
            }
        }

        *hasher.finalize().as_bytes()
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_vec(bytes)
    }
}

impl PartialEq for Bytes {
    /// Two runs are equal when they read as the same bytes, whichever of the two kinds their pieces
    /// are: what a caller compares is what the run says, not where it lies.
    fn eq(&self, other: &Self) -> bool {
        let mut mine = self.parts();
        let mut theirs = other.parts();

        loop {
            match (mine.next(), theirs.next()) {
                (None, None) => return true,
                (Some(left), Some(right)) if left == right => {}
                _ => return false,
            }
        }
    }
}

impl std::fmt::Debug for Bytes {
    /// Says how long the run is and how many pieces hold it, never the bytes: a content read here
    /// can be a gigabyte, and what a report of one needs is its shape.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Bytes")
            .field("len", &self.len())
            .field("pieces", &self.parts.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{Bytes, MIN_MAPPED, Mapped};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-utils-mapped-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    #[test]
    fn a_small_file_and_a_large_one_read_as_their_bytes() {
        let dir = scratch("read");
        let small = dir.join("small");
        let large = dir.join("large");

        let few = b"the content a small file holds".to_vec();
        let many: Vec<u8> = (0..MIN_MAPPED + 1024)
            .map(|value| u8::try_from(value % 251).unwrap())
            .collect();

        fs::write(&small, &few).unwrap();
        fs::write(&large, &many).unwrap();

        let small = Mapped::open(&small).unwrap();
        let large = Mapped::open(&large).unwrap();

        assert_eq!(&*small, few.as_slice());
        assert_eq!(&*large, many.as_slice());
        assert_eq!(small.len(), few.len());
        assert_eq!(large.len(), many.len());
        assert!(!small.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_digest_is_the_hash_of_the_bytes_either_way() {
        let dir = scratch("digest");

        for (label, size) in [
            ("heap", 32_usize),
            ("mapped", usize::try_from(MIN_MAPPED).unwrap()),
        ] {
            let path = dir.join(label);
            let bytes: Vec<u8> = (0..size)
                .map(|value| u8::try_from(value % 251).unwrap())
                .collect();
            fs::write(&path, &bytes).unwrap();

            let mapped = Mapped::open(&path).unwrap();

            assert_eq!(mapped.digest(), *blake3::hash(&bytes).as_bytes(), "{label}");
            assert_eq!(
                Bytes::from(mapped).digest(),
                *blake3::hash(&bytes).as_bytes(),
                "{label}"
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pieces_are_walked_in_order_and_come_back_as_one_run() {
        let dir = scratch("pieces");
        let path = dir.join("file");
        fs::write(&path, b"the mapped half").unwrap();

        let mapped = Arc::new(Mapped::open(&path).unwrap());
        let mut run = Bytes::mapped(Arc::clone(&mapped), 4..10).unwrap();
        run.push(Bytes::from(b", and the half held".to_vec()));

        assert_eq!(run.len(), 6 + 19);
        assert_eq!(run.to_vec(), b"mapped, and the half held".to_vec());
        assert_eq!(run.as_slice(), None);

        let pieces: Vec<&[u8]> = run.parts().collect();
        assert_eq!(pieces, vec![&b"mapped"[..], &b", and the half held"[..]]);

        let whole = Bytes::from(fs::read(&path).unwrap());
        assert_eq!(whole.as_slice(), Some(&b"the mapped half"[..]));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_range_outside_the_mapping_is_refused() {
        let dir = scratch("range");
        let path = dir.join("file");
        fs::write(&path, b"short").unwrap();

        let mapped = Arc::new(Mapped::open(&path).unwrap());

        // The two ends are variables rather than a literal, so the reversed range is a value this
        // test hands over rather than something the compiler reads as a loop that cannot run.
        let (start, end) = (4, 2);

        assert!(Bytes::mapped(Arc::clone(&mapped), 0..5).is_some());
        assert!(Bytes::mapped(Arc::clone(&mapped), 0..6).is_none());
        assert!(Bytes::mapped(Arc::clone(&mapped), start..end).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_is_read_from_another_thread() {
        let dir = scratch("thread");
        let path = dir.join("file");
        fs::write(&path, b"across a thread").unwrap();

        let run = Bytes::from(Mapped::open(&path).unwrap());
        let pieces = std::thread::scope(|scope| {
            scope
                .spawn(|| run.parts().map(<[u8]>::to_vec).collect::<Vec<_>>())
                .join()
                .unwrap()
        });

        assert_eq!(pieces.concat(), b"across a thread");

        let _ = fs::remove_dir_all(&dir);
    }
}

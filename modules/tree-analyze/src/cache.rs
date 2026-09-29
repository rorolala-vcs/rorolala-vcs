//! What the last analysis found, kept so the next one can skip what has not changed.
//!
//! A reading of a tree costs what reading its files costs, so what was read is written down: each
//! file's time, its length, the digest of its content, and — for a text file the Layout names —
//! the spans its content was cut into, which is what a move is recognised by when the file that
//! moved is no longer there to be read. A file whose time and length are what they were is not
//! read again; its digest is the one already written down.
//!
//! Two digests are kept, not one. The first is what the file holds now, which is what tells a move
//! from a loss and a gain; the second is what it held when the Layout was last known to agree with
//! it, which is what tells a change from a file that is simply as it was. They are the same until
//! the file changes, and the pair is what lets a reading be told what changed without forgetting it
//! — a file that changed and was never written down again would be a change reported once.
//!
//! What is written down is a cache: a file that does not read, or that was written by another
//! build, is nothing rather than a failure, since the next reading makes it again.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rorolala_layout::LayoutPath;

use crate::error::TreeDiffError;

/// The magic every cache starts with.
const MAGIC: &[u8; 4] = b"RLTA";

/// The version of the cache format this build writes.
const VERSION: u8 = 2;

/// What one file looked like when it was last read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// When it was last changed, in whole seconds since the epoch.
    seconds: u64,
    /// When it was last changed, the part of a second after them.
    nanos: u32,
    /// How long it was.
    size: u64,
    /// The digest of its content.
    digest: [u8; 32],
    /// The digest it held when the Layout was last known to agree with it, or nothing when the two
    /// are known to disagree — a move whose file was edited as well, which the reading found and
    /// nothing has recorded yet.
    agree: Option<[u8; 32]>,
    /// The spans its content was cut into, when it read as text.
    spans: Option<Vec<(u32, u32)>>,
}

impl Entry {
    /// What a file looked like: when it changed, how long it is, its digest, and its spans.
    ///
    /// A file just read is one the Layout agrees with until something says otherwise: what the
    /// reading found is what is there, and nothing has yet been recorded that says the two differ.
    #[must_use]
    pub const fn new(
        seconds: u64,
        nanos: u32,
        size: u64,
        digest: [u8; 32],
        spans: Option<Vec<(u32, u32)>>,
    ) -> Self {
        Self {
            seconds,
            nanos,
            size,
            digest,
            agree: Some(digest),
            spans,
        }
    }

    /// The digest of the file's content.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// The digest the Layout was last known to agree with, or nothing when it is known to differ.
    #[must_use]
    pub const fn agreed(&self) -> Option<[u8; 32]> {
        self.agree
    }

    /// How long the file is.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// The spans the file's content was cut into, when it read as text.
    #[must_use]
    pub fn spans(&self) -> Option<&[(u32, u32)]> {
        self.spans.as_deref()
    }

    /// Whether a file changed at `seconds`/`nanos` and this long is the one this describes.
    #[must_use]
    pub const fn same_stamp(&self, seconds: u64, nanos: u32, size: u64) -> bool {
        self.seconds == seconds && self.nanos == nanos && self.size == size
    }

    /// The same file, with the spans its content was cut into forgotten.
    ///
    /// The digest is what a file is, and the spans are only how a move of it may be guessed when
    /// two files are not the same bytes. Forgetting them leaves a record that still says what the
    /// file was, and no longer says what it resembled — which is how a pairing the reader has
    /// decided against is taken back without losing what the path is remembered by.
    #[must_use]
    pub const fn without_spans(&self) -> Self {
        Self {
            seconds: self.seconds,
            nanos: self.nanos,
            size: self.size,
            digest: self.digest,
            agree: self.agree,
            spans: None,
        }
    }

    /// The same file, with the Layout known not to agree with what it holds.
    ///
    /// It is what a move that was edited as well leaves behind: the Layout still names the version
    /// the file held before it moved, so the two disagree until the change is recorded.
    #[must_use]
    pub fn disagreed(&self) -> Self {
        Self {
            seconds: self.seconds,
            nanos: self.nanos,
            size: self.size,
            digest: self.digest,
            agree: None,
            spans: self.spans.clone(),
        }
    }

    /// The same file, with the digest the Layout is to be held to agree with.
    #[must_use]
    pub fn with_agreed(self, agree: Option<[u8; 32]>) -> Self {
        Self { agree, ..self }
    }
}

/// What the last analysis of one Layout found, by the path each file was at.
#[derive(Debug, Default, Clone)]
pub struct Cache {
    /// What each file looked like, in path order.
    entries: BTreeMap<LayoutPath, Entry>,
}

impl Cache {
    /// An empty cache.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// What the file at `path` looked like, if it was read last time.
    #[must_use]
    pub fn get(&self, path: &LayoutPath) -> Option<&Entry> {
        self.entries.get(path)
    }

    /// Says what the file at `path` looks like now.
    pub fn insert(&mut self, path: LayoutPath, entry: Entry) {
        self.entries.insert(path, entry);
    }

    /// The cache kept at `path`, or an empty one when there is none that reads.
    #[must_use]
    pub fn read(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| Self::decode(&bytes))
            .unwrap_or_default()
    }

    /// Writes the cache to `path`, making the directory it sits in.
    ///
    /// # Errors
    ///
    /// Returns [`TreeDiffError::Io`] if the directory cannot be made or the file cannot be written.
    pub fn write(&self, path: &Path) -> Result<(), TreeDiffError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.encode())?;

        Ok(())
    }

    /// The cache as the bytes it is written down as.
    fn encode(&self) -> Vec<u8> {
        let mut body = Vec::new();
        put_u64(
            &mut body,
            u64::try_from(self.entries.len()).unwrap_or(u64::MAX),
        );

        for (path, entry) in &self.entries {
            let mut flags = u8::from(entry.spans.is_some());
            if entry.agree.is_some() {
                flags |= 2;
            }

            body.push(flags);
            put_str(&mut body, path.as_str());
            put_u64(&mut body, entry.seconds);
            put_u32(&mut body, entry.nanos);
            put_u64(&mut body, entry.size);
            body.extend_from_slice(&entry.digest);

            if let Some(agree) = &entry.agree {
                body.extend_from_slice(agree);
            }

            if let Some(spans) = &entry.spans {
                put_u64(&mut body, u64::try_from(spans.len()).unwrap_or(u64::MAX));
                for (hash, count) in spans {
                    put_u32(&mut body, *hash);
                    put_u32(&mut body, *count);
                }
            }
        }

        let mut bytes = Vec::with_capacity(MAGIC.len() + 1 + 4 + body.len());
        bytes.extend_from_slice(MAGIC);
        bytes.push(VERSION);
        bytes.extend_from_slice(&crc32(&body).to_be_bytes());
        bytes.extend_from_slice(&body);

        bytes
    }

    /// The cache `bytes` holds, or nothing when they are not one this build wrote.
    fn decode(bytes: &[u8]) -> Option<Self> {
        let body = bytes.strip_prefix(MAGIC.as_slice())?;
        let (&version, body) = body.split_first()?;
        if version != VERSION {
            return None;
        }

        let (checksum, body) = take_u32(body)?;
        if crc32(body) != checksum {
            return None;
        }

        let (count, mut rest) = take_u64(body)?;
        let count = usize::try_from(count).ok()?;

        let mut entries = BTreeMap::new();
        for _ in 0..count {
            let (&flags, after) = rest.split_first()?;
            rest = after;

            let (text, after) = take_str(rest)?;
            rest = after;
            let path = LayoutPath::new(text).ok()?;

            let (seconds, after) = take_u64(rest)?;
            let (nanos, after) = take_u32(after)?;
            let (size, after) = take_u64(after)?;
            let (digest, after) = after.split_at_checked(32)?;
            let digest: [u8; 32] = digest.try_into().ok()?;
            rest = after;

            let agree = if flags & 2 != 0 {
                let (agree, after) = rest.split_at_checked(32)?;
                rest = after;
                Some(<[u8; 32]>::try_from(agree).ok()?)
            } else {
                None
            };

            let spans = if flags & 1 != 0 {
                let (held, after) = take_u64(rest)?;
                let held = usize::try_from(held).ok()?;
                rest = after;

                let mut spans = Vec::with_capacity(held.min(rest.len() / 8));
                for _ in 0..held {
                    let (hash, after) = take_u32(rest)?;
                    let (count, after) = take_u32(after)?;
                    spans.push((hash, count));
                    rest = after;
                }
                Some(spans)
            } else {
                None
            };

            entries.insert(
                path,
                Entry {
                    seconds,
                    nanos,
                    size,
                    digest,
                    agree,
                    spans,
                },
            );
        }

        rest.is_empty().then_some(Self { entries })
    }
}

/// Writes a number, big-endian.
fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

/// Writes a number, big-endian.
fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

/// Writes a string: how long it is, then its UTF-8 bytes.
fn put_str(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    put_u32(out, u32::try_from(bytes.len()).unwrap_or(u32::MAX));
    out.extend_from_slice(bytes);
}

/// Reads a number, big-endian.
fn take_u32(bytes: &[u8]) -> Option<(u32, &[u8])> {
    let (head, rest) = bytes.split_at_checked(4)?;

    Some((u32::from_be_bytes(head.try_into().ok()?), rest))
}

/// Reads a number, big-endian.
fn take_u64(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let (head, rest) = bytes.split_at_checked(8)?;

    Some((u64::from_be_bytes(head.try_into().ok()?), rest))
}

/// Reads what [`put_str`] wrote.
fn take_str(bytes: &[u8]) -> Option<(&str, &[u8])> {
    let (length, rest) = take_u32(bytes)?;
    let (text, rest) = rest.split_at_checked(usize::try_from(length).ok()?)?;

    Some((std::str::from_utf8(text).ok()?, rest))
}

/// The CRC-32 (IEEE) of `bytes`, which tells a cache written whole from one cut short.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;

    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let carry = crc & 1;
            crc >>= 1;
            if carry != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }

    !crc
}

#[cfg(test)]
mod tests {
    use rorolala_layout::LayoutPath;

    use super::{Cache, Entry};

    /// A path, from text a test can read.
    fn path(text: &str) -> LayoutPath {
        LayoutPath::new(text).unwrap()
    }

    #[test]
    fn a_cache_comes_back_as_it_went_in() {
        let mut cache = Cache::empty();
        cache.insert(
            path("a/b.txt"),
            Entry::new(1, 2, 3, [7; 32], Some(vec![(9, 4), (11, 5)])),
        );
        cache.insert(path("c.bin"), Entry::new(4, 5, 6, [8; 32], None));
        // A file the Layout is known to disagree with keeps that through a round trip too.
        cache.insert(
            path("d.txt"),
            Entry::new(7, 8, 9, [9; 32], None).disagreed(),
        );

        let read = Cache::decode(&cache.encode()).unwrap();

        assert_eq!(read.get(&path("a/b.txt")), cache.get(&path("a/b.txt")));
        assert_eq!(read.get(&path("c.bin")), cache.get(&path("c.bin")));
        assert_eq!(read.get(&path("d.txt")), cache.get(&path("d.txt")));
    }

    #[test]
    fn what_does_not_read_as_a_cache_is_nothing_rather_than_a_failure() {
        let mut cache = Cache::empty();
        cache.insert(path("a.txt"), Entry::new(1, 2, 3, [7; 32], None));
        let bytes = cache.encode();

        assert!(Cache::decode(b"not a cache").is_none());

        let mut broken = bytes.clone();
        let last = broken.len() - 1;
        broken[last] ^= 0xff;
        assert!(Cache::decode(&broken).is_none());

        let mut extra = bytes;
        extra.push(0);
        assert!(Cache::decode(&extra).is_none());
    }
}

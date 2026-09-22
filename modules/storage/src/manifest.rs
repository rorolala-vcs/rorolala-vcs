use crate::{BLAKE3_HASH_LEN, Blake3Hash, Error, Key, split_u64};

/// One piece of a stored file: where its content sits, and how long it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    /// The key the chunk's content is stored under.
    key: Key,
    /// The length, in bytes, of the chunk's content.
    len: u64,
}

impl Chunk {
    /// A chunk made of `len` bytes kept under `key`.
    #[must_use]
    pub const fn new(key: Key, len: u64) -> Self {
        Self { key, len }
    }

    /// The key the chunk's content is stored under.
    #[must_use]
    pub const fn key(&self) -> Key {
        self.key
    }

    /// The length, in bytes, of the chunk's content.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Whether the chunk holds nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// How a file was stored: the ordered chunks it is made of.
///
/// A manifest is the whole of what a reader needs to put content back together, and it is
/// deliberately free of any hint about how the chunks were found — see
/// [`Chunker`](crate::Chunker). Two stores that cut the same content differently still agree on
/// what the content is, because the key is the hash of the content and not of the cutting.
///
/// It is stored apart from [`obj`](crate::RorolalaStorage) — under a directory of its own — so
/// that the chunks a manifest refers to can be told from the objects nothing refers to. That is
/// what a cleanup reads to find what is an orphan and what is not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The chunks, in the order their content appears.
    chunks: Vec<Chunk>,
}

impl Manifest {
    /// A manifest of `chunks`, in the order their content appears.
    #[must_use]
    pub const fn new(chunks: Vec<Chunk>) -> Self {
        Self { chunks }
    }

    /// The chunks, in the order their content appears.
    #[must_use]
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// How many chunks the file is made of.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Whether the manifest names no chunk at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Adds a chunk after the ones already here.
    pub fn push(&mut self, chunk: Chunk) {
        self.chunks.push(chunk);
    }

    /// The length, in bytes, of the whole content the chunks add up to.
    #[must_use]
    pub fn content_len(&self) -> u64 {
        self.chunks.iter().map(|chunk| chunk.len).sum()
    }

    /// Writes the manifest down, so it can be read back by [`decode`](Self::decode).
    ///
    /// A manifest is a count and then one record per chunk — the digest and the length — which is
    /// all a reader needs and nothing about how the chunks were found.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(size_of::<u64>() + self.chunks.len() * RECORD_LEN);

        bytes.extend_from_slice(&(self.chunks.len() as u64).to_be_bytes());
        for chunk in &self.chunks {
            bytes.extend_from_slice(chunk.key.digest());
            bytes.extend_from_slice(&chunk.len.to_be_bytes());
        }

        bytes
    }

    /// Reads a manifest written by [`encode`](Self::encode).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if `bytes` does not read as a manifest — a count that will
    /// not fit, a record that is cut short, or bytes left over at the end.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let (count, mut rest) = split_u64(bytes).ok_or(Error::Malformed)?;
        let count = usize::try_from(count).map_err(|_| Error::Malformed)?;

        // The count is what the manifest *says*, not what it holds: a count larger than the bytes
        // left could hold is one no manifest of this size can mean, and reserving room for it would
        // be reserving room a corrupt count asked for. The loop below refuses a count that does not
        // hold up.
        let mut chunks = Vec::with_capacity(count.min(rest.len() / RECORD_LEN));

        for _ in 0..count {
            let (digest, after) = rest
                .split_at_checked(BLAKE3_HASH_LEN)
                .ok_or(Error::Malformed)?;
            let digest: Blake3Hash = digest.try_into().map_err(|_| Error::Malformed)?;
            let (len, after) = split_u64(after).ok_or(Error::Malformed)?;

            chunks.push(Chunk::new(Key::new(digest), len));
            rest = after;
        }

        // A record that was not asked about is not one to read past: a manifest says exactly how
        // long it is, and anything after it is not part of it.
        if !rest.is_empty() {
            return Err(Error::Malformed);
        }

        Ok(Self::new(chunks))
    }
}

/// How long one chunk record is: the digest and the length.
const RECORD_LEN: usize = BLAKE3_HASH_LEN + size_of::<u64>();

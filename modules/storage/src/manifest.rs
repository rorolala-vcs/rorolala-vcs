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
/// It is kept apart from [`obj`](crate::RorolalaStorage) — under a directory of its own, and in packs
/// of its own whose indexes sit among the manifests — so that the chunks a manifest refers to can be
/// told from the objects nothing refers to. That is what a cleanup reads to find what is an orphan and
/// what is not, without reading a single object.
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
    ///
    /// A manifest that came from bytes never adds up past the limit [`decode`](Self::decode)
    /// enforces, since it refuses one that does; neither does one built here a chunk at a time,
    /// because each length is a length of bytes that were really there. The sum saturates rather than
    /// wrapping so that a manifest that is neither — one built by a caller with lengths nothing backs
    /// — cannot panic a debug build and cannot wrap into a small lie in a release one. A caller that
    /// has to tell a saturating sum from a real one wants [`checked_content_len`].
    ///
    /// [`checked_content_len`]: Self::checked_content_len
    #[must_use]
    pub fn content_len(&self) -> u64 {
        self.checked_content_len().unwrap_or(u64::MAX)
    }

    /// The length the chunks add up to, or `None` when the sum does not fit in a `u64`.
    #[must_use]
    pub fn checked_content_len(&self) -> Option<u64> {
        self.chunks
            .iter()
            .try_fold(0_u64, |total, chunk| total.checked_add(chunk.len))
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
    /// not fit, a record that is cut short, bytes left over at the end, or lengths that add up to
    /// more than this build can put back together.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let (count, mut rest) = split_u64(bytes).ok_or(Error::Malformed)?;
        let count = usize::try_from(count).map_err(|_| Error::Malformed)?;

        // The count is what the manifest *says*, not what it holds: a count larger than the bytes
        // left could hold is one no manifest of this size can mean, and reserving room for it would
        // be reserving room a corrupt count asked for. The loop below refuses a count that does not
        // hold up.
        let mut chunks = Vec::with_capacity(count.min(rest.len() / RECORD_LEN));

        // A length is the manifest's to say and nothing here has read the bytes behind it, so the
        // total is where a corrupt or hostile record would aim: one length of `u64::MAX` is a
        // content no `Vec` can hold, and a run of them would overflow the sum. Both are refused
        // here rather than met later by an allocation that aborts the process.
        let mut total = 0_u64;

        for _ in 0..count {
            let (digest, after) = rest
                .split_at_checked(BLAKE3_HASH_LEN)
                .ok_or(Error::Malformed)?;
            let digest: Blake3Hash = digest.try_into().map_err(|_| Error::Malformed)?;
            let (len, after) = split_u64(after).ok_or(Error::Malformed)?;

            total = total.checked_add(len).ok_or(Error::Malformed)?;
            if total > MAX_CONTENT_LEN {
                return Err(Error::Malformed);
            }

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

/// The longest content a manifest may describe.
///
/// Content is put back together as one `Vec<u8>`, and no `Vec` holds more than `isize::MAX` bytes —
/// so a manifest whose chunks add up to more describes content this build cannot carry, whatever
/// the disks it was read from are big enough for. It is a bound on what can be put back together
/// rather than a guess at what content is worth keeping: a genuinely large file stays legal for as
/// long as this machine could hold it.
const MAX_CONTENT_LEN: u64 = isize::MAX as u64;

#[cfg(test)]
mod tests {
    use super::{Chunk, Manifest};
    use crate::{Error, Key};

    /// A manifest of chunks of `lens`, each under a key of its own.
    fn of(lens: &[u64]) -> Manifest {
        let mut manifest = Manifest::default();
        for (at, len) in lens.iter().enumerate() {
            manifest.push(Chunk::new(
                Key::new([u8::try_from(at).unwrap_or(u8::MAX); 32]),
                *len,
            ));
        }

        manifest
    }

    #[test]
    fn a_length_no_content_could_have_is_not_a_manifest() {
        // One record claiming more than a `Vec` can hold is one no read could ever satisfy, and the
        // total is what a corrupt record would aim at.
        let bytes = of(&[u64::MAX]).encode();

        assert!(matches!(Manifest::decode(&bytes), Err(Error::Malformed)));
    }

    #[test]
    fn lengths_that_overflow_their_sum_are_not_a_manifest() {
        let bytes = of(&[u64::MAX, 1]).encode();

        assert!(matches!(Manifest::decode(&bytes), Err(Error::Malformed)));
    }

    #[test]
    fn a_content_merely_large_is_still_a_manifest() {
        // Two chunks of four gigabytes are more than this machine wants to read in one go, and
        // nothing about them is inconsistent: the bound is what cannot be held at all.
        let manifest = of(&[4 << 30, 4 << 30]);
        let decoded = Manifest::decode(&manifest.encode()).unwrap();

        assert_eq!(decoded, manifest);
        assert_eq!(decoded.content_len(), 8 << 30);
    }

    #[test]
    fn a_sum_that_does_not_fit_is_answered_with_nothing_rather_than_wrapping() {
        // A manifest built a chunk at a time has not been through `decode`, so the sum it reports is
        // the one a caller has to check rather than trust.
        let manifest = of(&[u64::MAX, 1]);

        assert_eq!(manifest.checked_content_len(), None);
        assert_eq!(manifest.content_len(), u64::MAX);
    }
}

use crate::{BLAKE3_HASH_LEN, Blake3Hash, Error, Key, split_u64};

/// The magic every pack index starts with.
///
/// A pack's `.pack` holds nothing but entries and needs no preamble — each entry begins with the
/// frame it would have had loose — so the file that has to say what it is is the index beside it.
pub const PACK_INDEX_MAGIC: [u8; 4] = *b"RLPI";

/// The version of the pack index format this build writes.
///
/// It went to two when the key records stopped carrying a hash algorithm, which the store had only
/// ever written one of: a record is now a digest, an offset and a length, and an index written by
/// the version before is refused rather than misread.
pub const PACK_INDEX_VERSION: u8 = 2;

/// Where one object sits inside a pack, and how long it is.
///
/// `len` counts the whole entry — the frame as well as the payload — so an object read out of a
/// pack is read as the very bytes it would have been loose: packing changes where content lives,
/// and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackEntry {
    /// The key the object is stored under.
    key: Key,
    /// How many bytes into the pack the entry starts.
    offset: u64,
    /// How many bytes of the pack the entry takes up, frame and all.
    len: u64,
}

impl PackEntry {
    /// An entry of `len` bytes kept at `offset` in the pack, under `key`.
    #[must_use]
    pub const fn new(key: Key, offset: u64, len: u64) -> Self {
        Self { key, offset, len }
    }

    /// The key the object is stored under.
    #[must_use]
    pub const fn key(&self) -> Key {
        self.key
    }

    /// How many bytes into the pack the entry starts.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// How many bytes of the pack the entry takes up.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Whether the entry holds nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The directory of a pack: which objects it holds and where each one sits.
///
/// The entries are kept in key order, which is what a caller looks one up by — a pack may hold
/// thousands of objects, and finding one by reading the whole directory is the difference between
/// a read that is worth making and one that is not. The order is not a caller's to get wrong: an
/// index is put into it as it is built — see [`new`](Self::new) and [`push`](Self::push) — and an
/// index *read back* out of order is refused, so a lookup by binary search always answers what a
/// walk of the entries would.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackIndex {
    /// The entries, in key order.
    entries: Vec<PackEntry>,
}

impl PackIndex {
    /// An index of `entries`, put into key order.
    ///
    /// Whatever order they arrive in, what is handed back is one they can be found in. A key named
    /// twice is not something an index holds: the entries of one index name distinct objects, which
    /// is what [`decode`](Self::decode) checks for, so a caller with a duplicate has one object named
    /// twice rather than two objects.
    #[must_use]
    pub fn new(mut entries: Vec<PackEntry>) -> Self {
        entries.sort_unstable_by_key(|entry| entry.key);

        Self { entries }
    }

    /// The entries, in key order.
    #[must_use]
    pub fn entries(&self) -> &[PackEntry] {
        &self.entries
    }

    /// How many objects the pack holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the pack holds nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Where the object stored under `key` sits, if the pack holds it.
    #[must_use]
    pub fn find(&self, key: &Key) -> Option<&PackEntry> {
        self.entries
            .binary_search_by(|entry| entry.key.cmp(key))
            .ok()
            .map(|at| &self.entries[at])
    }

    /// Adds `entry`, keeping the index in key order.
    ///
    /// An entry that follows the ones already here is added after them, which is what building an
    /// index from keys already in order costs; one that does not is put where it belongs, so an
    /// index built in any order is still one [`find`](Self::find) can be trusted with.
    pub fn push(&mut self, entry: PackEntry) {
        let at = self.entries.partition_point(|held| held.key < entry.key);
        self.entries.insert(at, entry);
    }

    /// Writes the index down, so it can be read back by [`decode`](Self::decode).
    ///
    /// An index is a header and then one record per object — the digest, the offset and the length —
    /// which is everything a reader needs to reach an entry and nothing about what the entry means.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN + self.entries.len() * RECORD_LEN);

        bytes.extend_from_slice(&PACK_INDEX_MAGIC);
        bytes.push(PACK_INDEX_VERSION);
        bytes.extend_from_slice(&(self.entries.len() as u64).to_be_bytes());

        for entry in &self.entries {
            bytes.extend_from_slice(entry.key.digest());
            bytes.extend_from_slice(&entry.offset.to_be_bytes());
            bytes.extend_from_slice(&entry.len.to_be_bytes());
        }

        bytes
    }

    /// Reads an index written by [`encode`](Self::encode).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if `bytes` does not read as an index — the wrong magic, a
    /// version this build does not know, a count or a record that does not fit, entries out of key
    /// order, or bytes left over at the end.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let rest = bytes
            .strip_prefix(PACK_INDEX_MAGIC.as_slice())
            .ok_or(Error::Malformed)?;
        let (version, rest) = rest.split_first().ok_or(Error::Malformed)?;
        if *version != PACK_INDEX_VERSION {
            return Err(Error::Malformed);
        }

        let (count, mut rest) = split_u64(rest).ok_or(Error::Malformed)?;
        let count = usize::try_from(count).map_err(|_| Error::Malformed)?;

        // The count is what the file *says*, not what it holds: a count larger than the bytes left
        // could hold is one no file of this size can mean, and reserving room for it would be
        // reserving room a corrupt count asked for. The loop below is what refuses a count that
        // does not hold up.
        let mut entries = Vec::with_capacity(count.min(rest.len() / RECORD_LEN));

        for _ in 0..count {
            let (digest, after) = rest
                .split_at_checked(BLAKE3_HASH_LEN)
                .ok_or(Error::Malformed)?;
            let digest: Blake3Hash = digest.try_into().map_err(|_| Error::Malformed)?;
            let (offset, after) = split_u64(after).ok_or(Error::Malformed)?;
            let (len, after) = split_u64(after).ok_or(Error::Malformed)?;
            let key = Key::new(digest);

            // Key order is what a lookup by binary search stands on, so an index that is not in it
            // is one this build will not hand back. A key repeated is out of order as well, which
            // is the right answer: one key has one entry.
            if entries
                .last()
                .is_some_and(|previous: &PackEntry| previous.key >= key)
            {
                return Err(Error::Malformed);
            }

            entries.push(PackEntry::new(key, offset, len));
            rest = after;
        }

        // A record that was not asked about is not one to read past: an index says exactly how many
        // entries it holds, and anything after them is not part of it.
        if !rest.is_empty() {
            return Err(Error::Malformed);
        }

        Ok(Self::new(entries))
    }
}

/// How long an index header is: the magic, the version, and the count.
const HEADER_LEN: usize = PACK_INDEX_MAGIC.len() + 1 + size_of::<u64>();

/// How long one entry record is: the digest, the offset and the length.
const RECORD_LEN: usize = BLAKE3_HASH_LEN + 2 * size_of::<u64>();

#[cfg(test)]
mod tests {
    use super::{PackEntry, PackIndex};
    use crate::Key;

    /// A key from a byte, so a test names the same one twice.
    fn key(seed: u8) -> Key {
        Key::new([seed; 32])
    }

    #[test]
    fn an_index_comes_back_as_it_went_in() {
        let index = PackIndex::new(vec![
            PackEntry::new(key(1), 0, 40),
            PackEntry::new(key(2), 40, 80),
        ]);

        assert_eq!(PackIndex::decode(&index.encode()).unwrap(), index);
    }

    #[test]
    fn an_entry_is_found_by_its_key() {
        let index = PackIndex::new(vec![
            PackEntry::new(key(1), 0, 40),
            PackEntry::new(key(2), 40, 80),
        ]);

        assert_eq!(index.find(&key(2)).map(PackEntry::offset), Some(40));
        assert_eq!(index.find(&key(9)), None);
    }

    #[test]
    fn what_does_not_read_as_an_index_is_refused() {
        let index = PackIndex::new(vec![PackEntry::new(key(1), 0, 40)]);
        let mut bytes = index.encode();

        // The magic is what says a file is an index at all.
        assert!(PackIndex::decode(b"not an index").is_err());

        // The count says how many records follow, so a file shorter than they say is cut short.
        bytes.truncate(bytes.len() - 1);
        assert!(PackIndex::decode(&bytes).is_err());

        // And a file longer than the count says is not one to read past.
        let mut extra = index.encode();
        extra.push(0);
        assert!(PackIndex::decode(&extra).is_err());
    }

    #[test]
    fn an_index_puts_its_entries_in_key_order_however_they_arrive() {
        // Built out of order, and by pushing out of order: either way what is handed back is one a
        // lookup by binary search can be trusted with.
        let built = PackIndex::new(vec![
            PackEntry::new(key(2), 40, 80),
            PackEntry::new(key(1), 0, 40),
        ]);
        assert_eq!(built.find(&key(1)).map(PackEntry::offset), Some(0));
        assert_eq!(built.find(&key(2)).map(PackEntry::offset), Some(40));

        let mut pushed = PackIndex::default();
        pushed.push(PackEntry::new(key(2), 40, 80));
        pushed.push(PackEntry::new(key(1), 0, 40));
        assert_eq!(pushed.find(&key(1)).map(PackEntry::offset), Some(0));
        assert_eq!(pushed.find(&key(2)).map(PackEntry::offset), Some(40));
    }

    #[test]
    fn an_index_not_in_key_order_is_refused() {
        // A file out of order, as one that went wrong on disk would be — the builders here cannot
        // make one, which is the point of them.
        let mut out_of_order = Vec::new();
        out_of_order.extend_from_slice(&super::PACK_INDEX_MAGIC);
        out_of_order.push(super::PACK_INDEX_VERSION);
        out_of_order.extend_from_slice(&2_u64.to_be_bytes());
        for entry in [
            PackEntry::new(key(2), 0, 40),
            PackEntry::new(key(1), 40, 80),
        ] {
            out_of_order.extend_from_slice(entry.key().digest());
            out_of_order.extend_from_slice(&entry.offset().to_be_bytes());
            out_of_order.extend_from_slice(&entry.len().to_be_bytes());
        }

        assert!(matches!(
            PackIndex::decode(&out_of_order),
            Err(crate::Error::Malformed)
        ));

        // A repeat of one key is not in order either: one key has one entry.
        let repeated = PackIndex::new(vec![
            PackEntry::new(key(1), 0, 40),
            PackEntry::new(key(1), 40, 80),
        ])
        .encode();
        assert!(matches!(
            PackIndex::decode(&repeated),
            Err(crate::Error::Malformed)
        ));
    }
}

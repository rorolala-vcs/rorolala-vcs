//! The reverse records an inverse index is made of, and how they are written down.
//!
//! A record is kept under the key it is about and holds the objects that point at that key, each
//! with the role it points by. A version's number is the one derived value here; it stands beside
//! the records because the question it answers — what number is this version — is asked of the same
//! index, and because the records are what make filling it cheap.

use std::collections::{BTreeMap, BTreeSet};

use rorolala_vcs::Hash;

use crate::{Edge, InverseIndexError};

/// What a file this build writes leads with.
const MAGIC: &[u8; 8] = b"ROLAINV1";

/// The format this build writes; a file written by another is not read as this one.
const FORMAT: u32 = 1;

/// Every reverse record, under the key it is about.
#[derive(Debug, Default, Clone)]
pub struct Table {
    /// The object keys the index held when this was built, in key order.
    covered: Vec<Hash>,
    /// The records, by the key they are about.
    entries: BTreeMap<Hash, ReverseEntry>,
}

/// The objects that point at one key, and a version's number.
#[derive(Debug, Default, Clone)]
struct ReverseEntry {
    /// The sources, each with the role it points by.
    dependents: BTreeSet<(Hash, Edge)>,
    /// The version's number, when it has one.
    number: Option<u64>,
}

impl Table {
    /// A table covering `covered`, which is the object keys it describes.
    #[must_use]
    pub const fn new(covered: Vec<Hash>) -> Self {
        Self {
            covered,
            entries: BTreeMap::new(),
        }
    }

    /// Notes that `source` points at `target` by `edge`.
    pub fn add(&mut self, target: Hash, source: Hash, edge: Edge) {
        self.entries
            .entry(target)
            .or_default()
            .dependents
            .insert((source, edge));
    }

    /// Records the number of the version named `key`.
    pub fn set_number(&mut self, key: Hash, number: u64) {
        self.entries.entry(key).or_default().number = Some(number);
    }

    /// The object keys the index held when this was built.
    #[must_use]
    pub fn covered(&self) -> &[Hash] {
        &self.covered
    }

    /// Whether this describes exactly `current`: the same objects, no more and no fewer.
    ///
    /// The keys are in the order [`covered`](Self::covered) was written in, which is key order, and
    /// so is the answer a listing gives: the two are compared as they are rather than sorted again.
    #[must_use]
    pub fn is_fresh(&self, current: &[Hash]) -> bool {
        self.covered.len() == current.len() && self.covered.iter().eq(current.iter())
    }

    /// The dependents of `key` pointed at by `edge`, in key order.
    #[must_use]
    pub fn dependents(&self, key: Hash, edge: Edge) -> Vec<Hash> {
        self.entries.get(&key).map_or_else(Vec::new, |entry| {
            entry
                .dependents
                .iter()
                .filter(|(_, role)| *role == edge)
                .map(|(source, _)| *source)
                .collect()
        })
    }

    /// The number recorded for `key`, if one is.
    #[must_use]
    pub fn number(&self, key: Hash) -> Option<u64> {
        self.entries.get(&key).and_then(|entry| entry.number)
    }

    /// How many records there are.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// How many dependents are noted across every record.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.entries
            .values()
            .map(|entry| entry.dependents.len())
            .sum()
    }

    /// The bytes this table is written down as.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&width(self.covered.len()).to_be_bytes());
        for key in &self.covered {
            body.extend_from_slice(key.digest());
        }

        body.extend_from_slice(&width(self.entries.len()).to_be_bytes());
        for (key, entry) in &self.entries {
            body.extend_from_slice(key.digest());
            if let Some(number) = entry.number {
                body.push(1);
                body.extend_from_slice(&number.to_be_bytes());
            } else {
                body.push(0);
            }

            body.extend_from_slice(&width(entry.dependents.len()).to_be_bytes());
            for (source, edge) in &entry.dependents {
                body.extend_from_slice(source.digest());
                body.push(edge.id());
            }
        }

        let mut bytes = Vec::with_capacity(MAGIC.len() + 4 + 32 + body.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&FORMAT.to_be_bytes());
        bytes.extend_from_slice(blake3::hash(&body).as_bytes());
        bytes.extend_from_slice(&body);

        bytes
    }

    /// The table `bytes` holds, if they are one this build wrote.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError::Malformed`] if the bytes do not lead with the magic and format
    /// this build writes, do not come back as what their digest names, or do not hold the records
    /// their counts promise.
    pub fn decode(bytes: &[u8]) -> Result<Self, InverseIndexError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(MAGIC.len())? != MAGIC {
            return Err(InverseIndexError::Malformed);
        }

        let format = u32::from_be_bytes(
            cursor
                .take(4)?
                .try_into()
                .map_err(|_| InverseIndexError::Malformed)?,
        );
        if format != FORMAT {
            return Err(InverseIndexError::Malformed);
        }

        let expected = cursor.take(32)?;
        let body = cursor.rest();
        if blake3::hash(body).as_bytes() != expected {
            return Err(InverseIndexError::Malformed);
        }

        let mut body = Cursor::new(body);
        let covered_len = body.len()?;
        let mut covered = Vec::with_capacity(covered_len);
        for _ in 0..covered_len {
            covered.push(body.key()?);
        }

        let entries_len = body.len()?;
        let mut entries = BTreeMap::new();
        for _ in 0..entries_len {
            let key = body.key()?;
            let entry = body.entry()?;
            if entries.insert(key, entry).is_some() {
                return Err(InverseIndexError::Malformed);
            }
        }

        if !body.is_empty() {
            return Err(InverseIndexError::Malformed);
        }

        Ok(Self { covered, entries })
    }
}

/// `len` as the width this format writes counts in.
fn width(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// A cursor over the bytes of a table.
struct Cursor<'a> {
    /// What is left to read.
    bytes: &'a [u8],
}

impl<'a> Cursor<'a> {
    /// A cursor at the front of `bytes`.
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// The next `len` bytes, and moves past them.
    const fn take(&mut self, len: usize) -> Result<&'a [u8], InverseIndexError> {
        if self.bytes.len() < len {
            return Err(InverseIndexError::Malformed);
        }

        let (head, rest) = self.bytes.split_at(len);
        self.bytes = rest;

        Ok(head)
    }

    /// What is left, without moving.
    const fn rest(&self) -> &'a [u8] {
        self.bytes
    }

    /// Whether everything has been read.
    const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The next byte.
    fn u8(&mut self) -> Result<u8, InverseIndexError> {
        Ok(self.take(1)?[0])
    }

    /// The next eight bytes, big endian.
    fn u64(&mut self) -> Result<u64, InverseIndexError> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| InverseIndexError::Malformed)?;

        Ok(u64::from_be_bytes(bytes))
    }

    /// The next eight bytes as a count.
    fn len(&mut self) -> Result<usize, InverseIndexError> {
        usize::try_from(self.u64()?).map_err(|_| InverseIndexError::Malformed)
    }

    /// The next thirty-two bytes as a key.
    fn key(&mut self) -> Result<Hash, InverseIndexError> {
        let bytes: [u8; 32] = self
            .take(32)?
            .try_into()
            .map_err(|_| InverseIndexError::Malformed)?;

        Ok(Hash::new(bytes))
    }

    /// The next record: an optional number and the dependents.
    fn entry(&mut self) -> Result<ReverseEntry, InverseIndexError> {
        let number = match self.u8()? {
            0 => None,
            1 => Some(self.u64()?),
            _ => return Err(InverseIndexError::Malformed),
        };

        let count = self.len()?;
        let mut dependents = BTreeSet::new();
        for _ in 0..count {
            let source = self.key()?;
            let edge = Edge::from_id(self.u8()?).ok_or(InverseIndexError::Malformed)?;
            dependents.insert((source, edge));
        }

        Ok(ReverseEntry { dependents, number })
    }
}

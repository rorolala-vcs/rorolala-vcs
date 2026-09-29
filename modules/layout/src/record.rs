//! One change, as it is written to a shard's log.
//!
//! A change is a whole, self-describing record: a length, a checksum over the rest, and then the
//! kind of change with what it names. A reader takes records off the front until it meets one that
//! is not all there — which is what a write cut short by a crash leaves — and stops, so whatever
//! was written whole is what is read back.
//!
//! Every record carries the `Uuid` it is about, which is what decides the shard it is appended to:
//! a change is one append of one small buffer, so two writers appending at once cannot tear each
//! other's record.

use uuid::Uuid;

use crate::bytes::{
    crc32, put_data, put_str, put_uuid, rest_empty, take_data, take_str, take_uuid,
};
use crate::data::MutableData;
use crate::error::LayoutError;

/// The number each kind of change is written down by.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    /// A path was bound to a `Uuid`.
    CreatePath = 0,
    /// A path stopped naming its `Uuid`.
    RemovePath = 1,
    /// A `Uuid` was moved from one path to another.
    MovePath = 2,
    /// An entry was created.
    CreateEntry = 3,
    /// An entry was dropped.
    RemoveEntry = 4,
    /// An entry's data was changed.
    UpdateEntry = 5,
}

impl Tag {
    /// The number this kind is written down by.
    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// The kind written down by `number`, if this build knows one.
    #[must_use]
    pub const fn from_id(number: u8) -> Option<Self> {
        match number {
            0 => Some(Self::CreatePath),
            1 => Some(Self::RemovePath),
            2 => Some(Self::MovePath),
            3 => Some(Self::CreateEntry),
            4 => Some(Self::RemoveEntry),
            5 => Some(Self::UpdateEntry),
            _ => None,
        }
    }
}

/// One change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// A path was bound to `id`.
    CreatePath {
        /// The `Uuid` the path now names.
        id: Uuid,
        /// The path, normalized.
        path: String,
    },
    /// The path stopped naming `id`.
    RemovePath {
        /// The `Uuid` the path named.
        id: Uuid,
        /// The path, normalized.
        path: String,
    },
    /// `id` was moved from `from` to `to`.
    MovePath {
        /// The `Uuid` that moved.
        id: Uuid,
        /// The path it was at.
        from: String,
        /// The path it is at now.
        to: String,
    },
    /// `id` was created, with `data`.
    CreateEntry {
        /// The `Uuid` that was created.
        id: Uuid,
        /// What it holds.
        data: MutableData,
    },
    /// `id` was dropped.
    RemoveEntry {
        /// The `Uuid` that was dropped.
        id: Uuid,
    },
    /// `id`'s data was changed to `data`.
    UpdateEntry {
        /// The `Uuid` that changed.
        id: Uuid,
        /// What it holds now.
        data: MutableData,
    },
}

impl Record {
    /// The `Uuid` this change is about, which decides the shard it is appended to.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        match self {
            Self::CreatePath { id, .. }
            | Self::RemovePath { id, .. }
            | Self::MovePath { id, .. }
            | Self::CreateEntry { id, .. }
            | Self::RemoveEntry { id }
            | Self::UpdateEntry { id, .. } => *id,
        }
    }

    /// This change, as the bytes appended to a shard's log.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::CreatePath { id, path } => {
                let mut fields = Vec::new();
                put_str(&mut fields, path);
                wrap(Tag::CreatePath, *id, fields)
            }
            Self::RemovePath { id, path } => {
                let mut fields = Vec::new();
                put_str(&mut fields, path);
                wrap(Tag::RemovePath, *id, fields)
            }
            Self::MovePath { id, from, to } => {
                let mut fields = Vec::new();
                put_str(&mut fields, from);
                put_str(&mut fields, to);
                wrap(Tag::MovePath, *id, fields)
            }
            Self::CreateEntry { id, data } => {
                let mut fields = Vec::new();
                put_data(&mut fields, data);
                wrap(Tag::CreateEntry, *id, fields)
            }
            Self::RemoveEntry { id } => wrap(Tag::RemoveEntry, *id, Vec::new()),
            Self::UpdateEntry { id, data } => {
                let mut fields = Vec::new();
                put_data(&mut fields, data);
                wrap(Tag::UpdateEntry, *id, fields)
            }
        }
    }

    /// The change `bytes` starts with, and how many bytes of them it took.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Malformed`] if `bytes` holds a whole record this build cannot read.
    /// Returns `Ok(None)` if `bytes` does not hold a whole record yet — what a write cut short
    /// leaves, and what a reader stops at.
    pub fn decode(bytes: &[u8]) -> Result<Option<(Self, usize)>, LayoutError> {
        let Some(head) = bytes.get(..HEAD_LEN) else {
            return Ok(None);
        };

        let length = usize::try_from(u32::from_be_bytes(head[..4].try_into().unwrap_or_default()))
            .map_err(|_| LayoutError::Malformed)?;
        if length < HEAD_LEN || bytes.len() < length {
            return Ok(None);
        }

        let checksum = u32::from_be_bytes(head[4..8].try_into().unwrap_or_default());
        let body = &bytes[HEAD_LEN..length];
        if crc32(body) != checksum {
            return Err(LayoutError::Malformed);
        }

        let (&tag, rest) = body.split_first().ok_or(LayoutError::Malformed)?;
        let (id, rest) = take_uuid(rest)?;
        let tag = Tag::from_id(tag).ok_or(LayoutError::Malformed)?;

        let record = match tag {
            Tag::CreatePath => {
                let (path, rest) = take_str(rest)?;
                rest_empty(rest)?;
                Self::CreatePath {
                    id,
                    path: path.to_owned(),
                }
            }
            Tag::RemovePath => {
                let (path, rest) = take_str(rest)?;
                rest_empty(rest)?;
                Self::RemovePath {
                    id,
                    path: path.to_owned(),
                }
            }
            Tag::MovePath => {
                let (from, rest) = take_str(rest)?;
                let (to, rest) = take_str(rest)?;
                rest_empty(rest)?;
                Self::MovePath {
                    id,
                    from: from.to_owned(),
                    to: to.to_owned(),
                }
            }
            Tag::CreateEntry => {
                let (data, rest) = take_data(rest)?;
                rest_empty(rest)?;
                Self::CreateEntry { id, data }
            }
            Tag::RemoveEntry => {
                rest_empty(rest)?;
                Self::RemoveEntry { id }
            }
            Tag::UpdateEntry => {
                let (data, rest) = take_data(rest)?;
                rest_empty(rest)?;
                Self::UpdateEntry { id, data }
            }
        };

        Ok(Some((record, length)))
    }
}

/// How long the head every record starts with is: the length and the checksum.
const HEAD_LEN: usize = 8;

/// Wraps a kind, a `Uuid` and the fields that follow into a whole record.
fn wrap(tag: Tag, id: Uuid, fields: Vec<u8>) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + 16 + fields.len());
    body.push(tag.id());
    put_uuid(&mut body, id);
    body.extend(fields);

    // The length covers the 8-byte head and the checksummed body together, so a reader knows a
    // whole record from a cut one before it checks anything.
    let length = u32::try_from(HEAD_LEN + body.len()).unwrap_or(u32::MAX);
    let checksum = crc32(&body);

    let mut record = Vec::with_capacity(HEAD_LEN + body.len());
    record.extend_from_slice(&length.to_be_bytes());
    record.extend_from_slice(&checksum.to_be_bytes());
    record.extend_from_slice(&body);

    record
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::Record;
    use crate::data::MutableData;

    #[test]
    fn every_kind_of_change_round_trips_through_its_bytes() {
        let id = Uuid::from_u128(7);
        let data = MutableData::new(Some("alice".to_owned()), [3; 32], "first".to_owned());

        for record in [
            Record::CreatePath {
                id,
                path: "a/b".to_owned(),
            },
            Record::RemovePath {
                id,
                path: "a/b".to_owned(),
            },
            Record::MovePath {
                id,
                from: "a/b".to_owned(),
                to: "c/d".to_owned(),
            },
            Record::CreateEntry {
                id,
                data: data.clone(),
            },
            Record::RemoveEntry { id },
            Record::UpdateEntry { id, data },
        ] {
            let bytes = record.encode();
            let (read, used) = Record::decode(&bytes).unwrap().unwrap();

            assert_eq!(read, record);
            assert_eq!(used, bytes.len());
        }
    }

    #[test]
    fn a_record_that_is_not_all_there_reads_as_nothing() {
        let id = Uuid::from_u128(9);
        let bytes = Record::CreatePath {
            id,
            path: "a/b".to_owned(),
        }
        .encode();

        for cut in 0..bytes.len() {
            assert!(Record::decode(&bytes[..cut]).unwrap().is_none(), "{cut}");
        }

        assert!(Record::decode(&bytes).unwrap().is_some());
    }

    #[test]
    fn a_record_whose_checksum_does_not_hold_is_refused() {
        let id = Uuid::from_u128(11);
        let mut bytes = Record::CreatePath {
            id,
            path: "a/b".to_owned(),
        }
        .encode();

        let last = bytes.last_mut().unwrap();
        *last ^= 0xff;

        assert!(Record::decode(&bytes).is_err());
    }
}

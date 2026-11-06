//! A shard written down whole.
//!
//! A snapshot is what makes a log droppable: it says everything a shard holds as of the moment it
//! was written, so the log that led to it can be thrown away and the log that follows it starts
//! empty. It is written beside where it belongs and moved into place, so the snapshot a reader
//! finds is always one written whole.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;

use uuid::Uuid;

use crate::bytes::{
    crc32, put_count, put_data, put_str, put_uuid, rest_empty, take_checksum, take_count,
    take_data, take_data_v1, take_str, take_uuid,
};
use crate::error::LayoutError;
use crate::path::LayoutPath;
use crate::slot::Slot;

/// What a snapshot starts with, so one this build did not write is refused rather than misread.
const MAGIC: &[u8; 4] = b"RLAY";

/// The version of the snapshot this build writes.
///
/// A snapshot carries what an entry's data is, and that shape changed twice: when an entry gained a
/// preference, and when an entry gained a kind. The version is what refuses a snapshot this build
/// cannot read by name, rather than leaving the refusal to a decoder that would read the next
/// entry's bytes as this one's and call the result malformed. Version 2 is read as file entries, so
/// a snapshot written before trees is still a Layout this build works in.
const VERSION: u8 = 3;

/// The version before entries had a kind, which reads as file entries.
const LEGACY_VERSION: u8 = 2;

/// Writes `slots` to `path` as a whole shard.
///
/// # Errors
///
/// Returns why the snapshot could not be written.
pub fn write(path: &Path, slots: &HashMap<Uuid, Slot>) -> Result<(), LayoutError> {
    let bytes = encode(slots);

    // The name is one writer's own, so two writers compacting at once do not write over each
    // other's half-written snapshot.
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)?;

    Ok(())
}

/// `slots` written down whole, as [`write`] writes them.
///
/// It is what a layout is copied *as*: a shard's memory written out is the whole of what the shard
/// is at that moment, where the files on disk may be a snapshot and a log that are a change apart.
///
/// The entries are written in `Uuid` order rather than in the order the map yields them, so what one
/// shard holds is the same bytes whenever it is written: a copy is compared with the layout it came
/// from by its bytes, and a map's order is a different answer in every process.
pub fn encode(slots: &HashMap<Uuid, Slot>) -> Vec<u8> {
    let mut ordered: Vec<(&Uuid, &Slot)> = slots.iter().collect();
    ordered.sort_unstable_by_key(|(id, _)| **id);

    let mut body = Vec::new();
    put_count(&mut body, ordered.len());
    for (id, slot) in ordered {
        put_uuid(&mut body, *id);
        match &slot.path {
            Some(path) => {
                body.push(1);
                put_str(&mut body, path.as_str());
            }
            None => body.push(0),
        }
        match &slot.data {
            Some(data) => {
                body.push(1);
                put_data(&mut body, data);
            }
            None => body.push(0),
        }
    }

    let mut bytes = Vec::with_capacity(MAGIC.len() + 1 + 4 + body.len());
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.extend_from_slice(&crc32(&body).to_be_bytes());
    bytes.extend_from_slice(&body);

    bytes
}

/// The shard `bytes` hold, which [`encode`] is what made.
///
/// # Errors
///
/// Returns [`LayoutError::Malformed`] if the bytes are not a snapshot this build wrote.
pub fn decode(bytes: &[u8]) -> Result<HashMap<Uuid, Slot>, LayoutError> {
    let (magic, rest) = bytes
        .split_at_checked(MAGIC.len())
        .ok_or(LayoutError::Malformed)?;
    if magic != MAGIC {
        return Err(LayoutError::Malformed);
    }

    let (&version, rest) = rest.split_first().ok_or(LayoutError::Malformed)?;
    // Version 2 is the shape before entries had a kind, and its files are still files; version 3 is
    // what this build writes. Anything else is a snapshot whose bytes may mean anything.
    if version != VERSION && version != LEGACY_VERSION {
        return Err(LayoutError::Malformed);
    }

    let (checksum, rest) = take_checksum(rest)?;
    if crc32(rest) != checksum {
        return Err(LayoutError::Malformed);
    }

    let (count, mut rest) = take_count(rest)?;
    let mut slots = HashMap::with_capacity(count.min(1024));
    for _ in 0..count {
        let (id, after) = take_uuid(rest)?;
        rest = after;

        let (&has_path, after) = rest.split_first().ok_or(LayoutError::Malformed)?;
        rest = after;
        let path = match has_path {
            0 => None,
            1 => {
                let (text, after) = take_str(rest)?;
                rest = after;
                Some(LayoutPath::from_stored(text)?)
            }
            _ => return Err(LayoutError::Malformed),
        };

        let (&has_data, after) = rest.split_first().ok_or(LayoutError::Malformed)?;
        rest = after;
        let data = match has_data {
            0 => None,
            1 => {
                let (data, after) = if version == LEGACY_VERSION {
                    take_data_v1(rest)?
                } else {
                    take_data(rest)?
                };
                rest = after;
                Some(data)
            }
            _ => return Err(LayoutError::Malformed),
        };

        slots.insert(id, Slot { path, data });
    }

    rest_empty(rest)?;

    Ok(slots)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;

    use super::{LEGACY_VERSION, MAGIC, VERSION, decode, encode};
    use crate::bytes::{crc32, put_count, put_data_without_kind, put_str, put_uuid};
    use crate::data::{EntryKind, MutableData};
    use crate::path::LayoutPath;
    use crate::slot::Slot;

    /// A snapshot as a build before trees wrote it.
    fn legacy(version: u8) -> Vec<u8> {
        let id = Uuid::from_u128(21);
        let data = MutableData::new(Some("alice".to_owned()), [7; 32], "legacy".to_owned());

        let mut body = Vec::new();
        put_count(&mut body, 1);
        put_uuid(&mut body, id);
        body.push(1);
        put_str(&mut body, "a/b");
        body.push(1);
        put_data_without_kind(&mut body, &data);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.push(version);
        bytes.extend_from_slice(&crc32(&body).to_be_bytes());
        bytes.extend_from_slice(&body);

        bytes
    }

    #[test]
    fn a_snapshot_written_before_trees_reads_as_files() {
        let slots = decode(&legacy(LEGACY_VERSION)).unwrap();
        let id = Uuid::from_u128(21);
        let slot = slots.get(&id).unwrap();

        assert_eq!(slot.path.as_ref().map(LayoutPath::as_str), Some("a/b"));
        assert_eq!(
            slot.data.as_ref().map(MutableData::kind),
            Some(EntryKind::File)
        );
    }

    #[test]
    fn a_snapshot_this_build_writes_keeps_the_kind_of_an_entry() {
        let id = Uuid::from_u128(22);
        let mut slots = HashMap::new();
        slots.insert(
            id,
            Slot {
                path: Some(LayoutPath::root()),
                data: Some(
                    MutableData::new(None, [8; 32], String::new()).with_kind(EntryKind::Tree),
                ),
            },
        );

        let bytes = encode(&slots);
        assert_eq!(bytes[MAGIC.len()], VERSION);

        let read = decode(&bytes).unwrap();
        let slot = read.get(&id).unwrap();
        assert_eq!(slot.path, Some(LayoutPath::root()));
        assert_eq!(
            slot.data.as_ref().map(MutableData::kind),
            Some(EntryKind::Tree)
        );
    }
}

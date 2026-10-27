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
    take_data, take_str, take_uuid,
};
use crate::error::LayoutError;
use crate::path::LayoutPath;
use crate::slot::Slot;

/// What a snapshot starts with, so one this build did not write is refused rather than misread.
const MAGIC: &[u8; 4] = b"RLAY";

/// The version of the snapshot this build writes.
///
/// A snapshot carries what an entry's data is, and that shape changed when an entry gained a
/// preference. The version is what refuses an older one by name, rather than leaving the refusal to
/// a decoder that would read the next entry's bytes as this one's and call the result malformed.
const VERSION: u8 = 2;

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
    if version != VERSION {
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
                Some(LayoutPath::new(text)?)
            }
            _ => return Err(LayoutError::Malformed),
        };

        let (&has_data, after) = rest.split_first().ok_or(LayoutError::Malformed)?;
        rest = after;
        let data = match has_data {
            0 => None,
            1 => {
                let (data, after) = take_data(rest)?;
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

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

/// Writes `slots` to `path` as a whole shard.
///
/// # Errors
///
/// Returns why the snapshot could not be written.
pub fn write(path: &Path, slots: &HashMap<Uuid, Slot>) -> Result<(), LayoutError> {
    let mut body = Vec::new();
    put_count(&mut body, slots.len());
    for (id, slot) in slots {
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

    let mut bytes = Vec::with_capacity(MAGIC.len() + 4 + body.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&crc32(&body).to_be_bytes());
    bytes.extend_from_slice(&body);

    // The name is one writer's own, so two writers compacting at once do not write over each
    // other's half-written snapshot.
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)?;

    Ok(())
}

/// The shard `path` holds, or nothing when there is no snapshot there.
///
/// # Errors
///
/// Returns [`LayoutError::Io`] if it could not be read, and [`LayoutError::Malformed`] if it is not
/// a snapshot this build wrote.
pub fn read(path: &Path) -> Result<Option<HashMap<Uuid, Slot>>, LayoutError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };

    let (magic, rest) = bytes
        .split_at_checked(MAGIC.len())
        .ok_or(LayoutError::Malformed)?;
    if magic != MAGIC {
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

    Ok(Some(slots))
}

//! The little encodings a layout writes down.
//!
//! Everything a layout keeps on disk — a change in a log, a shard in a snapshot — is written with
//! the same few shapes: a UUID, a string with its length in front, and the data an entry holds.
//! They are kept here so both writers cannot come to disagree about what those bytes mean.

use uuid::Uuid;

use crate::data::MutableData;
use crate::error::LayoutError;

/// Writes a UUID as the sixteen bytes it is.
pub fn put_uuid(out: &mut Vec<u8>, id: Uuid) {
    out.extend_from_slice(id.as_bytes());
}

/// Reads the sixteen bytes a UUID is.
///
/// # Errors
///
/// Returns [`LayoutError::Malformed`] if there are not sixteen bytes left.
pub fn take_uuid(bytes: &[u8]) -> Result<(Uuid, &[u8]), LayoutError> {
    let (id, rest) = bytes.split_at_checked(16).ok_or(LayoutError::Malformed)?;
    let id: [u8; 16] = id.try_into().map_err(|_| LayoutError::Malformed)?;

    Ok((Uuid::from_bytes(id), rest))
}

/// Writes a string: how long it is, then its UTF-8 bytes.
pub fn put_str(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    let length = usize::try_from(length).unwrap_or(0);

    out.extend_from_slice(&u32::try_from(length).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(&bytes[..length]);
}

/// Reads what [`put_str`] wrote.
///
/// # Errors
///
/// Returns [`LayoutError::Malformed`] if the length or the bytes are not all there, or are not
/// UTF-8.
pub fn take_str(bytes: &[u8]) -> Result<(&str, &[u8]), LayoutError> {
    let (head, rest) = bytes.split_at_checked(4).ok_or(LayoutError::Malformed)?;
    let length = usize::try_from(u32::from_be_bytes(head.try_into().unwrap_or_default()))
        .map_err(|_| LayoutError::Malformed)?;

    let (text, rest) = rest
        .split_at_checked(length)
        .ok_or(LayoutError::Malformed)?;

    Ok((
        std::str::from_utf8(text).map_err(|_| LayoutError::Malformed)?,
        rest,
    ))
}

/// Writes the data an entry holds: whether an owner is named and who, the version, and what it says.
pub fn put_data(out: &mut Vec<u8>, data: &MutableData) {
    match data.owner() {
        Some(owner) => {
            out.push(1);
            put_str(out, owner);
        }
        None => out.push(0),
    }

    out.extend_from_slice(&data.version());
    put_str(out, data.description());
}

/// Reads what [`put_data`] wrote.
///
/// # Errors
///
/// Returns [`LayoutError::Malformed`] if what follows is not the data this build writes.
pub fn take_data(bytes: &[u8]) -> Result<(MutableData, &[u8]), LayoutError> {
    let (&flag, rest) = bytes.split_first().ok_or(LayoutError::Malformed)?;
    let (owner, rest) = match flag {
        0 => (None, rest),
        1 => {
            let (owner, rest) = take_str(rest)?;
            (Some(owner.to_owned()), rest)
        }
        _ => return Err(LayoutError::Malformed),
    };

    let (version, rest) = rest.split_at_checked(32).ok_or(LayoutError::Malformed)?;
    let version: [u8; 32] = version.try_into().map_err(|_| LayoutError::Malformed)?;

    let (description, rest) = take_str(rest)?;

    Ok((
        MutableData::new(owner, version, description.to_owned()),
        rest,
    ))
}

/// Writes a count, so a reader knows how many follow.
pub fn put_count(out: &mut Vec<u8>, count: usize) {
    let count = u64::try_from(count).unwrap_or(u64::MAX);
    out.extend_from_slice(&count.to_be_bytes());
}

/// Reads the count [`put_count`] wrote.
///
/// # Errors
///
/// Returns [`LayoutError::Malformed`] if there are not eight bytes left.
pub fn take_count(bytes: &[u8]) -> Result<(usize, &[u8]), LayoutError> {
    let (head, rest) = bytes.split_at_checked(8).ok_or(LayoutError::Malformed)?;
    let count = usize::try_from(u64::from_be_bytes(head.try_into().unwrap_or_default()))
        .map_err(|_| LayoutError::Malformed)?;

    Ok((count, rest))
}

/// Reads the four bytes a checksum is.
///
/// # Errors
///
/// Returns [`LayoutError::Malformed`] if there are not four bytes left.
pub fn take_checksum(bytes: &[u8]) -> Result<(u32, &[u8]), LayoutError> {
    let (head, rest) = bytes.split_at_checked(4).ok_or(LayoutError::Malformed)?;

    Ok((
        u32::from_be_bytes(head.try_into().unwrap_or_default()),
        rest,
    ))
}

/// Accepts `rest` when it is empty, and refuses it otherwise: a value is exactly as long as it says.
///
/// # Errors
///
/// Returns [`LayoutError::Malformed`] if there is anything left.
pub const fn rest_empty(rest: &[u8]) -> Result<(), LayoutError> {
    if rest.is_empty() {
        Ok(())
    } else {
        Err(LayoutError::Malformed)
    }
}

/// The CRC-32 (IEEE) of `bytes`.
///
/// A checksum is what tells a value that was written whole from one a crash cut short, so it only
/// has to catch that.
pub fn crc32(bytes: &[u8]) -> u32 {
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
    use super::{crc32, put_str, take_str};

    #[test]
    fn a_string_round_trips_through_its_bytes() {
        let mut bytes = Vec::new();
        put_str(&mut bytes, "a/b");

        let (text, rest) = take_str(&bytes).unwrap();
        assert_eq!(text, "a/b");
        assert!(rest.is_empty());
    }

    #[test]
    fn a_checksum_is_the_same_for_the_same_bytes() {
        assert_eq!(crc32(b"rorolala"), crc32(b"rorolala"));
        assert_ne!(crc32(b"rorolala"), crc32("rorolalá".as_bytes()));
    }
}

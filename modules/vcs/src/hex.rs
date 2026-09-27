//! Hexadecimal, for the hashes written into structured output.
//!
//! A hash is thirty-two bytes and reads as sixty-four hex characters. In the JSON a `rola
//! vcs-index` command writes, that is the string a hash is written as — the same string
//! [`Key`](rorolala_storage::Key) is written as, without the name of the hash in front of it.

use rorolala_storage::Blake3Hash;

/// The digits a hash is written out with.
const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// The hash, written as 64 lowercase hex characters.
#[must_use]
pub fn to_hex(hash: &Blake3Hash) -> String {
    let mut hex = String::with_capacity(64);

    for byte in hash {
        hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
        hex.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }

    hex
}

/// The hash a 64-character hex string stands for, if it is one.
#[must_use]
pub fn from_hex(text: &str) -> Option<Blake3Hash> {
    if text.len() != 64 {
        return None;
    }

    let mut hash = [0_u8; 32];
    for (at, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let pair = core::str::from_utf8(pair).ok()?;
        hash[at] = u8::from_str_radix(pair, 16).ok()?;
    }

    Some(hash)
}

/// Writing one hash as hex, for `#[serde(with = "crate::hex::hash")]`.
pub mod hash {
    use rorolala_storage::Blake3Hash;
    use serde::{Deserialize as _, Deserializer, Serializer};

    /// Writes the hash as its hex string.
    ///
    /// # Errors
    ///
    /// Whatever the serializer fails with.
    pub fn serialize<S: Serializer>(value: &Blake3Hash, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&super::to_hex(value))
    }

    /// Reads the hex string back to a hash.
    ///
    /// # Errors
    ///
    /// A deserialization error when the string is not a 64-character hex digest.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Blake3Hash, D::Error> {
        let text = String::deserialize(deserializer)?;

        super::from_hex(&text).ok_or_else(|| serde::de::Error::custom("not a hash"))
    }
}

/// Writing one optional hash as hex, for `#[serde(with = "crate::hex::maybe_hash")]`.
pub mod maybe_hash {
    use rorolala_storage::Blake3Hash;
    use serde::{Deserialize as _, Deserializer, Serializer};

    /// Writes the hash as its hex string, or nothing when there is none.
    ///
    /// # Errors
    ///
    /// Whatever the serializer fails with.
    // A serde `with` module is handed the field by reference, so `&Option<..>` is the signature
    // it has to have.
    #[allow(clippy::ref_option)]
    pub fn serialize<S: Serializer>(
        value: &Option<Blake3Hash>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(hash) => serializer.serialize_some(&super::to_hex(hash)),
            None => serializer.serialize_none(),
        }
    }

    /// Reads the hex string back to a hash, or none where there is nothing.
    ///
    /// # Errors
    ///
    /// A deserialization error when a string is there and is not a 64-character hex digest.
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Blake3Hash>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;

        text.map(|text| {
            super::from_hex(&text).ok_or_else(|| serde::de::Error::custom("not a hash"))
        })
        .transpose()
    }
}

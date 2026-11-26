//! Hexadecimal, for the hashes written into structured output.
//!
//! A hash is thirty-two bytes and reads as sixty-four hex characters. In the JSON a `rola
//! vcs-index` command writes, that is the string a hash is written as, the same string
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
    for (at, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
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

#[cfg(test)]
mod tests {
    use rorolala_storage::Blake3Hash;

    use serde::{Deserialize, Serialize};

    use super::{from_hex, to_hex};

    /// A hash written with the module under test.
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Hashed {
        #[serde(with = "crate::hex::hash")]
        hash: Blake3Hash,
    }

    /// A hash that may be absent, written with the module under test.
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct MaybeHashed {
        #[serde(with = "crate::hex::maybe_hash")]
        hash: Option<Blake3Hash>,
    }

    #[test]
    fn a_hash_is_written_as_sixty_four_lowercase_digits() {
        let mut hash: Blake3Hash = [0; 32];
        hash[0] = 0xab;
        hash[31] = 0x01;

        let hex = to_hex(&hash);

        assert_eq!(hex.len(), 64);
        assert!(hex.starts_with("ab"));
        assert!(hex.ends_with("01"));
    }

    #[test]
    fn a_hex_string_reads_back_or_is_refused() {
        let hash: Blake3Hash = [0xcd; 32];
        let hex = to_hex(&hash);

        assert_eq!(from_hex(&hex), Some(hash));
        assert_eq!(from_hex("not hex"), None);
        assert_eq!(from_hex(&"a".repeat(63)), None);
        assert_eq!(from_hex(&"z".repeat(64)), None);
    }

    #[test]
    fn a_hash_field_round_trips_through_json() {
        let hash: Blake3Hash = [0x5a; 32];
        let text = serde_json::to_string(&Hashed { hash }).unwrap();

        assert_eq!(text, format!("{{\"hash\":\"{}\"}}", to_hex(&hash)));
        assert_eq!(
            serde_json::from_str::<Hashed>(&text).unwrap(),
            Hashed { hash }
        );
        assert!(serde_json::from_str::<Hashed>("{\"hash\":\"short\"}").is_err());
    }

    #[test]
    fn an_optional_hash_field_round_trips_through_json() {
        let hash: Blake3Hash = [0x6b; 32];

        let text = serde_json::to_string(&MaybeHashed { hash: Some(hash) }).unwrap();
        assert_eq!(
            serde_json::from_str::<MaybeHashed>(&text).unwrap(),
            MaybeHashed { hash: Some(hash) }
        );

        let text = serde_json::to_string(&MaybeHashed { hash: None }).unwrap();
        assert_eq!(text, "{\"hash\":null}");
        assert_eq!(
            serde_json::from_str::<MaybeHashed>(&text).unwrap(),
            MaybeHashed { hash: None }
        );

        assert!(serde_json::from_str::<MaybeHashed>("{\"hash\":\"short\"}").is_err());
    }
}

use std::fmt;
use std::str::FromStr;

use serde::{Serialize, Serializer};

use crate::Error;

/// The width, in bytes, of a content hash.
pub const BLAKE3_HASH_LEN: usize = 32;

/// The content hash itself: the digest `Blake3` produces.
pub type Blake3Hash = [u8; BLAKE3_HASH_LEN];

/// The content address a stored object is named by.
///
/// A key is the `Blake3` hash of the object's **original content** — what a caller handed over, not
/// what the backend laid down. Which means a backend may encode, compress, or pack an object however
/// it likes without any key changing with it, and a read can always tell whether what came back is
/// what was asked for.
///
/// `Blake3` is the hash, and the key says so by being nothing else: a store reads and writes these
/// and no other. Keeping a name for the algorithm beside the digest would be a promise to read
/// another one, and there is nothing here that can — so a second algorithm would be a second kind of
/// store rather than a flag on this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    /// The digest itself.
    digest: Blake3Hash,
}

impl Key {
    /// A key from the digest it was derived from.
    #[must_use]
    pub const fn new(digest: Blake3Hash) -> Self {
        Self { digest }
    }

    /// The digest itself.
    #[must_use]
    pub const fn digest(&self) -> &Blake3Hash {
        &self.digest
    }

    /// The digest written out in lowercase hex.
    ///
    /// This is how a key names where its object sits, and it is the half of
    /// [`Display`](fmt::Display) that comes after `blake3:`.
    #[must_use]
    pub fn hex(&self) -> String {
        let mut hex = String::with_capacity(BLAKE3_HASH_LEN * 2);

        for byte in self.digest {
            hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
            hex.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }

        hex
    }
}

/// The digits a digest is written out with.
const DIGITS: &[u8; 16] = b"0123456789abcdef";

impl fmt::Display for Key {
    /// Writes the key as the hash and the digest in hex, `blake3:…`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "blake3:{}", self.hex())
    }
}

impl Serialize for Key {
    /// Writes the key the way it is written down anywhere else: as [`Display`](fmt::Display) writes
    /// it, `blake3:<hex>`.
    ///
    /// A key that crossed into structured output as its digest would be thirty-two numbers that
    /// say nothing on their own, and a reader would have to know the width and the order to put them
    /// back together. Written as a string it is the same key an object sits under, so what comes out
    /// of a listing is what goes back in — [`FromStr`] reads exactly this back.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl FromStr for Key {
    type Err = Error;

    /// Reads a key written as [`Display`](fmt::Display) writes it — `blake3:<hex>` — or as the
    /// digest alone, which is how one is written when the hash is not in doubt.
    ///
    /// `manifest:` is read in front of the digest as well as `blake3:`, since a key is the same
    /// key however the content under it happens to be kept; a listing that says which of the two a
    /// key is closes the loop by being readable back in.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if `text` is not a digest of the width this store writes, in
    /// hex — with or without the name of the hash or the word `manifest` in front of it.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let hex = text
            .strip_prefix("blake3:")
            .or_else(|| text.strip_prefix("manifest:"))
            .unwrap_or(text);
        if hex.len() != BLAKE3_HASH_LEN * 2 {
            return Err(Error::Malformed);
        }

        let mut digest = [0_u8; BLAKE3_HASH_LEN];
        for (at, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let pair = std::str::from_utf8(pair).map_err(|_| Error::Malformed)?;
            digest[at] = u8::from_str_radix(pair, 16).map_err(|_| Error::Malformed)?;
        }

        Ok(Self::new(digest))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::Key;

    #[test]
    fn a_key_reads_with_or_without_a_name_in_front() {
        let hex = "0f".repeat(32);
        let expected = Key::new([0x0f; 32]);

        // The digest on its own, and the two names a listing may put in front of it: a key is the
        // same key however the content under it is kept.
        assert_eq!(Key::from_str(&hex).unwrap(), expected);
        assert_eq!(Key::from_str(&format!("blake3:{hex}")).unwrap(), expected);
        assert_eq!(Key::from_str(&format!("manifest:{hex}")).unwrap(), expected);

        assert!(Key::from_str("not a hash").is_err());
        assert!(Key::from_str(&format!("manifest:{}", "0f".repeat(31))).is_err());
    }
}

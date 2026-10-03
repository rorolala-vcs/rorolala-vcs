use std::fmt;
use std::str::FromStr;

use serde::{Serialize, Serializer};

use crate::Error;

/// The width, in bytes, of a content hash.
pub const BLAKE3_HASH_LEN: usize = 32;

/// The fewest hex digits a hash may be named by.
///
/// Four is short enough to type and long enough that a head names one object in anything a person
/// keeps: two digests collide on four digits once in sixty-five thousand, so a listing would have
/// to hold that many before a short name became a nuisance.
pub const SHORT_HASH_MIN: usize = 4;

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

/// The head of a digest, which names an object when no other digest shares it.
///
/// A whole digest is [`Key`]'s to read; this is only the short form, so it is strictly shorter
/// than what [`Key`] takes. The digits alone cannot say what they name — a head names an object
/// only beside the keys that are there — so a caller settles it with [`resolve`](Self::resolve)
/// against the keys it can list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortHash {
    /// The digits, lowercase, so that comparing against a written-out digest is a byte compare.
    digits: String,
}

/// What a short hash named among the keys it was put to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// Exactly one key starts with the digits.
    ExactlyOne(Key),
    /// No key starts with them.
    NoMatch,
    /// More than one does, in the order the keys were given.
    Ambiguous(Vec<Key>),
}

impl ShortHash {
    /// Reads `text` as the head of a digest: hex, [`SHORT_HASH_MIN`] digits or more and fewer
    /// than a whole digest, with or without the name of the hash in front of it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] when `text` is not hex of a length a head may have.
    pub fn new(text: &str) -> Result<Self, Error> {
        let hex = text
            .strip_prefix("blake3:")
            .or_else(|| text.strip_prefix("manifest:"))
            .unwrap_or(text);
        if hex.len() < SHORT_HASH_MIN || hex.len() >= BLAKE3_HASH_LEN * 2 {
            return Err(Error::Malformed);
        }
        if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Error::Malformed);
        }

        Ok(Self {
            digits: hex.to_ascii_lowercase(),
        })
    }

    /// The digits, lowercase.
    #[must_use]
    pub fn digits(&self) -> &str {
        &self.digits
    }

    /// Whether `key`'s digest starts with these digits.
    #[must_use]
    pub fn matches(&self, key: &Key) -> bool {
        let digest = key.digest();

        self.digits.bytes().enumerate().all(|(at, digit)| {
            let byte = digest[at / 2];
            let nibble = if at % 2 == 0 { byte >> 4 } else { byte & 0x0f };

            DIGITS[usize::from(nibble)] == digit
        })
    }

    /// The key `candidates` names with this head, or none, or every one it names.
    ///
    /// Every match is kept rather than the first: a head that names two objects is a question the
    /// caller has to ask again with more digits, and which two they are is what lets it.
    #[must_use]
    pub fn resolve<'a>(&self, candidates: impl IntoIterator<Item = &'a Key>) -> Resolved {
        let named: Vec<Key> = candidates
            .into_iter()
            .filter(|key| self.matches(key))
            .copied()
            .collect();

        match named.len() {
            0 => Resolved::NoMatch,
            1 => Resolved::ExactlyOne(named[0]),
            _ => Resolved::Ambiguous(named),
        }
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

    use super::{Key, Resolved, SHORT_HASH_MIN, ShortHash};

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

    #[test]
    fn a_head_reads_as_hex_of_a_length_a_head_may_have() {
        // The two names a listing puts in front of a digest read in front of a head too, and an
        // uppercase head is the same head: the digits are kept lowercase for comparing against
        // what a digest is written out with.
        assert_eq!(ShortHash::new("blake3:0F1e").unwrap().digits(), "0f1e");
        assert_eq!(ShortHash::new("manifest:0f1e").unwrap().digits(), "0f1e");
        assert_eq!(ShortHash::new("0f1e").unwrap().digits(), "0f1e");

        // Four is the fewest, and a whole digest is not a head: [`Key`] takes that.
        assert!(ShortHash::new(&"0".repeat(SHORT_HASH_MIN - 1)).is_err());
        assert!(ShortHash::new(&"0f".repeat(32)).is_err());
        assert!(ShortHash::new("0f1g").is_err());
        assert!(ShortHash::new("0f1").is_err());
    }

    #[test]
    fn a_head_names_what_starts_with_it() {
        let one = Key::new([0x0f; 32]);
        let other = Key::new([0xf0; 32]);

        assert!(ShortHash::new("0f").is_err());
        assert!(ShortHash::new("0f0f").unwrap().matches(&one));
        assert!(!ShortHash::new("0f0f").unwrap().matches(&other));

        // A head of an odd number of digits still lines up with the digest's own nibbles.
        assert!(ShortHash::new("0f0f0").unwrap().matches(&one));
        assert!(!ShortHash::new("0f0f0").unwrap().matches(&other));
    }

    #[test]
    fn a_head_resolves_to_none_one_or_several() {
        let one = Key::new([0x0f; 32]);
        let mut digest = [0x0f; 32];
        digest[31] = 0x1f;
        let two = Key::new(digest);

        // `one` and `two` share their first sixty-two digits, so a head of any length up to that
        // names both of them.
        match ShortHash::new("0f0f0f0f0f0f0f0f0f")
            .unwrap()
            .resolve([&one, &two])
        {
            Resolved::Ambiguous(named) => assert_eq!(named, vec![one, two]),
            other => panic!("a head two keys share is ambiguous, not {other:?}"),
        }

        assert_eq!(
            ShortHash::new("0f0f").unwrap().resolve([&one]),
            Resolved::ExactlyOne(one)
        );
        assert_eq!(
            ShortHash::new("f0f0").unwrap().resolve([&one]),
            Resolved::NoMatch
        );
    }
}

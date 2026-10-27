//! The hash a material's content is stored under.

use std::fmt;

use crate::error::{FormatError, FormatErrorKind};

/// The prefix every content hash a lock writes carries.
pub const BLAKE3_PREFIX: &str = "blake3:";

/// How many bytes a blake3 digest is.
const DIGEST_LEN: usize = 32;

/// How many hexadecimal digits a blake3 digest is written with.
const HEX_LEN: usize = DIGEST_LEN * 2;

/// The hexadecimal digits lowercase, which is the spelling a lock writes.
const HEX: [u8; 16] = *b"0123456789abcdef";

/// The content a material is placed as, named by its blake3 hash.
///
/// A lock writes it as `blake3:` and 64 lowercase hexadecimal digits; no other algorithm is read,
/// since a hash this build cannot meet content with is not one it can place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash {
    /// The digest itself.
    digest: [u8; DIGEST_LEN],
}

impl ContentHash {
    /// The hash of `digest`.
    #[must_use]
    pub const fn from_digest(digest: [u8; DIGEST_LEN]) -> Self {
        Self { digest }
    }

    /// Reads `token` as a lock writes one: `blake3:` and 64 lowercase hexadecimal digits.
    ///
    /// # Errors
    ///
    /// Returns [`FormatErrorKind::MalformedHash`] when `token` carries another algorithm prefix,
    /// another length, or anything but lowercase hexadecimal digits.
    pub fn from_token(token: &str) -> Result<Self, FormatError> {
        let refused = || {
            FormatError::plain(FormatErrorKind::MalformedHash {
                text: token.to_owned(),
            })
        };

        let Some(hex) = token.strip_prefix(BLAKE3_PREFIX) else {
            return Err(refused());
        };

        if hex.len() != HEX_LEN || !hex.bytes().all(|byte| digit(byte).is_some()) {
            return Err(refused());
        }

        let mut digest = [0_u8; DIGEST_LEN];
        let bytes = hex.as_bytes();
        for at in 0..DIGEST_LEN {
            let Some(high) = digit(bytes[at * 2]) else {
                return Err(refused());
            };
            let Some(low) = digit(bytes[at * 2 + 1]) else {
                return Err(refused());
            };

            digest[at] = (high << 4) | low;
        }

        Ok(Self { digest })
    }

    /// The token a lock writes this hash as.
    #[must_use]
    pub fn to_token(&self) -> String {
        let mut token = String::with_capacity(BLAKE3_PREFIX.len() + HEX_LEN);
        token.push_str(BLAKE3_PREFIX);

        for byte in self.digest {
            token.push(char::from(HEX[usize::from(byte >> 4)]));
            token.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }

        token
    }

    /// The digest itself.
    #[must_use]
    pub const fn digest(&self) -> &[u8; DIGEST_LEN] {
        &self.digest
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_token())
    }
}

/// The value of a hexadecimal digit, or nothing when `byte` is not one.
const fn digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentHash, HEX_LEN};
    use crate::error::FormatErrorKind;

    #[test]
    fn a_token_round_trips_through_the_text_a_lock_writes() {
        let hash = ContentHash::from_digest([7; 32]);

        assert_eq!(hash.to_token(), format!("blake3:{}", "07".repeat(32)));
        assert_eq!(
            ContentHash::from_token(&hash.to_token()).unwrap(),
            hash,
            "the token a lock writes is the token it reads"
        );
    }

    #[test]
    fn a_digest_is_written_with_the_full_length() {
        let token = ContentHash::from_digest([0; 32]).to_token();

        assert_eq!(token.len(), "blake3:".len() + HEX_LEN);
    }

    #[test]
    fn only_a_lowercase_blake3_token_is_a_content_hash() {
        let digest = "ab".repeat(32);

        // The right token is read.
        assert!(ContentHash::from_token(&format!("blake3:{digest}")).is_ok());

        for wrong in [
            digest.clone(),                                 // no algorithm prefix
            format!("blake2:{digest}"),                     // another algorithm
            format!("blake3:{}", digest.to_uppercase()),    // uppercase
            format!("blake3:{}", &digest[..HEX_LEN - 2]),   // short
            format!("blake3:{digest}00"),                   // long
            format!("blake3:{}zz", &digest[..HEX_LEN - 2]), // not hexadecimal
            "blake3:".to_owned(),                           // nothing at all
        ] {
            let error = ContentHash::from_token(&wrong).unwrap_err();
            assert!(
                matches!(error.kind(), FormatErrorKind::MalformedHash { .. }),
                "{wrong}"
            );
        }
    }
}

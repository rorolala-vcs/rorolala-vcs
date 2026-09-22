use std::io;

use crate::Error;

/// The level `lz4` is asked for: the fastest there is.
///
/// It is the codec a write picks when it wants speed more than size, so the level that spends the
/// least time for the most of what `lz4` can find is the one that matches what it is for.
const LZ4_LEVEL: i32 = 1;

/// The level `zstd` is asked for: its own balance of speed and size.
///
/// It is the codec a write picks when it wants size more than speed, and it can be turned up when
/// a store is willing to spend more time for less of the content.
const ZSTD_LEVEL: i32 = 3;

/// The encodings a payload may be written in.
///
/// The number a codec is written down by travels in a frame header, so a reader can undo what a
/// writer did without being told anything else.
///
/// A codec is lossless by construction: what [`decode`](Self::decode) gives back is what
/// [`encode`](Self::encode) was handed, byte for byte. Storage promises nothing less, and it is
/// the codec's whole job to keep that promise — which is also why a payload whose encoding
/// cannot be undone is a bug, never a state a store is allowed to be in.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Codec {
    /// The payload is stored as it is.
    #[default]
    Raw,
    /// The payload is stored as an `lz4` block.
    Lz4,
    /// The payload is stored as a `zstd` frame.
    Zstd,
}

impl Codec {
    /// The number this codec is written down by in a frame header.
    #[must_use]
    pub const fn id(self) -> u8 {
        match self {
            Self::Raw => 0,
            Self::Lz4 => 1,
            Self::Zstd => 2,
        }
    }

    /// The codec written down by `number`, if this build knows one.
    #[must_use]
    pub const fn from_id(number: u8) -> Option<Self> {
        match number {
            0 => Some(Self::Raw),
            1 => Some(Self::Lz4),
            2 => Some(Self::Zstd),
            _ => None,
        }
    }

    /// Encodes `plain` for storage.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the codec itself fails.
    pub fn encode(self, plain: &[u8]) -> Result<Vec<u8>, Error> {
        match self {
            Self::Raw => Ok(plain.to_vec()),
            Self::Lz4 => lz4::block::compress(
                plain,
                Some(lz4::block::CompressionMode::FAST(LZ4_LEVEL)),
                // The frame already says how long the content was, so an `lz4` block does not have
                // to carry a length of its own.
                false,
            )
            .map_err(|said| Error::Io(io::Error::other(said.to_string()))),
            Self::Zstd => zstd::bulk::compress(plain, ZSTD_LEVEL)
                .map_err(|said| Error::Io(io::Error::other(said.to_string()))),
        }
    }

    /// Decodes `encoded` back to the `plain_len` bytes it was encoded from.
    ///
    /// What the frame said the content was long is passed down rather than guessed at: `lz4`
    /// blocks and `zstd` frames do not carry the length themselves, and knowing it is what lets
    /// the content be laid out in one allocation instead of grown as it is read.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] when `encoded` does not decode to `plain_len` bytes — the
    /// frame said how long the content was, so anything else is a store that does not hold
    /// together rather than a value to be guessed at.
    pub fn decode(self, encoded: &[u8], plain_len: usize) -> Result<Vec<u8>, Error> {
        let content = match self {
            Self::Raw if encoded.len() == plain_len => return Ok(encoded.to_vec()),
            Self::Raw => return Err(Error::Malformed),
            Self::Lz4 => {
                // An `lz4` block holds a length an `i32` can name, so a longer content is not one
                // an `lz4` block can be holding in the first place.
                let size = i32::try_from(plain_len).map_err(|_| Error::Malformed)?;

                lz4::block::decompress(encoded, Some(size)).map_err(|_| Error::Malformed)?
            }
            Self::Zstd => {
                zstd::bulk::decompress(encoded, plain_len).map_err(|_| Error::Malformed)?
            }
        };

        // The frame said how long the content was, so a codec that gives back another length is
        // not one to hand anything on.
        if content.len() == plain_len {
            Ok(content)
        } else {
            Err(Error::Malformed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Codec;
    use crate::Error;

    /// Content with repetition in it, since compressing what cannot be compressed says nothing.
    fn content() -> Vec<u8> {
        "the same words over and over, ".repeat(64).into_bytes()
    }

    #[test]
    fn every_codec_gives_back_what_it_was_given() {
        let content = content();

        for codec in [Codec::Raw, Codec::Lz4, Codec::Zstd] {
            let encoded = codec.encode(&content).unwrap();
            let decoded = codec.decode(&encoded, content.len()).unwrap();

            assert_eq!(decoded, content, "{codec:?}");
        }
    }

    #[test]
    fn an_empty_content_goes_through_every_codec() {
        for codec in [Codec::Raw, Codec::Lz4, Codec::Zstd] {
            let encoded = codec.encode(b"").unwrap();

            assert_eq!(codec.decode(&encoded, 0).unwrap(), b"", "{codec:?}");
        }
    }

    #[test]
    fn compressing_makes_repetition_smaller() {
        let content = content();

        for codec in [Codec::Lz4, Codec::Zstd] {
            let encoded = codec.encode(&content).unwrap();

            assert!(
                encoded.len() < content.len(),
                "{codec:?} kept {} of {}",
                encoded.len(),
                content.len()
            );
        }
    }

    #[test]
    fn a_number_and_a_codec_agree() {
        for codec in [Codec::Raw, Codec::Lz4, Codec::Zstd] {
            assert_eq!(Codec::from_id(codec.id()), Some(codec));
        }

        // A number no codec is written down by is not guessed at.
        assert_eq!(Codec::from_id(200), None);
    }

    #[test]
    fn what_will_not_decode_is_not_handed_on() {
        let content = content();
        let encoded = Codec::Lz4.encode(&content).unwrap();

        // The frame said how long the content was, so a codec that gives back another length
        // gives back nothing.
        assert!(matches!(
            Codec::Lz4.decode(&encoded, content.len() - 1),
            Err(Error::Malformed)
        ));
        assert!(matches!(
            Codec::Raw.decode(&encoded, content.len() + 1),
            Err(Error::Malformed)
        ));

        // And bytes that are not an encoded payload at all are not guessed at either.
        assert!(matches!(
            Codec::Lz4.decode(b"not an lz4 block", content.len()),
            Err(Error::Malformed)
        ));
        assert!(matches!(
            Codec::Zstd.decode(b"not a zstd frame", content.len()),
            Err(Error::Malformed)
        ));
    }
}

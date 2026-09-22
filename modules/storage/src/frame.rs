use crate::{Codec, Error};

/// The magic every frame header starts with.
pub const FRAME_MAGIC: [u8; 4] = *b"RLST";

/// The version of the frame format this build writes.
pub const FRAME_VERSION: u8 = 1;

/// How a payload is laid out behind its frame.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// The payload is the whole content, encoded.
    #[default]
    Single,
    /// The payload is a [`Manifest`](crate::Manifest), encoded.
    Chunked,
}

impl Layout {
    /// The number this layout is written down by in a frame header.
    #[must_use]
    pub const fn id(self) -> u8 {
        match self {
            Self::Single => 0,
            Self::Chunked => 1,
        }
    }

    /// The layout written down by `number`, if this build knows one.
    #[must_use]
    pub const fn from_id(number: u8) -> Option<Self> {
        match number {
            0 => Some(Self::Single),
            1 => Some(Self::Chunked),
            _ => None,
        }
    }
}

/// What a stored entry says about how it was stored.
///
/// The frame is what makes storage able to answer "how was this written in?" without having to
/// guess: it says how the payload is encoded and how it is laid out, and it is stored **as it
/// is**, never encoded itself — so a reader learns how to read the payload without decoding
/// anything first.
///
/// What it does *not* say is what the content means. A frame describes the encoding, not the
/// object, which is what keeps storage from having to know the difference between a file and
/// anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// The version of the frame format the entry was written in.
    pub version: u8,
    /// The codec the payload is encoded with.
    pub codec: Codec,
    /// How the payload is laid out.
    pub layout: Layout,
    /// The length, in bytes, of the content before it was encoded.
    ///
    /// For a [`Single`](Layout::Single) entry this is the object's own length; for a
    /// [`Chunked`](Layout::Chunked) one it is the length of the whole content the manifest adds
    /// up to.
    pub plain_len: u64,
}

impl Frame {
    /// Writes this frame as the header that goes in front of a payload.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut header = Vec::with_capacity(HEADER_LEN);

        header.extend_from_slice(&FRAME_MAGIC);
        header.push(self.version);
        header.push(self.codec.id());
        header.push(self.layout.id());
        header.extend_from_slice(&self.plain_len.to_be_bytes());

        header
    }

    /// Reads the header `bytes` starts with, and how many bytes of them it took.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Malformed`] if `bytes` does not start with a header this build
    /// understands — the wrong magic, a version or a codec it does not know, or too few bytes to
    /// hold one.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), Error> {
        let header = bytes.get(..HEADER_LEN).ok_or(Error::Malformed)?;

        if header.get(..FRAME_MAGIC.len()) != Some(FRAME_MAGIC.as_slice()) {
            return Err(Error::Malformed);
        }

        // A version this build does not write is a header it cannot say it understands: the
        // bytes after it may mean anything.
        let version = header[4];
        if version != FRAME_VERSION {
            return Err(Error::Malformed);
        }

        let codec = Codec::from_id(header[5]).ok_or(Error::Malformed)?;
        let layout = Layout::from_id(header[6]).ok_or(Error::Malformed)?;
        let plain_len = read_u64(&header[7..]).ok_or(Error::Malformed)?;

        Ok((
            Self {
                version,
                codec,
                layout,
                plain_len,
            },
            HEADER_LEN,
        ))
    }
}

/// How long the header every entry starts with is.
const HEADER_LEN: usize = FRAME_MAGIC.len() + 3 + size_of::<u64>();

/// Reads the first eight bytes of `bytes` as a big-endian number.
fn read_u64(bytes: &[u8]) -> Option<u64> {
    let bytes: [u8; 8] = bytes.get(..8)?.try_into().ok()?;

    Some(u64::from_be_bytes(bytes))
}

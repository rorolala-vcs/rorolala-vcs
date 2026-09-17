use std::fmt;
use std::io;

/// What can go wrong while reading an identity, proving one, or running a session.
///
/// The channel is a Rust-side type — nothing here is exported over FFI, which only wraps
/// plain request-and-result calls — so the variants are free to say exactly what failed.
#[derive(Debug)]
pub enum Error {
    /// A key or signature was not the shape its algorithm states.
    Malformed,
    /// A signature did not verify under the key it was checked against.
    BadSignature,
    /// The peer proved an identity other than the one that was expected.
    UnexpectedIdentity,
    /// A handshake did not follow the protocol: a message was out of order, named a suite
    /// this side does not offer, or ended early.
    Handshake,
    /// A record did not decrypt: it was altered, reordered, or never a record at all.
    BadRecord,
    /// The underlying stream failed.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("a key or signature was malformed"),
            Self::BadSignature => formatter.write_str("a signature did not verify"),
            Self::UnexpectedIdentity => {
                formatter.write_str("the peer proved an identity that was not expected")
            }
            Self::Handshake => formatter.write_str("the handshake did not follow the protocol"),
            Self::BadRecord => formatter.write_str("a record did not decrypt"),
            Self::Io(source) => write!(formatter, "the underlying stream failed: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

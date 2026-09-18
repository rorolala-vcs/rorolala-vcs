use std::fmt;
use std::io;

use rorolala_errors::IoError;
use rorolala_utils_lazyffi::lazyffi;

/// What can go wrong while reading an identity, proving one, or running a session.
///
/// The channel is a Rust-side type, but the errors it raises are not: they are what a
/// caller outside Rust reads when a session could not be established, so the enum
/// crosses the C ABI as [`RolaAuthError`]. The underlying stream's [`io::Error`] cannot
/// cross — it is foreign and has no repr-C sibling — so what crosses instead is the
/// stand-in [`IoError`] from `rorolala-errors`, which carries what it said.
#[derive(Debug)]
#[lazyffi(export = RolaAuthError)]
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
    Io(IoError),
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
        Self::Io(source.into())
    }
}

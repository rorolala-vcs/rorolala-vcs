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

#[cfg(test)]
mod tests {
    use std::error::Error as _;
    use std::io;

    use super::Error;

    #[test]
    fn each_error_says_what_went_wrong() {
        assert_eq!(
            Error::Malformed.to_string(),
            "a key or signature was malformed"
        );
        assert_eq!(
            Error::BadSignature.to_string(),
            "a signature did not verify"
        );
        assert_eq!(
            Error::UnexpectedIdentity.to_string(),
            "the peer proved an identity that was not expected"
        );
        assert_eq!(
            Error::Handshake.to_string(),
            "the handshake did not follow the protocol"
        );
        assert_eq!(Error::BadRecord.to_string(), "a record did not decrypt");
    }

    #[test]
    fn an_io_error_keeps_what_the_stream_said_and_carries_a_source() {
        let error: Error = io::Error::new(io::ErrorKind::ConnectionReset, "reset by peer").into();

        // The underlying stream's words survive the crossing, and it is the only variant
        // with a source to hand to a caller walking the chain.
        assert!(error.to_string().contains("reset by peer"));
        assert!(error.source().is_some());
        assert!(Error::Handshake.source().is_none());
    }
}

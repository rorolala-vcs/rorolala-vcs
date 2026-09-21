use std::fmt;
use std::io;

use rorolala_errors::{AddrError, BincodeError, IoError, JsonError};
use rorolala_utils_lazyffi::lazyffi;

/// Everything that can go wrong while an action runs.
///
/// The two sides of an action exchange values over one channel, so a context
/// with no channel, a value that will not fit a frame, and a channel that fails
/// are all the same kind of thing to a caller: the exchange did not happen. Every
/// protocol-level failure is one of these.
///
/// The I/O and codec failures carry the stand-in types from `rorolala-errors`: the
/// errors they come from are foreign, so they cannot cross the C ABI, and what a caller
/// gets instead is what those errors said.
#[derive(Debug)]
#[lazyffi]
pub enum ActionError {
    /// The action context has no channel, so nothing can be exchanged with the peer.
    NoChannel,
    /// The side that owns a value held none to send, which its side should never do.
    MissingValue,
    /// A value was longer than a frame can carry.
    ValueTooLarge,
    /// The channel failed.
    Io(IoError),
    /// A value could not be encoded, or a frame could not be decoded.
    Codec(BincodeError),
    /// No action answers to the id asked for.
    UnknownAction(u32),
    /// What an action produced could not be turned into JSON.
    Json(JsonError),
    /// The target a caller named would not read as an address.
    Addr(AddrError),
    /// The session could not be established: a key could not be read, the peer could not
    /// be reached, or the identity it proved was not the one expected.
    Auth(rorolala_auth::Error),
}

impl fmt::Display for ActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoChannel => formatter.write_str("the action context has no channel"),
            Self::MissingValue => formatter.write_str("the owning side held no value to send"),
            Self::ValueTooLarge => formatter.write_str("a value was too long to frame"),
            Self::Io(source) => write!(formatter, "the channel failed: {source}"),
            Self::Codec(source) => {
                write!(formatter, "a value could not cross the channel: {source}")
            }
            Self::UnknownAction(id) => write!(formatter, "no action answers to id {id}"),
            Self::Json(source) => {
                write!(
                    formatter,
                    "what the action produced would not encode: {source}"
                )
            }
            Self::Addr(source) => write!(formatter, "{source}"),
            Self::Auth(source) => {
                write!(formatter, "the session could not be established: {source}")
            }
        }
    }
}

impl std::error::Error for ActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Codec(source) => Some(source),
            Self::Json(source) => Some(source),
            Self::Addr(source) => Some(source),
            Self::Auth(source) => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for ActionError {
    fn from(source: io::Error) -> Self {
        Self::Io(source.into())
    }
}

impl From<bincode2::Error> for ActionError {
    fn from(source: bincode2::Error) -> Self {
        Self::Codec(source.into())
    }
}

impl From<rorolala_auth::Error> for ActionError {
    fn from(source: rorolala_auth::Error) -> Self {
        Self::Auth(source)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;
    use std::io;

    use super::ActionError;

    #[test]
    fn a_variant_with_no_cause_says_what_happened() {
        assert_eq!(
            ActionError::NoChannel.to_string(),
            "the action context has no channel"
        );
        assert_eq!(
            ActionError::MissingValue.to_string(),
            "the owning side held no value to send"
        );
        assert_eq!(
            ActionError::ValueTooLarge.to_string(),
            "a value was too long to frame"
        );
        assert_eq!(
            ActionError::UnknownAction(7).to_string(),
            "no action answers to id 7"
        );
    }

    #[test]
    fn a_variant_with_no_cause_has_no_source() {
        for error in [
            ActionError::NoChannel,
            ActionError::MissingValue,
            ActionError::ValueTooLarge,
            ActionError::UnknownAction(1),
        ] {
            assert!(error.source().is_none());
        }
    }

    #[test]
    fn an_io_failure_becomes_a_failed_channel() {
        let error = ActionError::from(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));

        assert!(matches!(error, ActionError::Io(_)));
        assert_eq!(error.to_string(), "the channel failed: denied");
        assert!(error.source().is_some());
    }

    #[test]
    fn a_codec_failure_becomes_a_value_that_would_not_cross() {
        let raw = bincode2::deserialize::<u32>(&[]).unwrap_err();
        let error = ActionError::from(raw);

        assert!(matches!(error, ActionError::Codec(_)));
        assert!(
            error
                .to_string()
                .starts_with("a value could not cross the channel:")
        );
        assert!(error.source().is_some());
    }

    #[test]
    fn an_auth_failure_becomes_a_session_that_would_not_establish() {
        let error = ActionError::from(rorolala_auth::Error::Malformed);

        assert!(matches!(error, ActionError::Auth(_)));
        assert_eq!(
            error.to_string(),
            "the session could not be established: a key or signature was malformed"
        );
        assert!(error.source().is_some());
    }
}

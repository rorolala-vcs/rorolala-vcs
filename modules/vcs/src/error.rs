use std::fmt;
use std::io;

use rorolala_errors::IoError;
use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;

/// The error of reading an index object as a kind it is not
///
/// Returned when a [`VCSIndexObject`](crate::VCSIndexObject) is turned into another kind
/// and the two do not match.
#[lazyffi(export = RolaParseVCSIndexObjectError)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseVCSIndexObjectError {
    /// A Variant was expected, but a Version was found
    ExpectVariant,
    /// A Version was expected, but a Variant was found
    ExpectVersion,
    /// A Creator was expected, but another kind of object was found
    ExpectCreator,
    /// A Message was expected, but another kind of object was found
    ExpectMessage,
}

impl core::fmt::Display for ParseVCSIndexObjectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ExpectVariant => {
                write!(f, "expected a variant object, found a version object")
            }
            Self::ExpectVersion => {
                write!(f, "expected a version object, found a variant object")
            }
            Self::ExpectCreator => {
                write!(f, "expected a creator object, found another kind")
            }
            Self::ExpectMessage => {
                write!(f, "expected a message object, found another kind")
            }
        }
    }
}

impl core::error::Error for ParseVCSIndexObjectError {}

/// What a Creator or a Message could not be made from
///
/// A Creator and a Message are short text — a name and a message — and what makes one is checked
/// once, when it is made: it may be at most 256 bytes, and it must be UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextError {
    /// The text is longer than 256 bytes.
    TooLong,
    /// The bytes are not valid UTF-8.
    NotUtf8,
}

impl core::fmt::Display for TextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooLong => write!(f, "the text is longer than 256 bytes"),
            Self::NotUtf8 => write!(f, "the text is not valid UTF-8"),
        }
    }
}

impl core::error::Error for TextError {}

impl rorolala_errors::Failure for TextError {
    fn name(&self) -> &'static str {
        match self {
            Self::TooLong => "text_too_long",
            Self::NotUtf8 => "text_not_utf8",
        }
    }

    fn reason(&self) -> String {
        self.to_string()
    }
}

rorolala_errors::failure!(TextError);

impl From<TextError> for VCSIndexError {
    fn from(_: TextError) -> Self {
        Self::Malformed
    }
}

/// What the index's own store failed with.
///
/// This is the error the store answers the two storage traits with — [`StorageBackend`] and,
/// through it, [`TransferableBackend`] — and what a read or a write is worked out from. It is
/// told in keys rather than in the text a caller reads, which is what the reading and writing
/// errors are for: this one is the store's own account of what happened.
///
/// [`StorageBackend`]: rorolala_storage::StorageBackend
/// [`TransferableBackend`]: rorolala_storage::TransferableBackend
#[derive(Debug)]
pub enum VCSIndexError {
    /// Nothing is stored under the key.
    NotFound(Key),
    /// What is stored does not read as an index object this build knows.
    Malformed,
    /// What came back does not hash to the key it was read under.
    Corrupt(Key),
    /// The index itself failed.
    Io(io::Error),
}

impl fmt::Display for VCSIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(key) => write!(formatter, "no index object is stored under {key}"),
            Self::Malformed => formatter.write_str("what is stored is not an index object"),
            Self::Corrupt(key) => {
                write!(formatter, "what is stored under {key} does not hash to it")
            }
            Self::Io(source) => write!(formatter, "the index failed: {source}"),
        }
    }
}

impl std::error::Error for VCSIndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for VCSIndexError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

/// What a read of the index failed with
///
/// A read names one object by the key it is stored under, so what can go wrong is that nothing
/// is there, that what is there does not read as an index object, that what came back is not
/// what the key names, or that the index itself could not be read.
#[lazyffi(export = RolaVCSIndexReadingError)]
#[derive(Debug)]
pub enum VCSIndexReadingError {
    /// Nothing is stored under the key.
    NotFound {
        /// The key nothing is stored under.
        key: String,
    },
    /// What is stored does not read as an index object this build knows.
    Malformed,
    /// What came back does not hash to the key it was read under.
    Corrupt {
        /// The key what came back does not hash to.
        key: String,
    },
    /// The index itself failed.
    Io(IoError),
}

impl rorolala_errors::Failure for VCSIndexReadingError {
    /// The name of the way the read failed, as a program reads it.
    fn name(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "vcs_index_reading_not_found",
            Self::Malformed => "vcs_index_reading_malformed",
            Self::Corrupt { .. } => "vcs_index_reading_corrupt",
            Self::Io(_) => "vcs_index_reading_io",
        }
    }

    /// What went wrong, in the library's own words, with what it went wrong about.
    fn reason(&self) -> String {
        match self {
            Self::NotFound { key } => format!("no index object is stored under {key}"),
            Self::Malformed => "what is stored is not an index object this build knows".to_owned(),
            Self::Corrupt { key } => {
                format!("what is stored under {key} does not hash to it")
            }
            Self::Io(source) => format!("the index failed: {source}"),
        }
    }
}

rorolala_errors::failure!(VCSIndexReadingError);

/// What a write to the index failed with
#[lazyffi(export = RolaVCSIndexWritingError)]
#[derive(Debug)]
pub enum VCSIndexWritingError {
    /// What was handed over does not encode as an index object.
    Malformed,
    /// The index itself failed.
    Io(IoError),
}

impl rorolala_errors::Failure for VCSIndexWritingError {
    /// The name of the way the write failed, as a program reads it.
    fn name(&self) -> &'static str {
        match self {
            Self::Malformed => "vcs_index_writing_malformed",
            Self::Io(_) => "vcs_index_writing_io",
        }
    }

    /// What went wrong, in the library's own words.
    fn reason(&self) -> String {
        match self {
            Self::Malformed => "what was handed over is not an index object".to_owned(),
            Self::Io(source) => format!("the index failed: {source}"),
        }
    }
}

rorolala_errors::failure!(VCSIndexWritingError);

impl From<VCSIndexError> for VCSIndexReadingError {
    fn from(error: VCSIndexError) -> Self {
        match error {
            VCSIndexError::NotFound(key) => Self::NotFound {
                key: key.to_string(),
            },
            VCSIndexError::Malformed => Self::Malformed,
            VCSIndexError::Corrupt(key) => Self::Corrupt {
                key: key.to_string(),
            },
            VCSIndexError::Io(source) => Self::Io(source.into()),
        }
    }
}

impl From<VCSIndexError> for VCSIndexWritingError {
    fn from(error: VCSIndexError) -> Self {
        match error {
            VCSIndexError::Io(source) => Self::Io(source.into()),
            _ => Self::Malformed,
        }
    }
}

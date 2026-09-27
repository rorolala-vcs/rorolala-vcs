//! What building the inverse index, and reading through it, failed with.

use std::fmt;

use rorolala_errors::{Failure, IoError};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_vcs::VCSIndexReadingError;

/// What building the inverse index failed with
///
/// Building reads every object the index holds, so what can go wrong is that the index itself does
/// not read, that what is written does not read back as a record this build knows, or that the file
/// it is kept in cannot be written.
#[lazyffi(export = RolaInverseIndexError)]
#[derive(Debug)]
pub enum InverseIndexError {
    /// The index it is built from could not be read.
    Read(VCSIndexReadingError),
    /// What is there does not read as an inverse index this build knows.
    Malformed,
    /// The inverse index could not be written.
    Io(IoError),
}

impl fmt::Display for InverseIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(source) => write!(
                formatter,
                "the index could not be read: {}",
                source.reason()
            ),
            Self::Malformed => formatter.write_str("what is there is not an inverse index"),
            Self::Io(source) => write!(formatter, "the inverse index failed: {source}"),
        }
    }
}

impl std::error::Error for InverseIndexError {}

impl From<std::io::Error> for InverseIndexError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source.into())
    }
}

impl From<VCSIndexReadingError> for InverseIndexError {
    fn from(source: VCSIndexReadingError) -> Self {
        Self::Read(source)
    }
}

impl Failure for InverseIndexError {
    fn name(&self) -> &'static str {
        match self {
            Self::Read(_) => "inverse_index_read",
            Self::Malformed => "inverse_index_malformed",
            Self::Io(_) => "inverse_index_io",
        }
    }

    fn reason(&self) -> String {
        self.to_string()
    }
}

rorolala_errors::failure!(InverseIndexError);

/// What a read through the inverse index failed with
///
/// A read asks for the dependents of one key, or for a version's number. It is answered from the
/// records when they describe the index, and by reading the objects when they do not, so what can
/// go wrong is that nothing is stored under the key, that what is stored is not the kind the
/// question is about, or that reading the index or the objects failed.
#[lazyffi(export = RolaInverseIndexReadingError)]
#[derive(Debug)]
pub enum InverseIndexReadingError {
    /// Nothing is stored under the key.
    NotFound {
        /// The key nothing is stored under.
        key: String,
    },
    /// What is stored is not the kind of object the question is about.
    Malformed,
    /// Reading the index failed.
    Read {
        /// Why it failed.
        cause: String,
    },
    /// The inverse index itself failed.
    Io(IoError),
}

impl fmt::Display for InverseIndexReadingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { key } => write!(formatter, "no index object is stored under {key}"),
            Self::Malformed => {
                formatter.write_str("what is stored is not the object the question is about")
            }
            Self::Read { cause } => write!(formatter, "the index could not be read: {cause}"),
            Self::Io(source) => write!(formatter, "the inverse index failed: {source}"),
        }
    }
}

impl std::error::Error for InverseIndexReadingError {}

impl From<std::io::Error> for InverseIndexReadingError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source.into())
    }
}

impl From<VCSIndexReadingError> for InverseIndexReadingError {
    fn from(source: VCSIndexReadingError) -> Self {
        Self::Read {
            cause: source.reason(),
        }
    }
}

impl Failure for InverseIndexReadingError {
    fn name(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "inverse_index_reading_not_found",
            Self::Malformed => "inverse_index_reading_malformed",
            Self::Read { .. } => "inverse_index_reading_read",
            Self::Io(_) => "inverse_index_reading_io",
        }
    }

    fn reason(&self) -> String {
        self.to_string()
    }
}

rorolala_errors::failure!(InverseIndexReadingError);

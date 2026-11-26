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

#[cfg(test)]
mod tests {
    use std::io;

    use rorolala_errors::Failure as _;
    use rorolala_storage::Key;
    use rorolala_vcs::VCSIndexError;

    use super::{InverseIndexError, InverseIndexReadingError};

    #[test]
    fn a_build_failure_says_what_went_wrong() {
        let read = InverseIndexError::from(rorolala_vcs::VCSIndexReadingError::from(
            VCSIndexError::NotFound(Key::new([1; 32])),
        ));
        assert!(read.to_string().contains("the index could not be read"));
        assert_eq!(read.name(), "inverse_index_read");

        assert_eq!(
            InverseIndexError::Malformed.to_string(),
            "what is there is not an inverse index"
        );
        assert_eq!(
            InverseIndexError::Malformed.name(),
            "inverse_index_malformed"
        );

        let io = InverseIndexError::from(io::Error::other("disk"));
        assert!(io.to_string().contains("disk"));
        assert_eq!(io.name(), "inverse_index_io");
        assert_eq!(
            InverseIndexError::Malformed.reason(),
            InverseIndexError::Malformed.to_string()
        );
    }

    #[test]
    fn a_reading_failure_says_what_went_wrong() {
        let missing = InverseIndexReadingError::NotFound {
            key: "key".to_owned(),
        };
        assert!(missing.to_string().contains("key"));
        assert_eq!(missing.name(), "inverse_index_reading_not_found");

        assert_eq!(
            InverseIndexReadingError::Malformed.name(),
            "inverse_index_reading_malformed"
        );

        let read = InverseIndexReadingError::Read {
            cause: "cause".to_owned(),
        };
        assert!(read.to_string().contains("cause"));
        assert_eq!(read.name(), "inverse_index_reading_read");

        let io = InverseIndexReadingError::from(io::Error::other("disk"));
        assert!(io.to_string().contains("disk"));
        assert_eq!(io.name(), "inverse_index_reading_io");
        assert_eq!(read.reason(), read.to_string());

        let carried = InverseIndexReadingError::from(rorolala_vcs::VCSIndexReadingError::from(
            VCSIndexError::Malformed,
        ));
        assert!(matches!(carried, InverseIndexReadingError::Read { .. }));
    }
}

//! What opening a book, or building one, fails with.

use std::fmt;
use std::io;

use rorolala_errors::{Failure, IoError};
use rorolala_utils_lazyffi::lazyffi;

/// What a book could not be, or could not be made.
///
/// A book is read from one file and nothing beside it, so what can go wrong is that the file
/// cannot be read, that it does not read as a book this build knows, or that it is a shape the
/// format itself refuses. Building reports the last two as well: a book that two articles of one
/// name would make ambiguous is not written at all.
#[lazyffi(export = RolaBookError)]
#[derive(Debug)]
pub enum BookError {
    /// The file could not be read, or the built book could not be written.
    Io(IoError),
    /// What is there does not read as a book this build knows.
    Malformed {
        /// What about it could not be read, in one sentence.
        detail: String,
    },
    /// The format version is one this build does not read.
    Unsupported {
        /// The version the file names.
        version: u32,
    },
    /// Two articles were given the same name, so neither could be found by it.
    Duplicate {
        /// The name that appeared more than once.
        name: String,
    },
    /// A book with no articles was asked to be written.
    Empty,
    /// The book is larger than the format's counts and offsets can name.
    Oversized {
        /// What could not be named.
        detail: String,
    },
}

impl fmt::Display for BookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(formatter, "the book's file failed: {source}"),
            Self::Malformed { detail } => write!(formatter, "the book could not be read: {detail}"),
            Self::Unsupported { version } => write!(
                formatter,
                "the book is written in format {version}, which this build does not know"
            ),
            Self::Duplicate { name } => {
                write!(
                    formatter,
                    "two articles are named `{name}`, so neither is found"
                )
            }
            Self::Empty => formatter.write_str("the book has no articles"),
            Self::Oversized { detail } => {
                write!(
                    formatter,
                    "the book is larger than its format allows: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for BookError {}

impl From<io::Error> for BookError {
    fn from(source: io::Error) -> Self {
        Self::Io(source.into())
    }
}

impl Failure for BookError {
    fn name(&self) -> &'static str {
        match self {
            Self::Io(_) => "book_io",
            Self::Malformed { .. } => "book_malformed",
            Self::Unsupported { .. } => "book_unsupported",
            Self::Duplicate { .. } => "book_duplicate",
            Self::Empty => "book_empty",
            Self::Oversized { .. } => "book_oversized",
        }
    }

    fn reason(&self) -> String {
        self.to_string()
    }
}

rorolala_errors::failure!(BookError);

#[cfg(test)]
mod tests {
    use std::io;

    use rorolala_errors::Failure as _;

    use super::BookError;

    #[test]
    fn a_book_failure_says_what_went_wrong() {
        assert!(
            BookError::from(io::Error::other("disk"))
                .to_string()
                .contains("disk")
        );
        assert_eq!(
            BookError::Malformed {
                detail: "bad".to_owned()
            }
            .to_string(),
            "the book could not be read: bad"
        );
        assert_eq!(
            BookError::Unsupported { version: 9 }.to_string(),
            "the book is written in format 9, which this build does not know"
        );
        assert_eq!(
            BookError::Duplicate {
                name: "a".to_owned()
            }
            .to_string(),
            "two articles are named `a`, so neither is found"
        );
        assert_eq!(BookError::Empty.to_string(), "the book has no articles");
        assert_eq!(
            BookError::Oversized {
                detail: "too big".to_owned()
            }
            .to_string(),
            "the book is larger than its format allows: too big"
        );
    }

    #[test]
    fn a_book_failure_names_itself_by_kind() {
        assert_eq!(BookError::from(io::Error::other("disk")).name(), "book_io");
        assert_eq!(
            BookError::Malformed {
                detail: String::new()
            }
            .name(),
            "book_malformed"
        );
        assert_eq!(
            BookError::Unsupported { version: 1 }.name(),
            "book_unsupported"
        );
        assert_eq!(
            BookError::Duplicate {
                name: String::new()
            }
            .name(),
            "book_duplicate"
        );
        assert_eq!(BookError::Empty.name(), "book_empty");
        assert_eq!(
            BookError::Oversized {
                detail: String::new()
            }
            .name(),
            "book_oversized"
        );
    }
}

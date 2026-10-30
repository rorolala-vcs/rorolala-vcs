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

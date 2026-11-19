use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::Key;

/// What a storage operation failed with.
///
/// Every backend is free to name its own failure, a backend that reaches another machine has
/// failures a local one does not, but this is what they all mean, so a caller that only wants
/// to tell one kind of failure from another has one thing to read.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// Nothing is stored under the key.
    NotFound(Key),
    /// What is stored under the key does not hash to it.
    Corrupt(Key),
    /// What is stored does not read as the format it says it is in: a frame that will not
    /// parse, an encoding that will not decode, a chunk list that will not read.
    Malformed,
    /// The store is there but not writable: a read-only mount, or permissions that do not allow
    /// a write. A set of stores reports this before writing rather than after, so a write aimed at
    /// a read-only copy is refused instead of being quietly moved to another one.
    ReadOnly {
        /// The store that cannot be written.
        path: PathBuf,
    },
    /// The storage itself failed.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(key) => write!(formatter, "no object is stored under {key}"),
            Self::Corrupt(key) => {
                write!(formatter, "what is stored under {key} does not hash to it")
            }
            Self::Malformed => formatter.write_str("what is stored does not read as its format"),
            Self::ReadOnly { path } => {
                write!(formatter, "`{}` is read-only", path.display())
            }
            Self::Io(source) => write!(formatter, "the storage failed: {source}"),
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

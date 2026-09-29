//! What a Layout fails with.

use std::fmt;
use std::io;

use rorolala_storage::Key;

/// What a Layout fails with.
///
/// There is no error for "the machine is not named": a layout is opened at a directory the caller
/// gives, so where it is kept is never a question this module has to answer.
#[derive(Debug)]
#[non_exhaustive]
pub enum LayoutError {
    /// The bytes would not be read or written.
    Io(io::Error),

    /// A path is not one a layout may name — empty, or climbing past the root.
    Path(String),

    /// A name is not one a layout may be given.
    Name(String),

    /// A create named something that is already there.
    AlreadyExists,

    /// A change named something that is not there.
    NotFound,

    /// Bytes on disk are not a record this build understands.
    Malformed,

    /// Content a packed layout needs that the store does not hold, so it could not be packed.
    MissingContent(Vec<Key>),

    /// The index a pack is built against failed to be read.
    Index(String),

    /// The store a pack is built against failed to be read.
    Storage(rorolala_storage::Error),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(formatter, "{source}"),
            Self::Path(path) => write!(formatter, "`{path}` is not a path a layout may name"),
            Self::Name(name) => write!(formatter, "`{name}` is not a name a layout may be given"),
            Self::AlreadyExists => formatter.write_str("it is already there"),
            Self::NotFound => formatter.write_str("there is nothing there"),
            Self::Malformed => {
                formatter.write_str("the layout's own bytes are not one this build wrote")
            }
            Self::MissingContent(keys) => {
                formatter.write_str("the store does not hold what the layout needs: ")?;
                for (at, key) in keys.iter().enumerate() {
                    if at > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "{key}")?;
                }
                Ok(())
            }
            Self::Index(said) => formatter.write_str(said),
            Self::Storage(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for LayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Path(_)
            | Self::Name(_)
            | Self::AlreadyExists
            | Self::NotFound
            | Self::Malformed
            | Self::MissingContent(_)
            | Self::Index(_) => None,
        }
    }
}

impl From<io::Error> for LayoutError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

impl From<rorolala_storage::Error> for LayoutError {
    fn from(source: rorolala_storage::Error) -> Self {
        Self::Storage(source)
    }
}

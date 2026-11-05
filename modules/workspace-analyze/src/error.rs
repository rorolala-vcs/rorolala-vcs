//! What a tree analysis fails with.

use std::fmt;
use std::io;

use rorolala_layout::LayoutError;

/// What a tree analysis fails with.
///
/// A file that cannot be read is not one of these: it is recorded in
/// [`TreeDiff::failed`](crate::TreeDiff), since one unreadable file is something to be told about
/// rather than a reason the whole reading has nothing to say.
#[derive(Debug)]
#[non_exhaustive]
pub enum TreeDiffError {
    /// The Layout could not be read.
    Layout(LayoutError),

    /// The tree could not be walked, or the cache could not be written.
    Io(io::Error),
}

impl fmt::Display for TreeDiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(source) => write!(formatter, "{source}"),
            Self::Io(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for TreeDiffError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(source) => Some(source),
            Self::Io(source) => Some(source),
        }
    }
}

impl From<LayoutError> for TreeDiffError {
    fn from(source: LayoutError) -> Self {
        Self::Layout(source)
    }
}

impl From<io::Error> for TreeDiffError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

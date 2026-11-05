//! What a tree operation failed with.

use std::fmt;
use std::path::PathBuf;

/// What reading or changing a tree failed with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    /// The bytes are not a structure this build wrote, or are not canonical.
    Malformed,

    /// A path a tree may not name, with the path as it was given.
    Path(String),

    /// A path named more than once.
    Duplicate(String),

    /// A path whose parent is a file or a symbolic link.
    Conflict(String),

    /// The path is not inside a tree.
    NotATree(PathBuf),

    /// The path is not inside the Workspace the tree belongs to.
    OutsideWorkspace(PathBuf),

    /// A file, a directory or a rule file could not be read or written.
    Io(String),

    /// The store a tree's content is kept in could not be read or written.
    Store(String),

    /// The version control index could not be read.
    Index(String),

    /// The Layout could not be read or changed.
    Layout(String),

    /// The tree's own staging area is not in the shape this build writes.
    Staging(String),
}

impl fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("the bytes are not a tree structure"),
            Self::Path(path) => write!(formatter, "{path} is not a path a tree may hold"),
            Self::Duplicate(path) => write!(formatter, "{path} is held twice"),
            Self::Conflict(path) => {
                write!(formatter, "{path} is a file or a link, so it holds nothing")
            }
            Self::NotATree(path) => write!(formatter, "{} is not inside a tree", path.display()),
            Self::OutsideWorkspace(path) => {
                write!(formatter, "{} is not inside the Workspace", path.display())
            }
            Self::Io(reason)
            | Self::Store(reason)
            | Self::Index(reason)
            | Self::Layout(reason)
            | Self::Staging(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for TreeError {}

impl From<std::io::Error> for TreeError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source.to_string())
    }
}

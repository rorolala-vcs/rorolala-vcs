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

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use super::TreeError;

    #[test]
    fn a_tree_failure_says_what_went_wrong() {
        assert_eq!(
            TreeError::Malformed.to_string(),
            "the bytes are not a tree structure"
        );
        assert_eq!(
            TreeError::Path("a".to_owned()).to_string(),
            "a is not a path a tree may hold"
        );
        assert_eq!(
            TreeError::Duplicate("a".to_owned()).to_string(),
            "a is held twice"
        );
        assert_eq!(
            TreeError::Conflict("a".to_owned()).to_string(),
            "a is a file or a link, so it holds nothing"
        );
        assert_eq!(
            TreeError::NotATree(PathBuf::from("a")).to_string(),
            "a is not inside a tree"
        );
        assert_eq!(
            TreeError::OutsideWorkspace(PathBuf::from("a")).to_string(),
            "a is not inside the Workspace"
        );
    }

    #[test]
    fn a_reason_is_written_as_it_was_given() {
        assert_eq!(TreeError::Io("io".to_owned()).to_string(), "io");
        assert_eq!(TreeError::Store("store".to_owned()).to_string(), "store");
        assert_eq!(TreeError::Index("index".to_owned()).to_string(), "index");
        assert_eq!(TreeError::Layout("layout".to_owned()).to_string(), "layout");
        assert_eq!(
            TreeError::Staging("staging".to_owned()).to_string(),
            "staging"
        );
    }

    #[test]
    fn an_io_failure_is_a_tree_failure() {
        let error = TreeError::from(io::Error::other("disk"));

        assert!(matches!(error, TreeError::Io(ref reason) if reason == "disk"));
    }
}

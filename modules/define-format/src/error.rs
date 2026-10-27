//! What a lock is refused for.

use std::fmt;

/// What is wrong with a lock.
///
/// A refusal met while reading carries the line it was met on; one met while a caller builds a
/// value does not, since what is wrong there is the value it handed over rather than a line of a
/// file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    /// The 1-based line the refusal was met on, when it was met on one.
    line: Option<usize>,
    /// What was refused.
    kind: FormatErrorKind,
}

impl FormatError {
    /// The 1-based line the refusal was met on, when it was met on one of a lock's lines.
    #[must_use]
    pub const fn line(&self) -> Option<usize> {
        self.line
    }

    /// What was refused, for a caller that branches on it.
    #[must_use]
    pub const fn kind(&self) -> &FormatErrorKind {
        &self.kind
    }

    /// This refusal, said as met on `line`.
    pub(crate) fn with_line(self, line: usize) -> Self {
        Self {
            line: Some(line),
            ..self
        }
    }

    /// This refusal, said as met on `line` of a file.
    pub(crate) const fn at(line: usize, kind: FormatErrorKind) -> Self {
        Self {
            line: Some(line),
            kind,
        }
    }

    /// This refusal, said by a caller building a value rather than reading a file.
    pub(crate) const fn plain(kind: FormatErrorKind) -> Self {
        Self { line: None, kind }
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "line {line}: {}", self.kind),
            None => fmt::Display::fmt(&self.kind, formatter),
        }
    }
}

impl std::error::Error for FormatError {}

/// What a lock is refused for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormatErrorKind {
    /// The lock names no format version, so nothing in it can be read with any confidence.
    MissingFormat,
    /// A line other than the format version comes before it, so the lock has no version to read.
    FormatNotFirst,
    /// The format version is named more than once.
    RepeatedFormat,
    /// The format version is not a number.
    MalformedFormat {
        /// What the line said instead of a version.
        text: String,
    },
    /// The format version is not one this build reads.
    UnsupportedFormat {
        /// The version the lock names.
        version: u32,
    },
    /// A line begins with a character no line of a lock begins with.
    UnknownLine {
        /// The character the line begins with.
        first: char,
    },
    /// A `?` line names a fact this build does not read.
    UnknownFact {
        /// The name the fact is given.
        name: String,
    },
    /// A `?` line does not carry the words the fact it names needs.
    MalformedFact {
        /// The name the fact is given.
        name: String,
    },
    /// A material line is not a path, a space and a content hash.
    MalformedMaterial,
    /// A content hash is not `blake3:` and 64 lowercase hexadecimal digits.
    MalformedHash {
        /// What the line said instead of a content hash.
        text: String,
    },
    /// A path is not one a lock may name.
    InvalidPath {
        /// The path as it was written.
        text: String,
    },
    /// A name is not one a lock may give a Vault.
    InvalidName {
        /// The name as it was written.
        name: String,
    },
    /// An address is not one a lock may write down.
    InvalidAddress {
        /// The address as it was written.
        address: String,
    },
    /// Two Vaults share one name.
    DuplicateVault {
        /// The name both are given.
        name: String,
    },
    /// Two materials share one path.
    DuplicatePath {
        /// The path both are placed at.
        path: String,
    },
}

impl fmt::Display for FormatErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFormat => formatter.write_str("the lock names no format version"),
            Self::FormatNotFirst => {
                formatter.write_str("the format version is not the lock's first value")
            }
            Self::RepeatedFormat => formatter.write_str("the lock names its format version twice"),
            Self::MalformedFormat { text } => {
                write!(formatter, "`{text}` is not a format version")
            }
            Self::UnsupportedFormat { version } => {
                write!(
                    formatter,
                    "{version} is not a format version this build reads"
                )
            }
            Self::UnknownLine { first } => {
                write!(formatter, "a line of a lock cannot begin with `{first}`")
            }
            Self::UnknownFact { name } => {
                write!(formatter, "`{name}` is not a fact this build reads")
            }
            Self::MalformedFact { name } => {
                write!(formatter, "`? {name}` does not carry the words it needs")
            }
            Self::MalformedMaterial => {
                formatter.write_str("a material line is a path, a space and a content hash")
            }
            Self::MalformedHash { text } => write!(
                formatter,
                "`{text}` is not `blake3:` and 64 lowercase hexadecimal digits"
            ),
            Self::InvalidPath { text } => {
                write!(formatter, "`{text}` is not a path a lock may name")
            }
            Self::InvalidName { name } => {
                write!(formatter, "`{name}` is not a name a lock may give a Vault")
            }
            Self::InvalidAddress { address } => {
                write!(
                    formatter,
                    "`{address}` is not an address a lock may write down"
                )
            }
            Self::DuplicateVault { name } => {
                write!(formatter, "more than one Vault is named `{name}`")
            }
            Self::DuplicatePath { path } => {
                write!(formatter, "more than one material is placed at `{path}`")
            }
        }
    }
}

//! What git support fails with.

use std::fmt;
use std::io;

use rola_define_format::FormatError;

/// What a git support run fails with.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(io::Error),
    /// A project's lock is not one this build reads.
    Format(FormatError),
    /// A project's `rola.toml` is not one this build reads.
    Config(toml::de::Error),
    /// The private area's state file is not one this build reads, or cannot be written.
    State(String),
    /// A project's `rola.toml` names something a run cannot use.
    ConfigValue(String),
    /// Git was asked something and answered with a failure.
    Git {
        /// The command, as it was written.
        command: String,
        /// What git said about the failure.
        reason: String,
    },
    /// The directory is inside no project.
    NoProject,
    /// The project is inside no git work tree.
    NoRepository,
    /// The project is inside a linked work tree, which git support does not act in.
    LinkedWorkTree,
    /// The project does not turn git support on.
    Disabled,
    /// A material's content is not the content the lock names.
    Content {
        /// Where the material is.
        path: String,
        /// The content the lock names.
        expected: String,
        /// The content found there.
        found: String,
    },
    /// A place a material is to be placed at already holds something else.
    Taken {
        /// The place.
        path: String,
    },
    /// No source holds the content a material names.
    Unavailable {
        /// The content no source holds.
        content: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(formatter, "{source}"),
            Self::Format(source) => write!(formatter, "the lock could not be read: {source}"),
            Self::Config(source) => write!(formatter, "`rola.toml` could not be read: {source}"),
            Self::State(said) => write!(formatter, "the state could not be read: {said}"),
            Self::ConfigValue(said) => formatter.write_str(said),
            Self::Git { command, reason } => write!(formatter, "`{command}` failed: {reason}"),
            Self::NoProject => formatter.write_str("the directory is inside no project"),
            Self::NoRepository => formatter.write_str("the project is inside no git work tree"),
            Self::LinkedWorkTree => formatter.write_str(
                "the project is inside a linked work tree, which git support does not act in; run \
                 the command in the repository's main work tree",
            ),
            Self::Disabled => formatter.write_str(
                "the project does not turn git support on (`features.git_support` is not true)",
            ),
            Self::Content {
                path,
                expected,
                found,
            } => write!(
                formatter,
                "`{path}` holds {found}, not the {expected} the lock names"
            ),
            Self::Taken { path } => write!(formatter, "`{path}` already holds something"),
            Self::Unavailable { content } => {
                write!(formatter, "no source holds the content {content}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Format(source) => Some(source),
            Self::Config(source) => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

impl From<FormatError> for Error {
    fn from(source: FormatError) -> Self {
        Self::Format(source)
    }
}

impl From<toml::de::Error> for Error {
    fn from(source: toml::de::Error) -> Self {
        Self::Config(source)
    }
}

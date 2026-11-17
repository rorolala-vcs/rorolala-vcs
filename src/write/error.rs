//! What a write failed with, as a type a command can translate and a C caller can branch on.

use std::fmt;

use rorolala_utils_lazyffi::lazyffi;

/// Why a write did not finish.
///
/// The variants are the categories a caller acts on rather than the words a command prints. A
/// command turns one into its own failure and exit code; the C ABI crosses it as
/// [`RolaWriteError`], whose tag is this enum's shape.
#[lazyffi(export = RolaWriteError)]
#[derive(Debug)]
pub enum WriteError {
    /// No Workspace holds the directory.
    NoWorkspace,
    /// The Workspace is working in no Layout.
    NoLayout,
    /// No Workspace or Vault holds a store.
    NoStore,
    /// No Workspace, Vault or directory holds an index.
    NoIndex,
    /// No account is bound, or the account named is not here.
    NoAccount,
    /// The Layout tracks no Vault.
    NoTrack,
    /// The copy of a Vault's Layout a name asked for was never fetched here.
    NotCached,
    /// The action needs the Vault and the run was told to stay offline.
    Offline,
    /// Another run holds the lock the write needs.
    Locked {
        /// The path whose lock is held.
        path: String,
    },
    /// A path is not one the write accepts.
    InvalidPath {
        /// The path that was refused.
        path: String,
        /// Why it was refused.
        reason: String,
    },
    /// The thing the write names is not there.
    NotFound {
        /// What is not there.
        what: String,
    },
    /// The write would break a rule it cannot break.
    Precondition {
        /// What the precondition is about.
        what: String,
        /// Why it does not hold.
        reason: String,
    },
    /// Ownership refuses the write.
    Ownership {
        /// The path ownership is about.
        path: String,
        /// What ownership says, as a short word.
        what: String,
        /// Why, when there is a longer reason.
        reason: String,
    },
    /// The caller asked the write to stop.
    Cancelled,
    /// An exchange with the Vault failed.
    Action {
        /// Why the exchange failed.
        reason: String,
    },
    /// A Layout could not be read or written.
    Layout {
        /// Why it failed.
        reason: String,
    },
    /// A store could not be read or written.
    Storage {
        /// Why it failed.
        reason: String,
    },
    /// An index could not be read or written.
    Index {
        /// Why it failed.
        reason: String,
    },
    /// A file or a directory failed.
    Io {
        /// The path that failed.
        path: String,
        /// Why it failed.
        reason: String,
    },
    /// A JSON document did not read as the shape a request or a plan has.
    Json {
        /// Why it did not.
        reason: String,
    },
    /// The write stopped for a reason this build does not name.
    Internal {
        /// Why it stopped.
        reason: String,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWorkspace => formatter.write_str("no Workspace holds the directory"),
            Self::NoLayout => formatter.write_str("the Workspace works in no Layout"),
            Self::NoStore => formatter.write_str("no Workspace or Vault holds a store"),
            Self::NoIndex => formatter.write_str("no index is there"),
            Self::NoAccount => formatter.write_str("no account is bound"),
            Self::NoTrack => formatter.write_str("the Layout tracks no Vault"),
            Self::NotCached => formatter.write_str("the Vault's Layout was never fetched here"),
            Self::Offline => formatter.write_str("the run is offline"),
            Self::Locked { path } => write!(formatter, "{path} is locked by another run"),
            Self::InvalidPath { path, reason } | Self::Io { path, reason } => {
                write!(formatter, "{path}: {reason}")
            }
            Self::NotFound { what } => write!(formatter, "{what} is not there"),
            Self::Precondition { what, reason } => write!(formatter, "{what}: {reason}"),
            Self::Ownership { path, what, reason } => {
                if reason.is_empty() {
                    write!(formatter, "{path}: ownership {what}")
                } else {
                    write!(formatter, "{path}: ownership {what}: {reason}")
                }
            }
            Self::Cancelled => formatter.write_str("the write was cancelled"),
            Self::Action { reason }
            | Self::Layout { reason }
            | Self::Storage { reason }
            | Self::Index { reason }
            | Self::Json { reason }
            | Self::Internal { reason } => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for WriteError {}

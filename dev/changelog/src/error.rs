//! The one failure type a command reports, and the status it stands for.

use std::fmt;
use std::path::PathBuf;

/// What a command reports when it cannot do its work.
///
/// Every failure carries the status the program exits with: that is the whole of what a command is
/// worth to whatever asked for it, and it is what `cargo changelog` exits with, so a caller branches
/// on one number however it reached the command. The one status that is not a failure is
/// [`crate::EC_NO_VERSION`], which `get-version` sets without an `Error`.
#[derive(Debug)]
pub enum Error {
    /// The directory the program was started in is not inside a git repository.
    NotRepository,
    /// The repository root has no manifest declaring a workspace.
    NoWorkspace {
        /// The manifest that was read.
        path: PathBuf,
    },
    /// A manifest cannot be read as the manifest it is named for.
    Manifest {
        /// The manifest that was read.
        path: PathBuf,
        /// What is wrong with it.
        detail: String,
    },
    /// The workspace section states no literal version.
    ManifestVersionMissing {
        /// The manifest that was read.
        path: PathBuf,
    },
    /// A file could not be read, written, removed or renamed.
    Io {
        /// What was being done, for a reader who has to know where it stopped.
        doing: &'static str,
        /// What a reader can recognise, with the paths the operation named.
        detail: String,
    },
    /// A tool a command runs could not be started.
    Tool {
        /// The program that could not be started.
        program: String,
        /// What the operating system answered.
        detail: String,
    },
    /// A program a command ran reported a failure.
    Status {
        /// The program that failed.
        program: String,
        /// The status it ended with.
        code: i32,
    },
    /// The tree has changes that a rewrite would be mixed into.
    DirtyTree {
        /// The paths `git status --porcelain` named, one per line.
        detail: String,
    },
    /// A draft is not the front matter and body the editor reads.
    Draft {
        /// The draft that was read.
        path: PathBuf,
        /// What is wrong with it.
        detail: String,
    },
    /// A version is not one this program will write into a manifest.
    InvalidVersion {
        /// The value that was named.
        value: String,
        /// Why it is not a version.
        reason: &'static str,
    },
    /// A draft names a category the changelog does not have.
    InvalidCategory {
        /// The category as the draft spelled it.
        value: String,
    },
    /// A draft names a scope that cannot be written into an entry title.
    InvalidScope {
        /// The scope as the draft spelled it.
        value: String,
    },
    /// A version has already been sealed and cannot be written to.
    Archived {
        /// The version that is sealed.
        version: String,
    },
    /// The changelog is open for a version the workspace is no longer at.
    StaleUnreleased {
        /// The version still marked `(Unreleased)`.
        stale: String,
        /// The version the workspace is at.
        current: String,
    },
    /// There is no `(Unreleased)` section to seal.
    NoUnreleased,
    /// `set-version` was run without a version.
    VersionNotNamed,
    /// A command was given a word it has no use for.
    UnknownArgument {
        /// The command they were given to.
        command: &'static str,
        /// The words no pick consumed.
        words: Vec<String>,
    },
    /// The system clock cannot say what today's UTC date is.
    Clock,
}

impl Error {
    /// The status a program exits with when this is what stopped it.
    ///
    /// A program a command ran keeps the status it ended with: what ran is the answer, and this
    /// only hands it on.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Status { code, .. } => *code,
            Self::NotRepository
            | Self::NoWorkspace { .. }
            | Self::Manifest { .. }
            | Self::ManifestVersionMissing { .. }
            | Self::Io { .. }
            | Self::Tool { .. }
            | Self::DirtyTree { .. }
            | Self::Draft { .. }
            | Self::InvalidVersion { .. }
            | Self::InvalidCategory { .. }
            | Self::InvalidScope { .. }
            | Self::Archived { .. }
            | Self::StaleUnreleased { .. }
            | Self::NoUnreleased
            | Self::VersionNotNamed
            | Self::UnknownArgument { .. }
            | Self::Clock => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRepository => write!(f, "not inside a git repository"),
            Self::NoWorkspace { path } => write!(
                f,
                "{}: the repository root declares no [workspace]",
                path.display()
            ),
            Self::Manifest { path, detail } | Self::Draft { path, detail } => {
                write!(f, "{}: {detail}", path.display())
            }
            Self::ManifestVersionMissing { path } => write!(
                f,
                "{}: [workspace.package] states no literal `version`",
                path.display()
            ),
            Self::Io { doing, detail } => write!(f, "{doing}: {detail}"),
            Self::Tool { program, detail } => {
                write!(f, "{program} could not be started: {detail}")
            }
            Self::Status { program, code } => {
                write!(f, "{program} failed with exit code {code}")
            }
            Self::DirtyTree { detail } => write!(
                f,
                "the working tree is not clean:\n{detail}\ncommit or stash the changes, or pass \
                 --allow-dirty"
            ),
            Self::InvalidVersion { value, reason } => {
                write!(f, "`{value}` is not a version: {reason}")
            }
            Self::InvalidCategory { value } => write!(
                f,
                "`{value}` is not a category: use bugfix, optimized, feature, or refactor"
            ),
            Self::InvalidScope { value } => write!(
                f,
                "`{value}` is not a scope: a scope is one word without brackets, backticks or spaces"
            ),
            Self::Archived { version } => write!(
                f,
                "`{version}` is already archived and cannot be written to; set the next version \
                 with `cargo changelog set-version <VERSION>`"
            ),
            Self::StaleUnreleased { stale, current } => write!(
                f,
                "CHANGELOG.md still has `{stale}` unreleased, but the workspace is at `{current}`; \
                 run `cargo changelog archive` to seal it, then write again"
            ),
            Self::NoUnreleased => {
                write!(f, "CHANGELOG.md has no `(Unreleased)` section to archive")
            }
            Self::VersionNotNamed => write!(
                f,
                "set-version needs a version: `cargo changelog set-version <VERSION>`"
            ),
            Self::UnknownArgument { command, words } => write!(
                f,
                "{command} does not take {}; see `cargo changelog --help`",
                words.join(" ")
            ),
            Self::Clock => write!(
                f,
                "the system clock is before the Unix epoch, so today's UTC date cannot be read"
            ),
        }
    }
}

impl std::error::Error for Error {}

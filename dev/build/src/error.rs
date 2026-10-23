//! The one failure type a step reports, and the status it stands for.

use std::fmt;
use std::path::PathBuf;

/// What a step reports when it cannot do its work.
///
/// Every failure carries the status the program exits with: that is the whole of what a build step
/// is worth to whatever asked for it, and it is what both entry points (`cargo dev` and
/// `./run.sh`) agree on.
#[derive(Debug)]
pub enum Error {
    /// No workspace above the directory the program was started in.
    NoWorkspace,
    /// A directory a build writes to, from an override that cannot name one.
    Directory {
        /// The environment variable the value came from, as a reader would set it.
        name: &'static str,
        /// The value it held.
        value: String,
        /// Why that value is not a directory a build may use.
        reason: &'static str,
    },
    /// The workspace's manifest does not say what a step needs.
    Manifest {
        /// The manifest that was read.
        path: PathBuf,
        /// What is wrong with it.
        detail: String,
    },
    /// A directory could not be read.
    Io {
        /// What was being done, for a reader who has to know where it stopped.
        doing: &'static str,
        /// What the operating system answered.
        detail: String,
    },
    /// A tool a step runs could not be started.
    Tool {
        /// The program that could not be started.
        program: String,
        /// What the operating system answered.
        detail: String,
    },
    /// A program a step ran reported a failure.
    Status {
        /// The program that failed.
        program: String,
        /// The status it ended with.
        code: i32,
    },
    /// Something a step needs that this machine does not have.
    Missing {
        /// What is missing, in one line.
        summary: String,
        /// What makes it available, one line each — never something done on the reader's behalf.
        advice: Vec<String>,
    },
}

impl Error {
    /// The status a program exits with when this is what stopped it.
    ///
    /// A step that failed keeps the status its program ended with: what ran is the answer, and this
    /// only hands it on.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Status { code, .. } => *code,
            Self::NoWorkspace
            | Self::Directory { .. }
            | Self::Manifest { .. }
            | Self::Io { .. }
            | Self::Tool { .. }
            | Self::Missing { .. } => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWorkspace => write!(
                f,
                "not inside a workspace: no Cargo.toml declaring [workspace] above the current directory"
            ),
            Self::Directory {
                name,
                value,
                reason,
            } => write!(
                f,
                "{name} is set to `{value}`, which is not a directory a build may write to: {reason}"
            ),
            Self::Manifest { path, detail } => write!(f, "{}: {detail}", path.display()),
            Self::Io { doing, detail } => write!(f, "{doing}: {detail}"),
            Self::Tool { program, detail } => {
                write!(f, "{program} could not be started: {detail}")
            }
            Self::Status { program, code } => {
                write!(f, "{program} failed with exit code {code}")
            }
            Self::Missing { summary, advice } => {
                write!(f, "{summary}")?;

                for line in advice {
                    write!(f, "\n{line}")?;
                }

                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {}

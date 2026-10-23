//! The toolchains a step runs, and how a machine's answer about one is read.
//!
//! `CARGO="cargo --offline"` is one variable holding a program and the words it is always run with:
//! a shell splits it before it runs, so this splits it too. Quoting it into one word would make it a
//! command that does not exist.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, ExitStatus, Output};

use crate::Error;

/// A toolchain: a program, and the words it is always run with.
#[derive(Clone, Debug)]
pub struct Tool {
    program: OsString,
    flags: Vec<OsString>,
}

impl Tool {
    /// The toolchain a step runs `cargo` with: `CARGO`, or the machine's own when it is unset.
    #[must_use]
    pub fn cargo() -> Self {
        Self::from_env("CARGO", "cargo")
    }

    /// The toolchain a step runs `dotnet` with: `DOTNET`, or the machine's own when it is unset.
    #[must_use]
    pub fn dotnet() -> Self {
        Self::from_env("DOTNET", "dotnet")
    }

    /// The renderer the icon set is drawn with: `RSVG`, or the machine's own when it is unset.
    #[must_use]
    pub fn rsvg() -> Self {
        Self::from_env("RSVG", "rsvg-convert")
    }

    /// The toolchain a step runs `git` with.
    #[must_use]
    pub fn git() -> Self {
        Self::from_env("GIT", "git")
    }

    /// The archiver an archive is made with.
    #[must_use]
    pub fn tar() -> Self {
        Self::from_env("TAR", "tar")
    }

    /// The toolchain `variable` names, or `default` when the machine says nothing.
    fn from_env(variable: &str, default: &'static str) -> Self {
        let raw = std::env::var(variable).ok();
        Self::read(raw.as_deref(), default)
    }

    /// A toolchain read from one value: the program first, then the words it is always run with.
    ///
    /// An empty value is the same as none: a variable set to nothing asked for nothing, which is what
    /// a shell's `:-` already reads it as.
    fn read(raw: Option<&str>, default: &'static str) -> Self {
        let mut words = raw.unwrap_or_default().split_whitespace();

        Self {
            program: words
                .next()
                .map_or_else(|| OsString::from(default), OsString::from),
            flags: words.map(OsString::from).collect(),
        }
    }

    /// The program, as the machine's answer named it.
    #[must_use]
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    /// The words the tool is always run with.
    #[must_use]
    pub fn flags(&self) -> &[OsString] {
        &self.flags
    }

    /// Runs the tool with `args` and answers the status it ended with, whatever that was.
    ///
    /// The step's own stdout and stderr are inherited rather than captured: what a compiler or a
    /// test runner says is for the reader, and only a step that has to read an answer asks for one
    /// (see [`Self::capture`]).
    ///
    /// # Errors
    ///
    /// Reports [`Error::Tool`] when the program cannot be started at all. A status that is not a
    /// success is the caller's to read: a step that tolerates a failure — the offline-first restore
    /// does — is where that is decided.
    pub fn status<I>(&self, args: I) -> Result<ExitStatus, Error>
    where
        I: IntoIterator,
        I::Item: Into<OsString>,
    {
        self.command(args)
            .status()
            .map_err(|error| self.could_not_start(&error))
    }

    /// Runs the tool with `args`, and reports the status it ended with unless it was a success.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Tool`] when the program cannot be started, and [`Error::Status`] carrying
    /// the status it ended with when that status is not a success.
    pub fn run<I>(&self, args: I) -> Result<(), Error>
    where
        I: IntoIterator,
        I::Item: Into<OsString>,
    {
        let status = self.status(args)?;

        if status.success() {
            Ok(())
        } else {
            Err(Error::Status {
                program: self.name(),
                code: status.code().unwrap_or(1),
            })
        }
    }

    /// Runs the tool with `args` in `directory`, with `environment` set for it, and answers the
    /// status it ended with.
    ///
    /// This is for the steps that run something somewhere other than where the build was started: a
    /// suite is run from beside the workspace it tests, and what it is told about the programs it
    /// runs is its environment.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Tool`] when the program cannot be started at all.
    pub fn status_in<I>(
        &self,
        directory: &Path,
        environment: &[(&str, &OsStr)],
        args: I,
    ) -> Result<ExitStatus, Error>
    where
        I: IntoIterator,
        I::Item: Into<OsString>,
    {
        let mut command = self.command(args);
        command.current_dir(directory);

        for (name, value) in environment {
            command.env(name, value);
        }

        command
            .status()
            .map_err(|error| self.could_not_start(&error))
    }

    /// Runs the tool with `args` in `directory`, and reports the status it ended with unless it was a
    /// success.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Tool`] when the program cannot be started, and [`Error::Status`] when it ran
    /// and failed.
    pub fn run_in<I>(&self, directory: &Path, args: I) -> Result<(), Error>
    where
        I: IntoIterator,
        I::Item: Into<OsString>,
    {
        let status = self.status_in(directory, &[], args)?;

        if status.success() {
            Ok(())
        } else {
            Err(Error::Status {
                program: self.name(),
                code: status.code().unwrap_or(1),
            })
        }
    }

    /// Runs the tool with `args` and captures what it said.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Tool`] when the program cannot be started. A status that is not a success is
    /// the caller's to read.
    pub fn capture<I>(&self, args: I) -> Result<Output, Error>
    where
        I: IntoIterator,
        I::Item: Into<OsString>,
    {
        self.command(args)
            .output()
            .map_err(|error| self.could_not_start(&error))
    }

    /// The command that runs this tool with `args`.
    fn command<I>(&self, args: I) -> Command
    where
        I: IntoIterator,
        I::Item: Into<OsString>,
    {
        let mut command = Command::new(&self.program);
        command.args(&self.flags);
        command.args(args.into_iter().map(Into::into));
        command
    }

    /// The program's name, for a failure that has to say what could not be started.
    fn name(&self) -> String {
        self.program.to_string_lossy().into_owned()
    }

    /// The failure that a program which cannot be started is.
    fn could_not_start(&self, error: &std::io::Error) -> Error {
        Error::Tool {
            program: self.name(),
            detail: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::Tool;

    #[test]
    fn a_toolchain_is_a_program_and_the_words_it_is_run_with() {
        let plain = Tool::read(None, "cargo");
        assert_eq!(plain.program(), "cargo");
        assert_eq!(plain.flags().len(), 0);

        // A variable holding a program and a flag is split the way a shell splits it, so
        // `CARGO="cargo --offline"` runs the one program with the one flag.
        let with_flags = Tool::read(Some("  cargo   --offline -q "), "cargo");
        assert_eq!(with_flags.program(), "cargo");
        assert_eq!(
            with_flags.flags(),
            [OsString::from("--offline"), OsString::from("-q")]
        );

        // A variable set to nothing asked for nothing.
        assert_eq!(Tool::read(Some(""), "dotnet").program(), "dotnet");
        assert_eq!(Tool::read(Some("   "), "dotnet").program(), "dotnet");

        // A plain command is used as it is.
        assert_eq!(
            Tool::read(Some("cargo-build-rola"), "cargo").program(),
            "cargo-build-rola"
        );
    }
}

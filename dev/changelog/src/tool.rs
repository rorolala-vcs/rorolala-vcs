//! The toolchains a command runs, and how a machine's answer about one is read.
//!
//! `CARGO="cargo --offline"` is one variable holding a program and the words it is always run with:
//! a shell splits it before it runs, so this splits it too. Quoting it into one word would make it a
//! command that does not exist.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};

use crate::Error;

/// A toolchain: a program, and the words it is always run with.
#[derive(Clone, Debug)]
pub struct Tool {
    program: OsString,
    flags: Vec<OsString>,
}

impl Tool {
    /// The toolchain a command runs `cargo` with: `CARGO`, or the machine's own when it is unset.
    #[must_use]
    pub fn cargo() -> Self {
        Self::from_env("CARGO", "cargo")
    }

    /// The toolchain a command runs `git` with: `GIT`, or the machine's own when it is unset.
    #[must_use]
    pub fn git() -> Self {
        Self::from_env("GIT", "git")
    }

    /// The toolchain `variable` names, or `default` when the machine says nothing.
    fn from_env(variable: &str, default: &'static str) -> Self {
        let raw = std::env::var(variable).ok();
        Self::read(raw.as_deref(), default)
    }

    /// A toolchain read from one value: the program first, then the words it is always run with.
    ///
    /// An empty value is the same as none: a variable set to nothing asked for nothing, which is
    /// what a shell's `:-` already reads it as.
    fn read(raw: Option<&str>, default: &'static str) -> Self {
        let mut words = raw.unwrap_or_default().split_whitespace();

        Self {
            program: words
                .next()
                .map_or_else(|| OsString::from(default), OsString::from),
            flags: words.map(OsString::from).collect(),
        }
    }

    /// Runs the tool with `args` in `directory`, and reports the status it ended with unless it was
    /// a success.
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
        let status = self
            .command(args)
            .current_dir(directory)
            .status()
            .map_err(|error| self.could_not_start(&error))?;

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
    /// Reports [`Error::Tool`] when the program cannot be started. A status that is not a success
    /// is the caller's to read.
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
    #[must_use]
    pub fn name(&self) -> String {
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
        assert_eq!(plain.program, "cargo");
        assert_eq!(plain.flags.len(), 0);

        // A variable holding a program and a flag is split the way a shell splits it, so
        // `CARGO="cargo --offline"` runs the one program with the one flag.
        let with_flags = Tool::read(Some("  cargo   --offline -q "), "cargo");
        assert_eq!(with_flags.program, "cargo");
        assert_eq!(
            with_flags.flags,
            [OsString::from("--offline"), OsString::from("-q")]
        );

        // A variable set to nothing asked for nothing.
        assert_eq!(Tool::read(Some(""), "git").program, "git");
        assert_eq!(Tool::read(Some("   "), "git").program, "git");
    }
}

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Where the programs this workspace builds are.
///
/// The gate sets `ROLA_BIN_DIR` to the release programs it built. Run by hand there is
/// nothing to set: a suite is run from the directory that holds it, so the programs are the
/// ones the workspace put beside that directory — `../.cache/rs-target/release`, as seen from
/// there.
#[must_use]
pub fn bin_dir() -> PathBuf {
    std::env::var_os("ROLA_BIN_DIR").map_or_else(
        || Path::new("..").join(".cache/rs-target/release"),
        PathBuf::from,
    )
}

/// A command that runs `program` out of [`bin_dir`].
///
/// The program is named rather than pathed, so a suite reaches the built programs the way
/// the gate names them to it and nothing has to be looked for.
#[must_use]
pub fn command(program: &str) -> Command {
    Command::new(bin_dir().join(program))
}

/// Runs `command` to completion and collects what it said.
///
/// The program is given no input, so one that reads from a terminal ends rather than waits
/// for a suite that has nothing to type into it. The command is taken by reference so that it
/// can be built where it is run: `arg` hands one back, which is what makes a command a chain
/// rather than a series of statements.
///
/// # Panics
///
/// Panics if the program could not be started at all, which is a broken environment rather
/// than an answer to be checked.
#[must_use]
pub fn run(command: &mut Command) -> Ran {
    let program = command.get_program().to_owned();
    command.stdin(Stdio::null());

    let output = command
        .output()
        .unwrap_or_else(|error| panic!("running {}: {error}", Path::new(&program).display()));

    Ran {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Starts `command` and waits until it accepts a connection at `at`.
///
/// What the program says is left to the terminal, because a program that serves is one a
/// suite wants to watch rather than collect: its logs are the whole story of what it did
/// with the connections afterwards. The command is taken by reference for the reason [`run`]
/// does.
///
/// # Panics
///
/// Panics if the program could not be started, if it stopped before it listened, or if it
/// was not listening within `within` — each of which is a suite that would otherwise wait
/// forever or check an answer nothing was there to give.
#[must_use]
pub fn serve(command: &mut Command, at: impl ToSocketAddrs, within: Duration) -> Serving {
    let program = command.get_program().to_owned();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("starting {}: {error}", Path::new(&program).display()));

    let deadline = Instant::now() + within;
    loop {
        if TcpStream::connect(&at).is_ok() {
            return Serving { child };
        }

        // A program that stopped on its own is one that will not be listening whatever is
        // waited for, so it is reported rather than waited out.
        if let Ok(Some(exit)) = child.try_wait() {
            panic!(
                "{} stopped before it listened: {exit}",
                Path::new(&program).display()
            );
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{} was not listening within {within:?}",
                Path::new(&program).display()
            );
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A directory of its own for one suite's run.
///
/// It is named after the suite and the process, so two runs at once do not meet, and it is
/// emptied when it is made, so a rerun starts clean.
pub struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    /// Makes a sandbox for the suite named `label`, emptied first so a rerun starts clean.
    ///
    /// # Panics
    ///
    /// Panics if the directory cannot be made, which no suite can get past anyway.
    #[must_use]
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("rolavcs-{label}-{}", std::process::id()));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the sandbox");

        Self { dir }
    }

    /// The directory itself.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// A path inside the sandbox.
    #[must_use]
    pub fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.dir.join(path)
    }

    /// Removes the sandbox and everything in it.
    ///
    /// It is asked for rather than done when the sandbox goes out of scope, so that a suite
    /// that panicked leaves behind whatever it had made to be looked at.
    pub fn cleanup(self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// What a program that ran to completion said.
pub struct Ran {
    /// The exit code, or `None` when a signal ended it rather than an ordinary return.
    pub code: Option<i32>,
    /// What it wrote to stdout.
    pub stdout: String,
    /// What it wrote to stderr.
    pub stderr: String,
}

impl Ran {
    /// Whether it ended successfully, which is the one exit code that says so.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Fails the suite unless it ended successfully, showing what it said when it did not.
    ///
    /// It borrows rather than consumes, so that a run that did end successfully can still be
    /// read afterwards — which is the usual shape: check the program was happy, then check what
    /// it said.
    ///
    /// # Panics
    ///
    /// Panics with what the program wrote, since a run that failed is a suite that has to
    /// say what was wrong with the program to be worth reading.
    pub fn expect_success(&self) {
        assert!(
            self.success(),
            "expected success, got {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code,
            self.stdout,
            self.stderr
        );
    }
}

/// A program that was started and is still running.
///
/// It is stopped when it goes out of scope, so a suite that returns early — even by
/// panicking — does not leave a program holding the port the next check needs.
pub struct Serving {
    child: Child,
}

impl Serving {
    /// Stops it now rather than when it goes out of scope.
    pub fn stop(mut self) {
        self.shut();
    }

    /// Kills it and waits for it to be gone, so that what it held is free again.
    fn shut(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.shut();
    }
}

#[cfg(test)]
mod tests {
    use super::Sandbox;

    #[test]
    fn a_sandbox_is_made_empty_and_taken_away_whole() {
        let sandbox = Sandbox::new("a-sandbox-is-made-empty-and-taken-away-whole");
        assert!(sandbox.dir().is_dir());
        assert_eq!(sandbox.dir().read_dir().expect("the sandbox").count(), 0);

        std::fs::write(sandbox.join("kept"), b"x").expect("writing in the sandbox");
        let dir = sandbox.dir().to_path_buf();
        sandbox.cleanup();

        assert!(!dir.exists());
    }
}

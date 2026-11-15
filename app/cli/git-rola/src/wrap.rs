//! Wrapping git's own commands so the materials are out of the way while it runs.
//!
//! git has no hook of its own before a checkout or a merge, so the "first take the materials out,
//! then run git, then put them back" order lives here: a word this program wraps is answered by
//! this module, and git is run with everything that followed it.
//!
//! What git says is git's, unchanged: its streams are inherited, and its status is what this run
//! ends with, a signal included. What this program says about its own half is git's too: `fatal:`
//! when it cannot act at all, `error:` when a material could not be put back, `warning:` when it
//! goes on regardless.

use rola_git_support::{Error as GitError, Session};
use rust_i18n::t;

use crate::exit_codes::EC_ERR;

/// The git commands that change the working tree, and so the materials with it.
pub const WRAPPED: [&str; 4] = ["checkout", "switch", "merge", "pull"];

/// The variable that says a run is already wrapping, so a git that reaches this program through a
/// hook does not wrap itself again.
const GUARD: &str = "ROLA_GIT_WRAPPING";

/// Takes the materials out, runs `verb`, and puts them back.
pub fn run(verb: &str, arguments: &[String]) -> i32 {
    if std::env::var_os(GUARD).is_some() {
        return git(verb, arguments);
    }

    let Ok(cwd) = std::env::current_dir() else {
        return git(verb, arguments);
    };

    let session = match Session::open(&cwd) {
        Ok(Some(session)) => session,
        // A work tree this build does not act in is refused before git is reached, since running
        // git there would leave the materials to be answered for by another work tree's lock.
        Err(error @ GitError::LinkedWorkTree) => {
            eprintln!("{}{error}", t!("message.fatal"));

            return EC_ERR;
        }
        // Anything else is git's to run: this program was named with a wrapped command, and what
        // the words after it mean is git's.
        Ok(None) | Err(_) => return git(verb, arguments),
    };

    let old = match session.lock() {
        Ok(lock) => lock,
        // The run goes on without the materials taken out: git's own answer is worth more than
        // this program giving up over a lock it could not read.
        Err(error) => {
            eprintln!("{}{error}", t!("message.warning"));

            return git(verb, arguments);
        }
    };

    // A tree is taken out only once the structure it was placed from is read, so the source is
    // made before the taking out rather than after git has run.
    let source = crate::source::of(session.project());

    if let Err(error) = session.detach(&source) {
        eprintln!("{}{error}", t!("message.fatal"));

        return EC_ERR;
    }

    let code = git(verb, arguments);

    let lock = if code == 0 {
        session.lock().unwrap_or(old)
    } else {
        old
    };

    // A material that could not be put back is an `error:` and nothing more: what the run ends with
    // is git's answer, and a wrapper that replaced it would be reporting its own failure as git's.
    match session.attach(&lock, &source) {
        Ok(failures) => {
            for (path, error) in &failures {
                eprintln!("{}{path}: {error}", t!("message.error"));
            }
        }
        Err(error) => eprintln!("{}{error}", t!("message.error")),
    }

    if let Err(error) = session.exclude(&lock) {
        eprintln!("{}{error}", t!("message.warning"));
    }

    code
}

/// Runs git with the words that followed the wrapped command.
fn git(verb: &str, arguments: &[String]) -> i32 {
    match std::process::Command::new("git")
        .arg(verb)
        .args(arguments)
        .env(GUARD, "1")
        .status()
    {
        Ok(status) => ended(status),
        Err(error) => {
            eprintln!(
                "{}{}",
                t!("message.fatal"),
                t!("git_rola.err_git", error = error.to_string())
            );

            EC_ERR
        }
    }
}

/// The status a git that has ended leaves this run with.
///
/// A child killed by a signal has no exit code. git's own callers read the number as 128 plus the
/// signal, which is what a shell computes when git dies on one, so the disposition is restored and
/// the signal raised in this process: what the shell sees is what it would have seen from git.
fn ended(status: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;

        if let Some(signal) = status.signal() {
            // SAFETY: the disposition of a signal this program never installed a handler for is
            // reset to the default, and the signal is raised in the thread reading the status. A
            // signal that is blocked or ignored leaves the fallback below to answer with git's
            // own arithmetic.
            unsafe {
                libc::signal(signal, libc::SIG_DFL);
                libc::raise(signal);
            }

            return EC_ERR + signal;
        }
    }

    status.code().unwrap_or(EC_ERR)
}

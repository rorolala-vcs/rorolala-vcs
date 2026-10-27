//! Wrapping git's own commands so the materials are out of the way while it runs.
//!
//! git has no hook of its own before a checkout or a merge, so the "first take the materials out,
//! then run git, then put them back" order lives here: a word this program wraps is answered by
//! this, and git is run with everything that followed it.

use rola_git_support::{Error as GitError, Session};
use rust_i18n::t;

use crate::exit_codes::EC_ERR;

/// The git commands that change the working tree, and so the materials with it.
const WRAPPED: [&str; 4] = ["checkout", "switch", "merge", "pull"];

/// The variable that says a run is already wrapping, so a git that reaches this program through a
/// hook does not wrap itself again.
const GUARD: &str = "ROLA_GIT_WRAPPING";

/// Runs `verb` wrapped when the run names a wrapped command, and nothing otherwise.
pub fn intercept(arguments: &mut Vec<String>) {
    let Some(verb) = arguments.first().cloned() else {
        return;
    };

    if !WRAPPED.contains(&verb.as_str()) {
        return;
    }

    let rest: Vec<String> = arguments.drain(1..).collect();

    std::process::exit(run(&verb, &rest));
}

/// Takes the materials out, runs `verb`, and puts them back.
fn run(verb: &str, arguments: &[String]) -> i32 {
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
            eprintln!("git-rola: {error}");

            return EC_ERR;
        }
        // Anything else is git's to run: this program was named with a wrapped command, and what
        // the words after it mean is git's.
        Ok(None) | Err(_) => return git(verb, arguments),
    };

    let old = match session.lock() {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("git-rola: {error}");

            return git(verb, arguments);
        }
    };

    if let Err(error) = session.detach() {
        eprintln!("git-rola: {error}");

        return EC_ERR;
    }

    let code = git(verb, arguments);

    let source = crate::source::of(session.project());
    let lock = if code == 0 {
        session.lock().unwrap_or(old)
    } else {
        old
    };

    match session.attach(&lock, &source) {
        Ok(failures) => {
            for (path, error) in &failures {
                eprintln!("git-rola: {path}: {error}");
            }
        }
        Err(error) => eprintln!("git-rola: {error}"),
    }

    if let Err(error) = session.exclude(&lock) {
        eprintln!("git-rola: {error}");
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
        Ok(status) => status.code().unwrap_or(EC_ERR),
        Err(error) => {
            let said = t!("git_rola.err_git", error = error.to_string()).to_string();
            eprintln!("git-rola: {said}");

            EC_ERR
        }
    }
}

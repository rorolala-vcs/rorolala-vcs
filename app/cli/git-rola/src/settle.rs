//! Putting the work tree, the state and the exclude block in step with the project's lock.
//!
//! Two commands answer with the same thing: `hook` after git changed the tree, and `refresh` when a
//! person asks for the lock to be applied. Both go through [`Session::settle`], which takes the
//! materials it recorded out, puts the lock's back, and rewrites the exclude block. What is here is
//! the reading of the lock and the failures said the way this program says them, so the two
//! commands cannot drift apart in either.

use mingling::{
    Grouped,
    macros::{buffer, r_eprintln, renderer},
    res::ResExitCode,
};
use rola_git_support::Session;
use rust_i18n::t;

use crate::exit_codes::EC_FAILED;

/// Result: what a settle could not place.
#[derive(Grouped)]
pub struct ResultSettled {
    /// Each material that could not be placed, and why.
    pub failures: Vec<(String, String)>,
}

/// Renders what a settle could not place, and ends the run on the first of them with git's 1.
#[renderer(buffer)]
pub fn render_settled(result: ResultSettled, ec: &mut ResExitCode) {
    for (path, error) in &result.failures {
        r_eprintln!(
            "{}{}",
            t!("message.error"),
            t!(
                "message.failed_path",
                path = path.as_str(),
                error = error.as_str()
            )
        );
    }

    if !result.failures.is_empty() {
        ec.exit_code = EC_FAILED;
    }
}

/// Applies the lock the project carries.
///
/// # Errors
///
/// Returns a message when the lock cannot be read, or when a material was changed after it was
/// placed: [`Session::settle`] checks every recorded material before it moves any, so a run that
/// cannot settle leaves the work tree as it was.
pub fn apply(session: &Session) -> Result<ResultSettled, String> {
    let lock = session
        .lock()
        .map_err(|error| t!("git_rola.err_lock", error = error.to_string()).to_string())?;
    let source = crate::source::of(session.project());
    let failures = session
        .settle(&lock, &source)
        .map_err(|error| error.to_string())?;

    Ok(ResultSettled {
        failures: failures
            .into_iter()
            .map(|(path, error)| (path, error.to_string()))
            .collect(),
    })
}

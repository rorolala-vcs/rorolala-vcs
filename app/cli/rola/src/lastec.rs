//! The exit code of the last run that failed, kept in Rorolala's own file for the user.
//!
//! It is what `rola explain exit-code` explains when it is given no number: the run just
//! before is the one being asked about, and all of it that is kept is a number. Only a run
//! that failed is written down — a run that did nothing wrong has nothing to explain, and
//! writing it would bury the failure a reader still has a question about.
//!
//! It is the smallest of the files Rorolala keeps — written once as a run ends, read once
//! when it is asked about — and a machine that does not name a local data directory has
//! nowhere to keep it, so both sides stay silent about a file they cannot reach.

use std::fs;

use crate::user::lastec_path;

/// Records `code` as the exit code of this run.
///
/// This is called only for a run that failed, so the file holds the last failure rather
/// than the last run. It runs as the run ends, so a failure here is silent: there is
/// nothing left to report it to, and a record that was not kept only means the next `rola
/// explain exit-code` has nothing to explain.
pub fn record(code: i32) {
    let Some(path) = lastec_path() else {
        return;
    };
    let Some(directory) = path.parent() else {
        return;
    };

    if fs::create_dir_all(directory).is_err() {
        return;
    }

    // A trailing newline makes the file read as one line, which is what `last` expects of
    // it and what anything that opens it in an editor shows as a line rather than a stray
    // number at the start of one.
    let _ = fs::write(path, format!("{code}\n"));
}

/// The exit code the last run that failed ended with, if one was recorded.
///
/// What cannot be read is not told apart from what was never written: a file that is
/// missing and one that does not hold a number both come back as no code, which is the one
/// answer a caller can act on.
#[must_use]
pub fn last() -> Option<i32> {
    let text = fs::read_to_string(lastec_path()?).ok()?;

    text.trim().parse().ok()
}

//! The exit codes `git-rola` ends with.
//!
//! They follow git's own reading of a status: nothing to report is zero, a command that could not
//! be made is 128, a command line that is not one this program reads is 129, and a command that
//! reports a difference rather than a failure — `check` — answers one.

/// The command did what it was asked.
pub const EC_OK: i32 = 0;

/// What the working tree holds differs from `rola.lock`.
pub const EC_DIFFERENCES: i32 = 1;

/// The command could not be made.
pub const EC_ERR: i32 = 128;

/// The command line was not one this program reads.
pub const EC_USAGE: i32 = 129;

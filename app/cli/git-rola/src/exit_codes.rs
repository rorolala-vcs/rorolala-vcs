//! The exit codes `git-rola` ends with.
//!
//! They are git's, which reads a status as three different answers: 1 for a command that read what
//! it was given and could not do it, 128 for one that could not be made at all, and 129 for a
//! command line that is not one the program reads. A run that reports a difference rather than a
//! failure, `check`, answers 1 as well, the same number `git diff --exit-code` ends with.

/// The command did what it was asked, which is git's 0.
pub const EC_OK: i32 = 0;

/// What the working tree holds differs from `rola.lock`, which is what `git diff --exit-code`
/// answers with.
pub const EC_DIFFERENCES: i32 = 1;

/// The command read what it was given and could not do it: a pathspec matched nothing, a material
/// could not be put back, a word named no command. This is git's 1.
pub const EC_FAILED: i32 = 1;

/// The command could not be made at all, which git says with `fatal:`. This is git's 128.
pub const EC_ERR: i32 = 128;

/// The command line was not one this program reads, which git answers with a usage line. This is
/// git's 129.
pub const EC_USAGE: i32 = 129;

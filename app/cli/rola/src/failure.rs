//! What a failure of this program says about itself, for a reader that is a program.
//!
//! A failure is reported to a person as a line — see `err_line!` — and to a program as
//! `{"name": ..., "reason": ...}`. The two are the same failure told twice, so each error
//! type says it once: [`Failure`](rorolala_errors::Failure) is where the name and the reason
//! are written, the renderer draws what it says rather than saying it again, and what is
//! handed over when a run is read rather than watched is the same two things.

/// Implements the structured form of a failure, and registers it as one the program can read.
///
/// Both halves are needed and neither is of much use alone. The first writes
/// `{"name": ..., "reason": ...}`, which is [`rorolala_errors::failure!`]; the second is what
/// lets `gen_program!` find the type at all, which is the whole difference between a failure
/// a program can read and a `null` where a failure was.
///
/// The name is spelled as the type is — see [`Failure::name`](rorolala_errors::Failure::name)
/// — and is not translated, so that what a program branches on does not change with the
/// language a run was asked for.
macro_rules! failure {
    ($failure:ty) => {
        ::mingling::macros::structural!($failure);
        ::rorolala_errors::failure!($failure);
    };
}

pub(crate) use failure;

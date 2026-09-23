//! What a failure says about itself, for a reader that is a program.
//!
//! A failure is spoken about in two ways at once, and they are for two different readers. A
//! person reads a sentence in the language the run was asked to speak; a program reads a
//! name, which is the program's own and means the same thing whatever language the run was
//! asked for. This is the shape both are handed over in, so that what a program branches on
//! does not have to be dug out of a translated sentence — which is the one thing that cannot
//! be branched on, since it changes with the locale.

use serde::Serialize;

/// What a failure is called and why it happened.
///
/// A failure that a program may read reports both: the name it is known by, which is stable
/// and untranslated, and the reason, which is written in the run's language where the failure
/// knows one.
pub trait Failure {
    /// The name this failure is known by.
    ///
    /// The name is the program's own and is never translated: it is what a reader branches
    /// on, so it has to mean the same thing in every locale. It is written the way the
    /// program's other names are — lowercase, with words joined by `_`.
    fn name(&self) -> &'static str;

    /// Why it happened, in the run's language.
    ///
    /// A failure that knows no words for itself may leave this empty; the name and the exit
    /// code are then the whole of what it reports, and what a person reads is written where
    /// the failure is rendered rather than here.
    fn reason(&self) -> String;

    /// This failure as it is handed over: the name, and the reason.
    #[must_use]
    fn report(&self) -> Report {
        Report {
            name: self.name(),
            reason: self.reason(),
        }
    }
}

/// One failure, written down the way a program reads it.
///
/// The fields are `name` and `reason`, and they are what a program reads a failure by: see
/// [`Failure`]. It is written out as one line, since a reader that parses it has no use for
/// the line breaks a person would be shown it with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The name the failure is known by.
    name: &'static str,
    /// Why it happened, in the run's language.
    reason: String,
}

/// Implements the structured form of a failure: `{"name": ..., "reason": ...}`.
///
/// The type must already implement [`Failure`], which is where the two fields come from. What
/// this adds is the shape they are written in, so that every failure a program can read is
/// read the same way rather than each in the words of whoever wrote it.
///
/// # Examples
///
/// ```ignore
/// impl Failure for ErrorNotBound {
///     fn name(&self) -> &'static str {
///         "error_not_bound"
///     }
///
///     fn reason(&self) -> String {
///         format!("`{}` is not bound", self.name)
///     }
/// }
///
/// failure!(ErrorNotBound);
/// ```
#[macro_export]
macro_rules! failure {
    ($failure:ty) => {
        impl $crate::__serde::Serialize for $failure {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: $crate::__serde::Serializer,
            {
                $crate::__serde::Serialize::serialize(&$crate::Failure::report(self), serializer)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{Failure, Report};

    /// A failure of this test's own, so that the shape is read without a program around it.
    struct ErrorMissing {
        what: &'static str,
    }

    impl Failure for ErrorMissing {
        fn name(&self) -> &'static str {
            "error_missing"
        }

        fn reason(&self) -> String {
            format!("{} is not there", self.what)
        }
    }

    failure!(ErrorMissing);

    #[test]
    fn a_failure_reports_its_name_and_its_reason() {
        let reported = ErrorMissing { what: "a vault" }.report();

        assert_eq!(
            reported,
            Report {
                name: "error_missing",
                reason: "a vault is not there".to_owned(),
            }
        );
    }

    #[test]
    fn a_failure_is_written_out_as_the_two_fields_it_reports() {
        let written = serde_json::to_string(&ErrorMissing { what: "a vault" }).unwrap();

        assert_eq!(
            written,
            r#"{"name":"error_missing","reason":"a vault is not there"}"#
        );
    }
}

//! The checks, one to a module, in the order the gate asks for them.
//!
//! A check is a command here rather than a script of its own: what it needs from the tree it reads
//! from `rola-build`, and what it leaves behind is what `report-collect` renders.

use mingling::res::ResExitCode;
use rola_build::Error;

use crate::reporter::Reporter;

pub mod check;
pub mod clippy;
pub mod doc;
pub mod doc_open;
pub mod dotnet_check;
pub mod dotnet_integration_test;
pub mod dotnet_lint;
pub mod fmt;
pub mod fmt_check;
pub mod generated;
pub mod i18n;
pub mod integration_test;
pub mod test;

// See the note at the crate root: a check's generated types have to be in scope there, and this is
// what puts them on the way. The wildcard and the crate-private path are the point for the same
// reason they are there.
#[allow(clippy::wildcard_imports, clippy::redundant_pub_crate)]
pub(crate) use {
    check::*, clippy::*, doc::*, doc_open::*, dotnet_check::*, dotnet_integration_test::*,
    dotnet_lint::*, fmt::*, fmt_check::*, generated::*, i18n::*, integration_test::*, test::*,
};

/// Runs one check, records how it ended, and gives its status to the run.
///
/// A check is the tree's own program run as it always was — its output goes to the terminal as it
/// runs — and what is recorded is only where it was run and how it ended: the report is a reading of
/// a run, not a second copy of its output.
fn run_check(
    task: &str,
    command: &str,
    ec: &mut ResExitCode,
    run: impl FnOnce() -> Result<(), Error>,
) {
    let reporter = Reporter::new();

    match run() {
        Ok(()) => reporter.task(task).passed("workspace", command),
        Err(error) => {
            eprintln!("{error}");
            reporter.task(task).failed("workspace", command, "");
            ec.exit_code = error.exit_code();
        }
    }
}

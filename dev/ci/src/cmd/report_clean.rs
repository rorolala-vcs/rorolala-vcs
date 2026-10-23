//! `report-clean`: what a previous run recorded, removed.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
};

use crate::{Next, reporter::Reporter};

/// Removes everything a previous run recorded, and the report rendered from it.
#[command(node = "report-clean")]
pub fn report_clean() -> StateReportClean {
    StateReportClean
}

/// The state the command starts in: it names nothing, since the whole of what was recorded is what
/// is removed.
#[derive(Grouped)]
pub struct StateReportClean;

/// Removes what was collected and rendered.
#[chain]
pub fn handle_report_clean(_state: StateReportClean) -> Next {
    Reporter::new().clean();

    empty_result!()
}

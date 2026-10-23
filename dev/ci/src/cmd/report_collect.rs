//! `report-collect`: a run's recorded outcomes, rendered as one page.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
};

use crate::{Next, reporter::Reporter};

/// Renders what the run recorded into `.cache/reports/result.md`.
#[command(node = "report-collect")]
pub fn report_collect() -> StateReportCollect {
    StateReportCollect
}

/// The state the command starts in: it names nothing, since everything recorded is what is
/// rendered.
#[derive(Grouped)]
pub struct StateReportCollect;

/// Renders the run.
#[chain]
pub fn handle_report_collect(_state: StateReportCollect) -> Next {
    match Reporter::new().collect() {
        Ok(Some(path)) => println!("report = {}", path.display()),
        Ok(None) => eprintln!("warning: not inside a workspace; there is nothing to render"),
        Err(error) => eprintln!("warning: {error}"),
    }

    empty_result!()
}

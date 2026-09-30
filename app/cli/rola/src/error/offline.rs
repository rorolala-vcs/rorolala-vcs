//! The error for a run that may not reach the Vault.
//!
//! `--offline` is one flag for every command, but it does not mean the same thing to all of them:
//! a command whose work is the exchange with the Vault is refused, while one that only fetches the
//! Vault's Layout on the way to local work goes on without it. What is left for this module is the
//! first kind — the refusal — said once, so every command that makes it says it the same way.

use mingling::{
    Grouped,
    macros::{buffer, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::exit_codes::EC_ERR_OFFLINE;
use crate::failure::failure;

/// Error: a command that reaches the Vault was run offline.
#[derive(Grouped)]
pub struct ErrorOffline;

impl Failure for ErrorOffline {
    fn name(&self) -> &'static str {
        "error_offline"
    }

    fn reason(&self) -> String {
        t!("error.offline.err_offline").trim().to_string()
    }
}

failure!(ErrorOffline);

/// Reports a command whose work is the exchange with the Vault.
#[renderer(buffer)]
pub fn render_error_offline(error: ErrorOffline, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.offline.err_offline_help").trim())
    );
    ec.exit_code = EC_ERR_OFFLINE;
}

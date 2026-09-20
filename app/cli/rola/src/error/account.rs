//! Errors for a command that has to act as someone.
//!
//! A name is only a label, so acting as an account is done by finding the key under it, and
//! an account is what a client proves an identity with: a command that talks to a Vault has
//! nothing to establish without one. The parts of the run that know answer with these, and
//! they are rendered here rather than in a command, since more than one command asks.

use mingling::{
    macros::{buffer, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::account::ErrorNoAccount;
use crate::exit_codes::{EC_ERR_ACCOUNT_NOT_BOUND, EC_ERR_ACCOUNT_NOT_FOUND};
use crate::keys::ErrorAccountUnknown;

/// Reports a run that acts as no account, where the command needs one.
#[renderer(buffer)]
pub fn render_error_no_account(_: ErrorNoAccount, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("error.account.err_no_account").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.account.err_no_account_help").trim())
    );
    ec.exit_code = EC_ERR_ACCOUNT_NOT_BOUND;
}

/// Reports a name no scope holds an account under.
#[renderer(buffer)]
pub fn render_error_account_unknown(error: ErrorAccountUnknown, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("error.account.err_unknown", name = error.name()).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("error.account.err_unknown_help").trim())
    );
    ec.exit_code = EC_ERR_ACCOUNT_NOT_FOUND;
}

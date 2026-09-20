//! Errors for a command that has to act as someone.
//!
//! An account is what a client proves an identity with, so a command that talks to a Vault
//! has nothing to establish without one. The resource that holds the choice answers with this,
//! and it is rendered here rather than in a command, since more than one command will ask.

use mingling::{
    macros::{buffer, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::account::ErrorNoAccount;
use crate::exit_codes::EC_ERR_ACCOUNT_NOT_BOUND;

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

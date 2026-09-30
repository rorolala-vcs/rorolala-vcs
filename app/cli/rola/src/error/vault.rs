//! Errors for the `rola vault` commands.
//!
//! A name that is missing and a name that is not bound are answers more than one of those
//! commands gives — binding, letting go, and choosing all need a name, and the last two both
//! answer when it names nothing — so they are rendered here rather than in any one of them.

use mingling::{
    Grouped,
    macros::{buffer, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::exit_codes::{EC_ERR_VAULT_ARGUMENT, EC_ERR_VAULT_NOT_ADMIN, EC_ERR_VAULT_NOT_BOUND};
use crate::failure::failure;

/// Error: the name a `rola vault` command was given is missing.
#[derive(Grouped)]
pub struct ErrorVaultNameMissing;

impl Failure for ErrorVaultNameMissing {
    fn name(&self) -> &'static str {
        "error_vault_name_missing"
    }

    fn reason(&self) -> String {
        t!("error.vault.err_name_missing").trim().to_string()
    }
}

failure!(ErrorVaultNameMissing);

#[renderer(buffer)]
pub fn render_error_vault_name_missing(error: ErrorVaultNameMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.vault.err_name_missing_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}

/// Error: the name a `rola vault` command was given is not bound to anything.
#[derive(Grouped)]
pub struct ErrorVaultNotBound {
    /// The name that is not bound.
    pub(crate) name: String,
}

impl Failure for ErrorVaultNotBound {
    fn name(&self) -> &'static str {
        "error_vault_not_bound"
    }

    fn reason(&self) -> String {
        t!("error.vault.err_not_bound", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorVaultNotBound);

#[renderer(buffer)]
pub fn render_error_vault_not_bound(error: ErrorVaultNotBound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.vault.err_not_bound_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_NOT_BOUND;
}

/// Error: the Vault names no administrator, so nobody may name one.
#[derive(Grouped)]
pub struct ErrorVaultAdminMissing;

impl Failure for ErrorVaultAdminMissing {
    fn name(&self) -> &'static str {
        "error_vault_admin_missing"
    }

    fn reason(&self) -> String {
        t!("error.vault.err_admin_missing").trim().to_string()
    }
}

failure!(ErrorVaultAdminMissing);

#[renderer(buffer)]
pub fn render_error_vault_admin_missing(error: ErrorVaultAdminMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.vault.err_admin_missing_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_NOT_ADMIN;
}

/// Error: the account this run acts as is not an administrator of the Vault.
#[derive(Grouped)]
pub struct ErrorVaultNotAdmin {
    /// The account that is not an administrator.
    pub name: String,
}

impl Failure for ErrorVaultNotAdmin {
    fn name(&self) -> &'static str {
        "error_vault_not_admin"
    }

    fn reason(&self) -> String {
        t!("error.vault.err_not_admin", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorVaultNotAdmin);

#[renderer(buffer)]
pub fn render_error_vault_not_admin(error: ErrorVaultNotAdmin, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.vault.err_not_admin_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_NOT_ADMIN;
}

/// Error: the name a `rola vault admin rm` was given is not an administrator.
#[derive(Grouped)]
pub struct ErrorVaultAdminUnknown {
    /// The name that is not an administrator.
    pub name: String,
}

impl Failure for ErrorVaultAdminUnknown {
    fn name(&self) -> &'static str {
        "error_vault_admin_unknown"
    }

    fn reason(&self) -> String {
        t!("error.vault.err_admin_unknown", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorVaultAdminUnknown);

#[renderer(buffer)]
pub fn render_error_vault_admin_unknown(error: ErrorVaultAdminUnknown, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.vault.err_admin_unknown_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_NOT_ADMIN;
}

/// Error: removing the administrator named would leave the Vault with none.
#[derive(Grouped)]
pub struct ErrorVaultLastAdmin;

impl Failure for ErrorVaultLastAdmin {
    fn name(&self) -> &'static str {
        "error_vault_last_admin"
    }

    fn reason(&self) -> String {
        t!("error.vault.err_last_admin").trim().to_string()
    }
}

failure!(ErrorVaultLastAdmin);

#[renderer(buffer)]
pub fn render_error_vault_last_admin(error: ErrorVaultLastAdmin, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("error.vault.err_last_admin_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_NOT_ADMIN;
}

//! Errors for a command that works from somewhere particular.
//!
//! Not every command can do its work wherever it is run: one that works on the copy being
//! edited needs a Workspace, one that serves the other side needs a Vault, and one that
//! reaches for a Vault needs the Workspace beside it to say which. The resources that know
//! answer with these, and they are rendered in one place because the same answer comes from
//! more than one of them.

use std::path::PathBuf;

use mingling::{
    Grouped,
    macros::{buffer, import_type, r_append, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::exit_codes::{
    EC_ERR_CONFIG_UNREADABLE, EC_ERR_SHOULD_IN_VAULT, EC_ERR_SHOULD_IN_WORKSPACE,
};

import_type!(ErrorShouldInWorkspace = rorolala_cli_setups::ErrorShouldInWorkspace);

/// Reports a run that is not inside a Workspace, where the command needs one.
#[renderer(buffer)]
pub fn render_error_should_in_workspace(_: ErrorShouldInWorkspace, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("error.placement.err_should_in_workspace").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("error.placement.err_should_in_workspace_help").trim())
    );
    ec.exit_code = EC_ERR_SHOULD_IN_WORKSPACE;
}

import_type!(ErrorShouldInVault = rorolala_cli_setups::ErrorShouldInVault);

/// Reports a run that is not inside a Vault, where the command needs one.
#[renderer(buffer)]
pub fn render_error_should_in_vault(_: ErrorShouldInVault, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("error.placement.err_should_in_vault").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("error.placement.err_should_in_vault_help").trim())
    );
    ec.exit_code = EC_ERR_SHOULD_IN_VAULT;
}

import_type!(ErrorRemoteVault = rorolala_cli_setups::ErrorRemoteVault);

/// Reports a Workspace that cannot say which Vault a run reaches for.
///
/// What it says is what the command that needed the Vault would have said, told in the same
/// words: reaching for a Vault is one more thing a Workspace is needed for, and one more
/// thing its configuration is read for.
#[renderer(buffer)]
pub fn render_error_remote_vault(err: ErrorRemoteVault, ec: &mut ResExitCode) {
    match err {
        rorolala_cli_setups::ErrorRemoteVault::ShouldInWorkspace => {
            r_append!(render_error_should_in_workspace(
                rorolala_cli_setups::ErrorShouldInWorkspace,
                ec
            ));
        }
        rorolala_cli_setups::ErrorRemoteVault::Unread { path, reason } => {
            r_append!(render_error_config_unreadable(
                ErrorConfigUnreadable::new(path, reason),
                ec
            ));
        }
    }
}

/// Error: the Workspace's configuration could not be read.
///
/// A Workspace that cannot be read is a Workspace whose Vaults cannot be found, which stops
/// whichever command was asking for one, whether it works on the Workspace or reaches for a
/// Vault through it.
#[derive(Grouped)]
pub struct ErrorConfigUnreadable {
    /// The file that could not be read.
    path: PathBuf,
    /// Why it could not be read.
    reason: String,
}

impl ErrorConfigUnreadable {
    /// The error for a Workspace whose configuration would not read.
    ///
    /// A command outside this module that needs the configuration says so through this, since
    /// the fields are the module's own.
    pub(crate) fn new(path: PathBuf, reason: String) -> Self {
        Self { path, reason }
    }
}

/// Reports the Workspace's configuration, which would not read.
#[renderer(buffer)]
pub fn render_error_config_unreadable(error: ErrorConfigUnreadable, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(
            t!(
                "error.placement.err_config_unreadable",
                path = error.path.display().to_string()
            )
            .trim()
        )
    );
    r_eprintln!(
        "{}",
        help_line!(
            t!(
                "error.placement.err_config_unreadable_help",
                reason = error.reason
            )
            .trim()
        )
    );
    ec.exit_code = EC_ERR_CONFIG_UNREADABLE;
}

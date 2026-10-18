//! Failures the daemon's commands share.
//!
//! Every one of them is about the table of namespaces or about the arguments naming a row in it,
//! so the type and the line it is reported as are written once here rather than once per command.

use std::path::PathBuf;

use mingling::{
    Grouped,
    macros::{buffer, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::{
    exit_codes::{
        EC_ALREADY_EXIST, EC_ERR_ARGUMENT, EC_ERR_DAEMON_CONFIG, EC_ERR_NO_DATA_DIR, EC_NOT_EXIST,
    },
    failure::failure,
};

/// Error: this machine names no place for the user's own files.
#[derive(Grouped)]
pub struct ErrorNoDataDir;

impl Failure for ErrorNoDataDir {
    fn name(&self) -> &'static str {
        "error_no_data_dir"
    }

    fn reason(&self) -> String {
        t!("error.no_data_dir").to_string()
    }
}

failure!(ErrorNoDataDir);

#[renderer(buffer)]
pub fn render_error_no_data_dir(_: ErrorNoDataDir, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("error.no_data_dir").trim()));
    r_eprintln!("{}", help_line!(t!("error.no_data_dir_help").trim()));
    ec.exit_code = EC_ERR_NO_DATA_DIR;
}

/// Error: the table of namespaces could not be read or written.
#[derive(Grouped)]
pub struct ErrorRegistry {
    /// The file that could not be reached.
    path: PathBuf,
    /// What the table's own reader or writer said about it.
    reason: String,
}

impl ErrorRegistry {
    /// A failure to reach the table at `path`, for `reason`.
    #[must_use]
    pub fn new(path: PathBuf, reason: impl Into<String>) -> Self {
        Self {
            path,
            reason: reason.into(),
        }
    }
}

impl Failure for ErrorRegistry {
    fn name(&self) -> &'static str {
        "error_registry"
    }

    fn reason(&self) -> String {
        t!(
            "error.registry",
            path = self.path.display().to_string(),
            reason = self.reason
        )
        .to_string()
    }
}

failure!(ErrorRegistry);

#[renderer(buffer)]
pub fn render_error_registry(error: ErrorRegistry, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("error.registry_help").trim()));
    ec.exit_code = EC_ERR_DAEMON_CONFIG;
}

/// Error: a command that needs an argument was given none of it.
#[derive(Grouped)]
pub struct ErrorArgument {
    /// The argument the command names in its usage line.
    argument: &'static str,
}

impl ErrorArgument {
    /// The failure for a command that named no `argument`.
    #[must_use]
    pub const fn missing(argument: &'static str) -> Self {
        Self { argument }
    }
}

impl Failure for ErrorArgument {
    fn name(&self) -> &'static str {
        "error_argument"
    }

    fn reason(&self) -> String {
        t!("error.argument", argument = self.argument).to_string()
    }
}

failure!(ErrorArgument);

#[renderer(buffer)]
pub fn render_error_argument(error: ErrorArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("error.argument_help").trim()));
    ec.exit_code = EC_ERR_ARGUMENT;
}

/// Error: a name is not one a namespace may be written as.
#[derive(Grouped)]
pub struct ErrorNamespaceInvalid {
    /// The name that would not read.
    namespace: String,
}

impl ErrorNamespaceInvalid {
    /// The failure for `namespace`.
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }
}

impl Failure for ErrorNamespaceInvalid {
    fn name(&self) -> &'static str {
        "error_namespace_invalid"
    }

    fn reason(&self) -> String {
        t!("error.namespace_invalid", namespace = self.namespace).to_string()
    }
}

failure!(ErrorNamespaceInvalid);

#[renderer(buffer)]
pub fn render_error_namespace_invalid(error: ErrorNamespaceInvalid, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("error.namespace_invalid_help").trim()));
    ec.exit_code = EC_ERR_ARGUMENT;
}

/// Error: a namespace is already registered.
#[derive(Grouped)]
pub struct ErrorAlreadyRegistered {
    /// The name that is taken.
    namespace: String,
}

impl ErrorAlreadyRegistered {
    /// The failure for `namespace`.
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }
}

impl Failure for ErrorAlreadyRegistered {
    fn name(&self) -> &'static str {
        "error_already_registered"
    }

    fn reason(&self) -> String {
        t!("error.already_registered", namespace = self.namespace).to_string()
    }
}

failure!(ErrorAlreadyRegistered);

#[renderer(buffer)]
pub fn render_error_already_registered(error: ErrorAlreadyRegistered, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("error.already_registered_help").trim()));
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: no namespace is registered under the name.
#[derive(Grouped)]
pub struct ErrorNotRegistered {
    /// The name that reaches nothing.
    namespace: String,
}

impl ErrorNotRegistered {
    /// The failure for `namespace`.
    #[must_use]
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }
}

impl Failure for ErrorNotRegistered {
    fn name(&self) -> &'static str {
        "error_not_registered"
    }

    fn reason(&self) -> String {
        t!("error.not_registered", namespace = self.namespace).to_string()
    }
}

failure!(ErrorNotRegistered);

#[renderer(buffer)]
pub fn render_error_not_registered(error: ErrorNotRegistered, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("error.not_registered_help").trim()));
    ec.exit_code = EC_NOT_EXIST;
}

/// Error: no path was named and the run is not inside a Vault.
#[derive(Grouped)]
pub struct ErrorNotInVault;

impl Failure for ErrorNotInVault {
    fn name(&self) -> &'static str {
        "error_not_in_vault"
    }

    fn reason(&self) -> String {
        t!("error.not_in_vault").to_string()
    }
}

failure!(ErrorNotInVault);

#[renderer(buffer)]
pub fn render_error_not_in_vault(_: ErrorNotInVault, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("error.not_in_vault").trim()));
    r_eprintln!("{}", help_line!(t!("error.not_in_vault_help").trim()));
    ec.exit_code = EC_ERR_ARGUMENT;
}

/// Error: the path a namespace was pointed at holds no Vault.
#[derive(Grouped)]
pub struct ErrorNotAVault {
    /// The path that holds no configuration.
    path: String,
}

impl ErrorNotAVault {
    /// The failure for `path`.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl Failure for ErrorNotAVault {
    fn name(&self) -> &'static str {
        "error_not_a_vault"
    }

    fn reason(&self) -> String {
        t!("error.not_a_vault", path = self.path).to_string()
    }
}

failure!(ErrorNotAVault);

#[renderer(buffer)]
pub fn render_error_not_a_vault(error: ErrorNotAVault, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("error.not_a_vault_help").trim()));
    ec.exit_code = EC_ERR_ARGUMENT;
}

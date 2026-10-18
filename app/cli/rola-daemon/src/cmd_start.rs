//! The daemon this program is: serve every namespace registered for the user, until stopped.
//!
//! It has no command node of its own. A run that names nothing has asked for the daemon, so the
//! fallback turns an empty run into this state; a run that names something else is a mistake and
//! says so.

use std::path::PathBuf;

use mingling::{
    Grouped,
    macros::{buffer, chain, r_eprintln, r_println, renderer},
    res::ResExitCode,
};
use rorolala_cli_setups::ResConcurrency;
use rorolala_daemon::{daemon_begin_with, exit, registry_path};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::{
    Next,
    errors::ErrorNoDataDir,
    exit_codes::{EC_ERR_DAEMON_CONFIG, EC_ERR_IDENTITY, EC_ERR_PORT},
    failure::failure,
    port::{PortError, ResPort},
};

/// The state a run that asked for the daemon starts in.
#[derive(Grouped)]
pub struct StateStart;

#[chain]
pub fn handle_start(_state: StateStart, port: &ResPort, concurrency: &ResConcurrency) -> Next {
    let port = match port.get() {
        Ok(port) => port,
        Err(error) => return ErrorPort { error }.into(),
    };

    let Some(registry) = registry_path() else {
        return ErrorNoDataDir.into();
    };

    // How much is carried at once is the run's to name; what it leaves out is the number a
    // machine-wide daemon serves with.
    let max_concurrency = concurrency
        .explicit()
        .then(|| u16::try_from(**concurrency).unwrap_or(u16::MAX));

    let stopped = daemon_begin_with(&registry, port, max_concurrency);

    match stopped.code() {
        exit::NO_IDENTITY => ErrorNoIdentity.into(),
        exit::REGISTRY_UNREADABLE => ErrorRegistryUnreadable { path: registry }.into(),
        exit::PORT_TAKEN => ErrorPortTaken { port }.into(),
        _ => ResultStopped.into(),
    }
}

/// Error: what was named after `--port`, or by the environment, is not a port.
#[derive(Grouped)]
pub struct ErrorPort {
    /// What was wrong with it.
    error: PortError,
}

impl Failure for ErrorPort {
    fn name(&self) -> &'static str {
        "error_port"
    }

    fn reason(&self) -> String {
        match self.error {
            PortError::NotANumber => t!("start.err_port_not_a_number").to_string(),
            PortError::Zero => t!("start.err_port_zero").to_string(),
        }
    }
}

failure!(ErrorPort);

#[renderer(buffer)]
pub fn render_error_port(error: ErrorPort, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("start.err_port_help").trim()));
    ec.exit_code = EC_ERR_PORT;
}

/// Error: the daemon could not find an account to prove itself with.
#[derive(Grouped)]
pub struct ErrorNoIdentity;

impl Failure for ErrorNoIdentity {
    fn name(&self) -> &'static str {
        "error_no_identity"
    }

    fn reason(&self) -> String {
        t!("start.err_no_identity").to_string()
    }
}

failure!(ErrorNoIdentity);

#[renderer(buffer)]
pub fn render_error_no_identity(_: ErrorNoIdentity, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("start.err_no_identity").trim()));
    r_eprintln!("{}", help_line!(t!("start.err_no_identity_help").trim()));
    ec.exit_code = EC_ERR_IDENTITY;
}

/// Error: the table of namespaces could not be read.
#[derive(Grouped)]
pub struct ErrorRegistryUnreadable {
    /// The file that could not be read.
    path: PathBuf,
}

impl Failure for ErrorRegistryUnreadable {
    fn name(&self) -> &'static str {
        "error_registry_unreadable"
    }

    fn reason(&self) -> String {
        t!("start.err_registry", path = self.path.display().to_string()).to_string()
    }
}

failure!(ErrorRegistryUnreadable);

#[renderer(buffer)]
pub fn render_error_registry_unreadable(error: ErrorRegistryUnreadable, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("start.err_registry_help").trim()));
    ec.exit_code = EC_ERR_DAEMON_CONFIG;
}

/// Error: the port is already taken.
#[derive(Grouped)]
pub struct ErrorPortTaken {
    /// The port that could not be bound.
    port: u16,
}

impl Failure for ErrorPortTaken {
    fn name(&self) -> &'static str {
        "error_port_taken"
    }

    fn reason(&self) -> String {
        t!("start.err_port_taken", port = self.port).to_string()
    }
}

failure!(ErrorPortTaken);

#[renderer(buffer)]
pub fn render_error_port_taken(error: ErrorPortTaken, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()).trim());
    r_eprintln!("{}", help_line!(t!("start.err_port_taken_help").trim()));
    ec.exit_code = EC_ERR_PORT;
}

/// Result: the daemon stopped listening.
#[derive(Grouped)]
pub struct ResultStopped;

#[renderer(buffer)]
pub fn render_result_stopped(_: ResultStopped) {
    r_println!("{}", t!("start.result_stopped").trim());
}

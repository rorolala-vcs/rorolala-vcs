use std::path::PathBuf;

use mingling::{
    Grouped, LazyRes,
    macros::{buffer, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResVaultConfig};
use rorolala_daemon::daemon_begin;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate;
use rust_i18n::t;

use crate::{
    Next,
    exit_codes::{EC_ERR_DAEMON_CONFIG, EC_HELP, EC_NOT_EXIST},
};

#[help(buffer)]
pub fn help_listen(_: EntryListen, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("listen.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryListen)]
pub fn desc_listen() -> Description {
    t!("listen.cmd_listen_description").to_string().into()
}

#[command]
pub fn listen(vault: &mut LazyRes<ResVault>, config: &mut LazyRes<ResVaultConfig>) -> Next {
    let Some(vault) = vault.get_ref().as_ref() else {
        return ErrorVaultNotExist.into();
    };

    // The configuration is a resource too, so it is read the same way the Vault is, and
    // written back once the daemon is done with it.
    let config = match config.get_ref() {
        ResVaultConfig::Read { config, .. } => config,
        ResVaultConfig::Absent => return ErrorVaultNotExist.into(),
        ResVaultConfig::Unread { path, reason } => {
            return ErrorConfigUnreadable {
                path: path.clone(),
                reason: reason.clone(),
            }
            .into();
        }
    };

    let _exit = daemon_begin(vault.get_root(), config);
    ResultStopped.into()
}

/// Error: no Vault was located to listen for.
#[derive(Grouped)]
pub struct ErrorVaultNotExist;

#[renderer(buffer)]
pub fn render_error_vault_not_exist(_: ErrorVaultNotExist, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("listen.err_vault_not_exist")).trim());
    r_eprintln!(
        "{}",
        help_line!(t!("listen.err_vault_not_exist_help")).trim()
    );
    ec.exit_code = EC_NOT_EXIST;
}

/// Error: the Vault configuration could not be read.
#[derive(Grouped)]
pub struct ErrorConfigUnreadable {
    /// The configuration file that could not be read.
    path: PathBuf,
    /// Why the configuration could not be read.
    reason: String,
}

#[renderer(buffer)]
pub fn render_error_config_unreadable(err: ErrorConfigUnreadable, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!(
            "listen.err_config_unreadable",
            path = err.path.display().to_string(),
            reason = err.reason
        ))
        .trim()
    );
    r_eprintln!(
        "{}",
        help_line!(t!("listen.err_config_unreadable_help")).trim()
    );
    ec.exit_code = EC_ERR_DAEMON_CONFIG;
}

/// Result: the daemon stopped listening.
#[derive(Grouped)]
pub struct ResultStopped;

#[renderer(buffer)]
pub fn render_result_stopped(_: ResultStopped) {
    r_println!("{}", t!("listen.result_stopped").trim());
}

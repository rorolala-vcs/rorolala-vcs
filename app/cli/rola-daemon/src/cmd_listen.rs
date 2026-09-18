use std::path::PathBuf;

use mingling::{
    Grouped, LazyRes,
    macros::{buffer, command, r_eprintln, r_println, renderer},
    res::ResExitCode,
};
use rorolala_cli_setups::ResVault;
use rorolala_daemon::daemon_begin;
use rorolala_utils_cli_theme::{err_line, help_line};
use rorolala_utils_configure::Configure;
use rorolala_utils_location::Locate;
use rorolala_vault::{CONFIG_PATH, Config};
use rust_i18n::t;

use crate::{
    Next,
    exit_codes::{EC_ERR_DAEMON_CONFIG, EC_NOT_EXIST},
};

#[command]
pub fn listen(vault: &mut LazyRes<ResVault>) -> Next {
    let Some(vault) = vault.get_ref().as_ref() else {
        return ErrorVaultNotExist.into();
    };

    let config_path = vault.get_root().join(CONFIG_PATH);
    let config = match Config::read_from(&config_path) {
        Ok(config) => config,
        Err(error) => {
            return ErrorConfigUnreadable {
                path: config_path,
                reason: error.to_string(),
            }
            .into();
        }
    };

    let _exit = daemon_begin(vault.get_root(), &config);
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

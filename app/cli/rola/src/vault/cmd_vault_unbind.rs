//! The `rola vault unbind` command: let a Workspace's name for a Vault go.

use mingling::{
    Grouped, LazyRes, ShellContext, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspaceConfig;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::cmd_create::ErrorWorkspaceNotExist;
use crate::error::{ErrorConfigUnreadable, ErrorVaultNameMissing, ErrorVaultNotBound};
use crate::exit_codes::EC_HELP;

#[help(buffer)]
pub fn help_vault_unbind(_: EntryVaultUnbind, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_unbind.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultUnbind)]
pub fn desc_vault_unbind() -> Description {
    t!("vault_unbind.cmd_vault_unbind_description")
        .to_string()
        .into()
}

/// Lets a name go, so it no longer means a Vault.
///
/// A name that is not bound is not something to let go, so it is reported rather than
/// passed over in silence. A name that was the one reached for by default goes along with
/// it: a default that named nothing would reach nowhere.
#[command(node = "vault.unbind")]
pub fn vault_unbind(args: EntryVaultUnbind) -> Next {
    let name = match args
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .to_result()
    {
        Ok(name) => name,
        Err(next) => return next,
    };

    StateVaultUnbind::from(name).into()
}

/// The state of letting a name go.
#[derive(Grouped, Wrap)]
pub struct StateVaultUnbind {
    /// The name that is let go.
    name: String,
}

#[chain]
pub fn handle_vault_unbind(
    state: StateVaultUnbind,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
    let StateVaultUnbind { name } = state;

    match config.get_mut() {
        state @ ResWorkspaceConfig::Read { .. } => {
            // UNWRAP: the arm this is in shows there is a configuration to change.
            let workspace = state.config_mut().unwrap();
            match workspace.vaults_mut().unbind(&name) {
                Some(address) => {
                    let default_config = workspace.default_config_mut();
                    let cleared_default = default_config.vault() == Some(name.as_str());
                    if cleared_default {
                        let _ = default_config.clear_vault();
                    }

                    ResultVaultUnbound {
                        name,
                        address,
                        cleared_default,
                    }
                    .into()
                }
                None => ErrorVaultNotBound { name }.into(),
            }
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
    }
}

/// Completes what `rola vault unbind` can be given next.
///
/// A name that is bound is one the command can act on, so those are what is offered.
#[completion(EntryVaultUnbind)]
pub fn complete_vault_unbind(
    ctx: ShellContext,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Suggest {
    if ctx.current_word.starts_with('-') {
        return suggest!();
    }

    let Some(config) = config.get_ref().config() else {
        return suggest!();
    };

    let mut names: Vec<String> = config.vaults().names().cloned().collect();
    names.sort();
    names.retain(|name| name.starts_with(&ctx.current_word));

    suggest! { names }
}

/// Result: a name was let go.
#[derive(Grouped)]
pub struct ResultVaultUnbound {
    /// The name that was let go.
    name: String,
    /// The address it had been bound to.
    address: String,
    /// Whether it was also the one reached for by default.
    cleared_default: bool,
}

#[renderer(buffer)]
pub fn render_result_vault_unbound(result: ResultVaultUnbound) {
    r_println!(
        "{}",
        t!(
            "vault_unbind.result_unbound",
            name = result.name,
            address = result.address
        )
        .trim()
    );

    if result.cleared_default {
        r_println!("{}", t!("vault_set_default.result_cleared").trim());
    }
}

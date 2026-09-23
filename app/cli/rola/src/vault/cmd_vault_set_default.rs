//! The `rola vault set-default` command: choose the Vault a Workspace reaches for.

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
pub fn help_vault_set_default(_: EntryVaultSetDefault, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_set_default.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultSetDefault)]
pub fn desc_vault_set_default() -> Description {
    t!("vault_set_default.cmd_vault_set_default_description")
        .to_string()
        .into()
}

/// Chooses the Vault the Workspace reaches for when nothing else names one.
///
/// A default is what commands that would otherwise ask which Vault to reach for fall back
/// on, so the name has to be one the Workspace knows: a default that named nothing would be
/// a default that reached nowhere. Naming the one already chosen is not an error, only a
/// choice made again.
#[command(node = "vault.set-default")]
pub fn vault_set_default(args: EntryVaultSetDefault) -> Next {
    let name = match args
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .to_result()
    {
        Ok(name) => name,
        Err(next) => return next,
    };

    StateVaultSetDefault::from(name).into()
}

/// The state of choosing the Vault the Workspace reaches for.
#[derive(Grouped, Wrap)]
pub struct StateVaultSetDefault {
    /// The name that is reached for.
    name: String,
}

#[chain]
pub fn handle_vault_set_default(
    state: StateVaultSetDefault,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
    let StateVaultSetDefault { name } = state;

    match config.get_mut() {
        state @ ResWorkspaceConfig::Read { .. } => {
            // UNWRAP: the arm this is in shows there is a configuration to change.
            let workspace = state.config_mut().unwrap();
            if !workspace.vaults().contains(&name) {
                return ErrorVaultNotBound { name }.into();
            }

            let replaced = workspace.default_config_mut().set_vault(name.clone());
            ResultVaultDefaultSet { name, replaced }.into()
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
    }
}

/// Completes what `rola vault set-default` can be given next.
///
/// Only a name that is bound can be reached for, so those are what is offered, the way
/// `vault unbind` offers them.
#[completion(EntryVaultSetDefault)]
pub fn complete_vault_set_default(
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

/// Result: a name was chosen to be reached for.
#[derive(Grouped)]
pub struct ResultVaultDefaultSet {
    /// The name that is reached for now.
    name: String,
    /// The name that was reached for before, if one was.
    replaced: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_vault_default_set(result: ResultVaultDefaultSet) {
    let rendered = if let Some(previous) = result.replaced {
        t!(
            "vault_set_default.result_changed",
            name = result.name,
            previous = previous
        )
    } else {
        t!("vault_set_default.result_chosen", name = result.name)
    };

    r_println!("{}", rendered.trim());
}

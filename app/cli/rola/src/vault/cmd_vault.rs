//! The `rola vault` namespace: which Vaults this Workspace knows, and by what names.
//!
//! A name is the Workspace's own shorthand for an address, and it lives in the Workspace's
//! configuration. `vault` itself lists them; binding a name, letting one go, choosing the one
//! reached for by default, and reaching one to speak to are its subcommands, each with a file of
//! its own beside this one.
//!
//! The configuration is a resource, so a change a subcommand makes is written back once the
//! program is done with it.

use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{buffer, chain, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspaceConfig;
use rorolala_utils_cli_theme::trd;
use rorolala_workspace::Config as WorkspaceConfig;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::cmd_create::ErrorWorkspaceNotExist;
use crate::error::ErrorConfigUnreadable;
use crate::exit_codes::EC_HELP;

#[help(buffer)]
pub fn help_vault(_: EntryVault, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVault)]
pub fn desc_vault() -> Description {
    t!("vault.cmd_vault_description").to_string().into()
}

/// Lists the Vaults this Workspace knows.
///
/// Each one is printed as the name it is known by and the address it answers at, in name
/// order, and the one the Workspace reaches for by default is marked. A Workspace that knows
/// none prints nothing about any, and one that cannot be found beside the current directory is
/// an error, since there is nothing to list from.
#[command(node = "vault")]
pub fn vault() -> StateVaultList {
    StateVaultList
}

/// The state a listing of the Workspace's Vaults starts in.
///
/// A listing names nothing: there is one list to read, and it is the Workspace's own.
#[derive(Grouped)]
pub struct StateVaultList;

#[chain]
pub fn handle_vault_list(_state: StateVaultList, config: &mut LazyRes<ResWorkspaceConfig>) -> Next {
    match config.get_ref() {
        ResWorkspaceConfig::Read { config, .. } => ResultVaults::of(config).into(),
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
    }
}

/// Result: the Workspace's Vaults were listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVaults {
    /// Each Vault, by name and address, in name order.
    vaults: Vec<(String, String)>,
    /// The Vault the Workspace reaches for, if one has been chosen.
    current: Option<String>,
}

impl ResultVaults {
    /// The Vaults of `config`, in name order, and the one it reaches for.
    fn of(config: &WorkspaceConfig) -> Self {
        let mut vaults: Vec<(String, String)> = config
            .vaults()
            .iter()
            .map(|(name, address)| (name.clone(), address.clone()))
            .collect();
        vaults.sort_by(|left, right| left.0.cmp(&right.0));

        Self {
            vaults,
            current: config.default_config().vault().map(str::to_string),
        }
    }
}

#[renderer(buffer)]
pub fn render_result_vaults(result: ResultVaults) {
    if result.vaults.is_empty() {
        r_println!("{}", t!("vault.result_none").trim());
    } else {
        let width = result
            .vaults
            .iter()
            .map(|(name, _)| name.chars().count())
            .max()
            .unwrap_or_default();

        for (name, address) in result.vaults {
            let vault = format!("{name:<width$}  {address}");
            if result.current.as_deref() == Some(name.as_str()) {
                r_println!("{}", t!("vault.result_current", vault = vault).trim());
            } else {
                r_println!("{vault}");
            }
        }
    }
}

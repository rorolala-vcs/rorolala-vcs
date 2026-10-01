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
    Grouped, LazyRes, StructuralData, Suggest,
    macros::{
        buffer, chain, command, completion, empty_result, help, metadata, r_eprintln, r_print,
        r_println, renderer, suggest,
    },
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspaceConfig;
use rorolala_utils_cli_theme::{err_line, trd};
use rorolala_workspace::Config as WorkspaceConfig;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::cmd_create::ErrorWorkspaceNotExist;
use crate::error::ErrorConfigUnreadable;
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::format::ResFormat;

/// How a listing of the Vaults is drawn when no template is named.
///
/// One name and address a line. Each Vault also carries whether it is the one reached for,
/// `is_default`, in the data a template or `--json` reads.
const DEFAULT_VAULT_LS_FORMAT: &str = "{{ vaults.name }}  {{ vaults.address }}";

/// How the default Vault is drawn when no template is named.
const DEFAULT_VAULT_DEFAULT_FORMAT: &str = "{{ name }}";

#[help(buffer)]
pub fn help_vault(_: EntryVault, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVault)]
pub fn desc_vault() -> Description {
    t!("vault.cmd_vault_description").to_string().into()
}

/// Completes what `rola vault` can be given next.
///
/// The namespace names nothing of its own and its subcommands are put to the line by the
/// dispatcher, so there is nothing here to offer.
#[completion(EntryVault)]
pub fn complete_vault() -> Suggest {
    suggest!()
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

#[help(buffer)]
pub fn help_vault_ls(_: EntryVaultLs, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_ls.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultLs)]
pub fn desc_vault_ls() -> Description {
    t!("vault_ls.description").to_string().into()
}

/// Completes what `rola vault ls` can be given next.
///
/// The listing names nothing, so there is nothing to offer.
#[completion(EntryVaultLs)]
pub fn complete_vault_ls() -> Suggest {
    suggest!()
}

/// Lists the Vaults this Workspace knows, each with whether it is the one reached for
///
/// The same names `rola vault` lists, drawn through a template the way every query is: the default
/// names each one and its address, and each carries `is_default` — whether it is the Vault the
/// Workspace reaches for — so a run that reads the output as a program can tell which one is chosen
/// without a marker a person would read.
#[command(node = "vault.ls")]
pub fn vault_ls(format: &mut ResFormat) -> StateVaultLs {
    format.default_template(DEFAULT_VAULT_LS_FORMAT);
    StateVaultLs
}

/// The state a listing of the Vaults starts in.
#[derive(Grouped)]
pub struct StateVaultLs;

#[chain]
pub fn handle_vault_ls(
    _state: StateVaultLs,
    config: &mut LazyRes<ResWorkspaceConfig>,
    format: &mut ResFormat,
) -> Next {
    match config.get_ref() {
        ResWorkspaceConfig::Read { config, .. } => {
            let default = config.default_config().vault().map(str::to_string);
            let mut vaults: Vec<VaultItem> = config
                .vaults()
                .iter()
                .map(|(name, address)| VaultItem {
                    name: name.clone(),
                    address: address.clone(),
                    is_default: default.as_deref() == Some(name.as_str()),
                })
                .collect();
            vaults.sort_by(|left, right| left.name.cmp(&right.name));

            format.set(
                "vaults",
                vaults
                    .iter()
                    .map(|vault| serde_json::json!(vault))
                    .collect(),
            );

            ResultVaultLs { vaults }.into()
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
    }
}

/// One Vault, as `vault ls` shows it.
#[derive(Serialize)]
pub struct VaultItem {
    /// The name the Workspace knows it by.
    name: String,
    /// The address it answers at.
    address: String,
    /// Whether it is the Vault the Workspace reaches for.
    is_default: bool,
}

/// Result: the Workspace's Vaults were listed, each with whether it is the one reached for.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVaultLs {
    /// Each Vault, by name and address, in name order.
    vaults: Vec<VaultItem>,
}

#[renderer(buffer)]
pub fn render_result_vault_ls(result: ResultVaultLs, format: &ResFormat, ec: &mut ResExitCode) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!(
                    "{}",
                    err_line!(t!("format.err_format", reason = error).trim())
                );
                ec.exit_code = EC_ERR_FORMAT;
            }
        }
    } else {
        for vault in &result.vaults {
            r_println!("{}  {}", vault.name, vault.address);
        }
    }
}

#[help(buffer)]
pub fn help_vault_default(_: EntryVaultDefault, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_default.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultDefault)]
pub fn desc_vault_default() -> Description {
    t!("vault_default.description").to_string().into()
}

/// Completes what `rola vault default` can be given next.
///
/// The command names nothing, so there is nothing to offer.
#[completion(EntryVaultDefault)]
pub fn complete_vault_default() -> Suggest {
    suggest!()
}

/// Prints the Vault the Workspace reaches for
///
/// The name alone, so a run can read it without a sentence around it. Nothing is printed when no
/// Vault has been chosen, since there is none to name.
#[command(node = "vault.default")]
pub fn vault_default(format: &mut ResFormat) -> StateVaultDefault {
    format.default_template(DEFAULT_VAULT_DEFAULT_FORMAT);
    StateVaultDefault
}

/// The state printing the default Vault starts in.
#[derive(Grouped)]
pub struct StateVaultDefault;

#[chain]
pub fn handle_vault_default(
    _state: StateVaultDefault,
    config: &mut LazyRes<ResWorkspaceConfig>,
    format: &mut ResFormat,
) -> Next {
    match config.get_ref() {
        ResWorkspaceConfig::Read { config, .. } => {
            let Some(name) = config.default_config().vault() else {
                return empty_result!();
            };

            let name = name.to_string();
            format.set("name", vec![serde_json::json!(name)]);

            ResultVaultDefault { name }.into()
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
    }
}

/// Result: the Vault the Workspace reaches for was printed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVaultDefault {
    /// The Vault the Workspace reaches for.
    name: String,
}

#[renderer(buffer)]
pub fn render_result_vault_default(
    result: ResultVaultDefault,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!(
                    "{}",
                    err_line!(t!("format.err_format", reason = error).trim())
                );
                ec.exit_code = EC_ERR_FORMAT;
            }
        }
    } else {
        r_println!("{}", result.name);
    }
}

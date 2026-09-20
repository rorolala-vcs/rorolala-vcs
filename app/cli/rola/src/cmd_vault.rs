//! The `rola vault` commands: which Vaults this Workspace knows, and by what names.
//!
//! A name is the Workspace's own shorthand for an address, and it lives in the Workspace's
//! configuration. Binding is how one is set or changed, unbinding is how one is let go, and
//! naming no name lists them all. The configuration is a resource, so a change made here is
//! written back once the program is done with it.

use std::path::PathBuf;

use librorolala::protocol::parse_address;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, command, completion, help, metadata, r_eprintln, r_println, renderer, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspaceConfig;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_DEFAULT_PORT;
use rorolala_workspace::{Config as WorkspaceConfig, SocketAddress};
use rust_i18n::t;

use crate::Next;
use crate::address::ResAddressHistory;
use crate::cmd_create::ErrorWorkspaceNotExist;
use crate::exit_codes::{
    EC_ERR_VAULT_ARGUMENT, EC_ERR_VAULT_CONFIG, EC_ERR_VAULT_NOT_BOUND, EC_HELP,
};

/// The last word of the two `rola vault bind` is dispatched by.
///
/// The argument being completed is counted from the words after it, so this is what tells
/// an address being typed from a name being typed.
const BIND_NODE_TAIL: &str = "bind";

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
/// order. A Workspace that knows none prints nothing about any, and one that cannot be
/// found beside the current directory is an error, since there is nothing to list from.
#[command(node = "vault")]
pub fn vault(config: &mut LazyRes<ResWorkspaceConfig>) -> Next {
    match config.get_ref() {
        ResWorkspaceConfig::Read { config, .. } => ResultVaults::of(config).into(),
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => ErrorConfigUnreadable {
            path: path.clone(),
            reason: reason.clone(),
        }
        .into(),
    }
}

#[help(buffer)]
pub fn help_vault_bind(_: EntryVaultBind, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_bind.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultBind)]
pub fn desc_vault_bind() -> Description {
    t!("vault_bind.cmd_vault_bind_description")
        .to_string()
        .into()
}

/// Binds a name to a Vault address, or changes what the name means.
///
/// The name is the Workspace's own, so the same Vault may be `origin` here and something
/// else elsewhere. Naming one that is already bound is how its address is changed. The
/// address is remembered for completion, whether or not the name had been bound before.
#[command(node = "vault.bind")]
pub fn vault_bind(
    args: EntryVaultBind,
    config: &mut LazyRes<ResWorkspaceConfig>,
    history: &mut LazyRes<ResAddressHistory>,
) -> Next {
    let pair = args
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .pick_or_route(&arg![String], || ErrorVaultAddressMissing.into())
        .to_result();
    let (name, address) = match pair {
        Ok(pair) => pair,
        Err(next) => return next,
    };

    let Ok(address) = parse_address(&address, VAULT_DEFAULT_PORT) else {
        return ErrorVaultAddressInvalid { address }.into();
    };

    match config.get_mut() {
        state @ ResWorkspaceConfig::Read { .. } => {
            // UNWRAP: the arm this is in shows there is a configuration to change.
            let workspace = state.config_mut().unwrap();
            let replaced = workspace.vaults_mut().bind(name.clone(), address);
            history.get_mut().remember(address.to_string());
            ResultVaultBound {
                name,
                address,
                replaced,
            }
            .into()
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => ErrorConfigUnreadable {
            path: path.clone(),
            reason: reason.clone(),
        }
        .into(),
    }
}

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
/// passed over in silence.
#[command(node = "vault.unbind")]
pub fn vault_unbind(args: EntryVaultUnbind, config: &mut LazyRes<ResWorkspaceConfig>) -> Next {
    let name = match args
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .to_result()
    {
        Ok(name) => name,
        Err(next) => return next,
    };

    match config.get_mut() {
        state @ ResWorkspaceConfig::Read { .. } => {
            // UNWRAP: the arm this is in shows there is a configuration to change.
            let workspace = state.config_mut().unwrap();
            match workspace.vaults_mut().unbind(&name) {
                Some(address) => ResultVaultUnbound { name, address }.into(),
                None => ErrorVaultNotBound { name }.into(),
            }
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => ErrorConfigUnreadable {
            path: path.clone(),
            reason: reason.clone(),
        }
        .into(),
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

/// Completes what `rola vault bind` can be given next.
///
/// Only the address is completed, from the addresses reached before; the name is
/// deliberately left alone, since a name is the caller's to choose and nothing here can
/// know what it should be.
#[completion(EntryVaultBind)]
pub fn complete_vault_bind(ctx: ShellContext, history: &mut LazyRes<ResAddressHistory>) -> Suggest {
    if ctx.current_word.starts_with('-') || positional(&ctx) != 1 {
        return suggest!();
    }

    let mut addresses: Vec<String> = history.get_ref().iter().map(str::to_string).collect();
    addresses.retain(|address| address.starts_with(&ctx.current_word));

    suggest! { addresses }
}

/// Which positional argument the word being completed is, counting from zero.
///
/// The words after the command node are the positional arguments. A word that has already
/// been typed fills its position; the word being completed fills the position it is about
/// to, so it is the words after the node that are counted, less the one in progress.
fn positional(ctx: &ShellContext) -> usize {
    let after_node = ctx
        .all_words
        .iter()
        .position(|word| word == BIND_NODE_TAIL)
        .map_or(0, |index| index + 1);

    let typed = ctx.all_words.len().saturating_sub(after_node);

    typed.saturating_sub(usize::from(!ctx.current_word.is_empty()))
}

/// Result: the Workspace's Vaults were listed.
#[derive(Grouped)]
pub struct ResultVaults {
    /// Each Vault, by name and address, in name order.
    vaults: Vec<(String, SocketAddress)>,
}

impl ResultVaults {
    /// The Vaults of `config`, in name order.
    fn of(config: &WorkspaceConfig) -> Self {
        let mut vaults: Vec<(String, SocketAddress)> = config
            .vaults()
            .iter()
            .map(|(name, address)| (name.clone(), *address))
            .collect();
        vaults.sort_by(|left, right| left.0.cmp(&right.0));

        Self { vaults }
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
            r_println!("{name:<width$}  {address}");
        }
    }
}

/// Result: a name was bound to an address.
#[derive(Grouped)]
pub struct ResultVaultBound {
    /// The name that was bound.
    name: String,
    /// The address it is bound to now.
    address: SocketAddress,
    /// The address it was bound to before, if it was bound at all.
    replaced: Option<SocketAddress>,
}

#[renderer(buffer)]
pub fn render_result_vault_bound(result: ResultVaultBound) {
    let rendered = if let Some(previous) = result.replaced {
        t!(
            "vault_bind.result_changed",
            name = result.name,
            previous = previous.to_string(),
            address = result.address.to_string()
        )
    } else {
        t!(
            "vault_bind.result_bound",
            name = result.name,
            address = result.address.to_string()
        )
    };

    r_println!("{}", rendered.trim());
}

/// Result: a name was let go.
#[derive(Grouped)]
pub struct ResultVaultUnbound {
    /// The name that was let go.
    name: String,
    /// The address it had been bound to.
    address: SocketAddress,
}

#[renderer(buffer)]
pub fn render_result_vault_unbound(result: ResultVaultUnbound) {
    r_println!(
        "{}",
        t!(
            "vault_unbind.result_unbound",
            name = result.name,
            address = result.address.to_string()
        )
        .trim()
    );
}

/// Error: the Workspace's configuration could not be read.
#[derive(Grouped)]
pub struct ErrorConfigUnreadable {
    /// The file that could not be read.
    path: PathBuf,
    /// Why it could not be read.
    reason: String,
}

#[renderer(buffer)]
pub fn render_error_config_unreadable(error: ErrorConfigUnreadable, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(
            t!(
                "vault.err_config_unreadable",
                path = error.path.display().to_string()
            )
            .trim()
        )
    );
    r_eprintln!(
        "{}",
        help_line!(t!("vault.err_config_unreadable_help", reason = error.reason).trim())
    );
    ec.exit_code = EC_ERR_VAULT_CONFIG;
}

/// Error: the name a `rola vault bind` or `rola vault unbind` was given is missing.
#[derive(Grouped)]
pub struct ErrorVaultNameMissing;

#[renderer(buffer)]
pub fn render_error_vault_name_missing(_: ErrorVaultNameMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("vault_bind.err_name_missing").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("vault_bind.err_name_missing_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}

/// Error: the address a `rola vault bind` was given is missing.
#[derive(Grouped)]
pub struct ErrorVaultAddressMissing;

#[renderer(buffer)]
pub fn render_error_vault_address_missing(_: ErrorVaultAddressMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("vault_bind.err_address_missing").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("vault_bind.err_address_missing_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}

/// Error: the address a `rola vault bind` was given would not read as one.
#[derive(Grouped)]
pub struct ErrorVaultAddressInvalid {
    /// The address that would not read.
    address: String,
}

#[renderer(buffer)]
pub fn render_error_vault_address_invalid(error: ErrorVaultAddressInvalid, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("vault_bind.err_address_invalid", address = error.address).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("vault_bind.err_address_invalid_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}

/// Error: the name a `rola vault unbind` was given is not bound to anything.
#[derive(Grouped)]
pub struct ErrorVaultNotBound {
    /// The name that is not bound.
    name: String,
}

#[renderer(buffer)]
pub fn render_error_vault_not_bound(error: ErrorVaultNotBound, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("vault_unbind.err_not_bound", name = error.name).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("vault_unbind.err_not_bound_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_NOT_BOUND;
}

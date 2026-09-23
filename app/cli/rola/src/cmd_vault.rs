//! The `rola vault` commands: which Vaults this Workspace knows, and by what names.
//!
//! A name is the Workspace's own shorthand for an address, and it lives in the Workspace's
//! configuration. Binding is how one is set or changed, unbinding is how one is let go, and
//! naming no name lists them all. The configuration is a resource, so a change made here is
//! written back once the program is done with it.

use librorolala::protocol::VaultAddress;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, command, completion, help, metadata, r_eprintln, r_println, renderer, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspaceConfig;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_workspace::Config as WorkspaceConfig;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::address::ResAddressHistory;
use crate::cmd_create::ErrorWorkspaceNotExist;
use crate::error::ErrorConfigUnreadable;
use crate::exit_codes::{EC_ERR_VAULT_ARGUMENT, EC_ERR_VAULT_NOT_BOUND, EC_HELP};
use crate::failure::failure;

/// The last word of the two `rola vault bind` is dispatched by.
///
/// The argument being completed is counted from the words after it, so this is what tells
/// an address being typed from a name being typed.
const BIND_NODE_TAIL: &str = "bind";

/// The flags `rola vault bind` takes.
#[derive(Pickable)]
struct VaultBindFlags {
    /// Also make the name the one the Workspace reaches for.
    #[arg(long)]
    set_default: Flag,
}

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
pub fn vault(config: &mut LazyRes<ResWorkspaceConfig>) -> Next {
    match config.get_ref() {
        ResWorkspaceConfig::Read { config, .. } => ResultVaults::of(config).into(),
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
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
///
/// With `--set-default`, binding also [chooses](vault_set_default) the name to be reached
/// for, which saves saying so twice.
#[command(node = "vault.bind")]
pub fn vault_bind(
    args: EntryVaultBind,
    config: &mut LazyRes<ResWorkspaceConfig>,
    history: &mut LazyRes<ResAddressHistory>,
) -> Next {
    let picked = args
        .pick(&arg![VaultBindFlags])
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .pick_or_route(&arg![String], || ErrorVaultAddressMissing.into())
        .to_result();
    let (flags, name, address) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    let Ok(address) = VaultAddress::parse(&address) else {
        return ErrorVaultAddressInvalid { address }.into();
    };

    // What is written down is the link the address adds up to, so a name reached for as
    // `10.0.0.1` and one reached for as `rola://10.0.0.1/` are written the same way, and
    // reading one back later is reading the same thing either time.
    let address = address.to_string();

    match config.get_mut() {
        state @ ResWorkspaceConfig::Read { .. } => {
            // UNWRAP: the arm this is in shows there is a configuration to change.
            let workspace = state.config_mut().unwrap();
            history.get_mut().remember(address.clone());
            let replaced = workspace.vaults_mut().bind(name.clone(), address.clone());

            // The name is bound by now, so choosing it is choosing one that can be reached
            // for. Picking a `Flag` cannot fail, so this is `Active` only when it was written.
            let made_default = matches!(flags.set_default, Flag::Active);
            if made_default {
                let _ = workspace.default_config_mut().set_vault(name.clone());
            }

            ResultVaultBound {
                name,
                address,
                replaced,
                made_default,
            }
            .into()
        }
        ResWorkspaceConfig::Absent => ErrorWorkspaceNotExist.into(),
        ResWorkspaceConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
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
/// passed over in silence. A name that was the one [reached for](vault_set_default) by
/// default goes along with it: a default that named nothing would reach nowhere.
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
pub fn vault_set_default(
    args: EntryVaultSetDefault,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Next {
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

/// Result: a name was bound to an address.
#[derive(Grouped)]
pub struct ResultVaultBound {
    /// The name that was bound.
    name: String,
    /// The address it is bound to now.
    address: String,
    /// The address it was bound to before, if it was bound at all.
    replaced: Option<String>,
    /// Whether the name was also chosen to be reached for.
    made_default: bool,
}

#[renderer(buffer)]
pub fn render_result_vault_bound(result: ResultVaultBound) {
    let rendered = if let Some(previous) = result.replaced {
        t!(
            "vault_bind.result_changed",
            name = result.name,
            previous = previous,
            address = result.address
        )
    } else {
        t!(
            "vault_bind.result_bound",
            name = result.name,
            address = result.address
        )
    };

    r_println!("{}", rendered.trim());

    if result.made_default {
        r_println!(
            "{}",
            t!("vault_set_default.result_chosen", name = result.name).trim()
        );
    }
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

/// Error: the name a `rola vault bind` or `rola vault unbind` was given is missing.
#[derive(Grouped)]
pub struct ErrorVaultNameMissing;

impl Failure for ErrorVaultNameMissing {
    fn name(&self) -> &'static str {
        "error_vault_name_missing"
    }

    fn reason(&self) -> String {
        t!("vault_bind.err_name_missing").trim().to_string()
    }
}

failure!(ErrorVaultNameMissing);

#[renderer(buffer)]
pub fn render_error_vault_name_missing(error: ErrorVaultNameMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("vault_bind.err_name_missing_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}

/// Error: the address a `rola vault bind` was given is missing.
#[derive(Grouped)]
pub struct ErrorVaultAddressMissing;

impl Failure for ErrorVaultAddressMissing {
    fn name(&self) -> &'static str {
        "error_vault_address_missing"
    }

    fn reason(&self) -> String {
        t!("vault_bind.err_address_missing").trim().to_string()
    }
}

failure!(ErrorVaultAddressMissing);

#[renderer(buffer)]
pub fn render_error_vault_address_missing(error: ErrorVaultAddressMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
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

impl Failure for ErrorVaultAddressInvalid {
    fn name(&self) -> &'static str {
        "error_vault_address_invalid"
    }

    fn reason(&self) -> String {
        t!("vault_bind.err_address_invalid", address = self.address)
            .trim()
            .to_string()
    }
}

failure!(ErrorVaultAddressInvalid);

#[renderer(buffer)]
pub fn render_error_vault_address_invalid(error: ErrorVaultAddressInvalid, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("vault_bind.err_address_invalid_help").trim())
    );
    ec.exit_code = EC_ERR_VAULT_ARGUMENT;
}

/// Error: the name a `rola vault` command was given is not bound to anything.
#[derive(Grouped)]
pub struct ErrorVaultNotBound {
    /// The name that is not bound.
    name: String,
}

impl Failure for ErrorVaultNotBound {
    fn name(&self) -> &'static str {
        "error_vault_not_bound"
    }

    fn reason(&self) -> String {
        t!("vault.err_not_bound", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorVaultNotBound);

#[renderer(buffer)]
pub fn render_error_vault_not_bound(error: ErrorVaultNotBound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("vault.err_not_bound_help").trim()));
    ec.exit_code = EC_ERR_VAULT_NOT_BOUND;
}

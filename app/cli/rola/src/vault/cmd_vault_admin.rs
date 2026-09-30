//! The `rola vault admin` commands: who may act as an administrator of a Vault.
//!
//! An administrator is named in the Vault's own configuration, under `[auth] admins`, so these
//! commands work on the Vault the run is inside rather than reaching for a remote one: the list is
//! the Vault's, and it is kept beside it. The first administrator is not named by a command — a
//! Vault with none has nobody who may name one — so a Vault writes the first one by hand, and
//! these commands work from there.
//!
//! The configuration is a resource, so a change is written back once the program is done with it.

use librorolala::vault::Config as VaultConfig;
use mingling::{
    Grouped, LazyRes, StructuralData, Wrap,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVaultConfig;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::cmd_create::ErrorVaultNotExist;
use crate::error::{
    ErrorConfigUnreadable, ErrorVaultAdminMissing, ErrorVaultAdminUnknown, ErrorVaultLastAdmin,
    ErrorVaultNameMissing, ErrorVaultNotAdmin,
};
use crate::exit_codes::EC_HELP;

#[help(buffer)]
pub fn help_vault_admin(_: EntryVaultAdmin, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_admin.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultAdmin)]
pub fn desc_vault_admin() -> Description {
    t!("vault_admin.description").to_string().into()
}

/// Lists the administrators of this Vault
#[command(node = "vault.admin")]
pub fn vault_admin() -> StateVaultAdmin {
    StateVaultAdmin
}

/// The state a listing of the administrators starts in.
#[derive(Grouped, Clone, Copy)]
pub struct StateVaultAdmin;

#[chain]
pub fn handle_vault_admin(_: StateVaultAdmin, config: &mut LazyRes<ResVaultConfig>) -> Next {
    match config.get_ref() {
        ResVaultConfig::Read { config, .. } => ResultVaultAdmins {
            admins: config.auth_config().admins().to_vec(),
        }
        .into(),
        ResVaultConfig::Absent => ErrorVaultNotExist.into(),
        ResVaultConfig::Unread { path, reason } => {
            ErrorConfigUnreadable::new(path.clone(), reason.clone()).into()
        }
    }
}

#[help(buffer)]
pub fn help_vault_admin_add(_: EntryVaultAdminAdd, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_admin.add_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultAdminAdd)]
pub fn desc_vault_admin_add() -> Description {
    t!("vault_admin.add_description").to_string().into()
}

/// Names a member an administrator of this Vault
///
/// `NAME` is a member of the Vault — the stem of a public key the Vault admits — and naming one
/// who is already named is not an error, only the same naming made again.
///
/// # Errors
///
/// Renders [`ErrorVaultNotExist`] when the run is not inside a Vault,
/// [`ErrorVaultNameMissing`] when no name was given, [`ErrorVaultAdminMissing`] when the Vault
/// names no administrator yet — the first one is written by hand — [`ErrorVaultNotAdmin`] when the
/// account this run acts as is not one, and [`ErrorConfigUnreadable`] when the configuration could
/// not be read.
#[command(node = "vault.admin.add", entry = EntryVaultAdminAdd)]
pub fn vault_admin_add(args: EntryVaultAdminAdd) -> Next {
    let name = match args
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .to_result()
    {
        Ok(name) => name,
        Err(next) => return next,
    };

    StateVaultAdminAdd::from(name).into()
}

/// The state of naming an administrator.
#[derive(Grouped, Wrap)]
pub struct StateVaultAdminAdd {
    /// The member to name.
    name: String,
}

#[chain(routeify)]
pub fn handle_vault_admin_add(
    state: StateVaultAdminAdd,
    config: &mut LazyRes<ResVaultConfig>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let acting = current.get_ref().must_bind()?;

    let vault = match writable(config, &acting) {
        Ok(vault) => vault,
        Err(next) => return next,
    };

    let named = vault.auth_config_mut().add_admin(state.name.clone());

    ResultVaultAdminAdded {
        name: state.name,
        named,
    }
    .into()
}

#[help(buffer)]
pub fn help_vault_admin_rm(_: EntryVaultAdminRm, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_admin.rm_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultAdminRm)]
pub fn desc_vault_admin_rm() -> Description {
    t!("vault_admin.rm_description").to_string().into()
}

/// Stops a member being an administrator of this Vault
///
/// The last administrator cannot be removed: a Vault left with none has no way back but the same
/// hand edit that made the first one.
///
/// # Errors
///
/// Renders what [`vault admin add`](crate::vault::cmd_vault_admin::vault_admin_add) does, the
/// not-an-administrator failure when the name is not one, and [`ErrorVaultLastAdmin`] when removing
/// it would leave the Vault with none.
#[command(node = "vault.admin.rm", entry = EntryVaultAdminRm)]
pub fn vault_admin_rm(args: EntryVaultAdminRm) -> Next {
    let name = match args
        .pick_or_route(&arg![String], || ErrorVaultNameMissing.into())
        .to_result()
    {
        Ok(name) => name,
        Err(next) => return next,
    };

    StateVaultAdminRm::from(name).into()
}

/// The state of un-naming an administrator.
#[derive(Grouped, Wrap)]
pub struct StateVaultAdminRm {
    /// The member to stop being an administrator.
    name: String,
}

#[chain(routeify)]
pub fn handle_vault_admin_rm(
    state: StateVaultAdminRm,
    config: &mut LazyRes<ResVaultConfig>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let acting = current.get_ref().must_bind()?;

    let vault = match writable(config, &acting) {
        Ok(vault) => vault,
        Err(next) => return next,
    };

    let auth = vault.auth_config();
    if !auth.is_admin(&state.name) {
        return ErrorVaultAdminUnknown { name: state.name }.into();
    }
    if auth.admins().len() == 1 {
        return ErrorVaultLastAdmin.into();
    }

    vault.auth_config_mut().remove_admin(&state.name);

    ResultVaultAdminRemoved { name: state.name }.into()
}

/// The Vault's configuration, once this run is allowed to change who is an administrator.
///
/// A Vault names its administrators itself, and a Vault with none has nobody who may name one: both
/// are said here rather than by each command, since both are about the same list.
///
/// # Errors
///
/// Returns [`ErrorVaultNotExist`] when the run is not inside a Vault,
/// [`ErrorConfigUnreadable`] when the configuration could not be read,
/// [`ErrorVaultAdminMissing`] when it names no administrator, and [`ErrorVaultNotAdmin`] when the
/// account this run acts as is not one.
fn writable<'a>(
    config: &'a mut LazyRes<ResVaultConfig>,
    acting: &str,
) -> Result<&'a mut VaultConfig, Next> {
    match config.get_mut() {
        resource @ ResVaultConfig::Read { .. } => {
            // UNWRAP: the arm this is in shows the configuration was read.
            let vault = resource.config_mut().unwrap();
            let auth = vault.auth_config();

            if auth.admins().is_empty() {
                return Err(ErrorVaultAdminMissing.into());
            }
            if !auth.is_admin(acting) {
                return Err(ErrorVaultNotAdmin {
                    name: acting.to_owned(),
                }
                .into());
            }

            Ok(vault)
        }
        ResVaultConfig::Absent => Err(ErrorVaultNotExist.into()),
        ResVaultConfig::Unread { path, reason } => {
            Err(ErrorConfigUnreadable::new(path.clone(), reason.clone()).into())
        }
    }
}

/// Result: the administrators were listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVaultAdmins {
    /// The members who may act as administrators, as they were named.
    admins: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_vault_admins(result: ResultVaultAdmins) {
    if result.admins.is_empty() {
        r_println!("{}", t!("vault_admin.result_none").trim());
    } else {
        for admin in &result.admins {
            r_println!("{admin}");
        }
    }
}

/// Result: a member was named an administrator, or was one already.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVaultAdminAdded {
    /// The member that was named.
    name: String,
    /// Whether it was named anew; naming one already named is not a change.
    named: bool,
}

#[renderer(buffer)]
pub fn render_result_vault_admin_added(result: ResultVaultAdminAdded) {
    let said = if result.named {
        t!("vault_admin.result_named", name = result.name)
    } else {
        t!("vault_admin.result_already", name = result.name)
    };

    r_println!("{}", said.trim());
}

/// Result: a member stopped being an administrator.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVaultAdminRemoved {
    /// The member that was unnamed.
    name: String,
}

#[renderer(buffer)]
pub fn render_result_vault_admin_removed(result: ResultVaultAdminRemoved) {
    r_println!(
        "{}",
        t!("vault_admin.result_removed", name = result.name).trim()
    );
}

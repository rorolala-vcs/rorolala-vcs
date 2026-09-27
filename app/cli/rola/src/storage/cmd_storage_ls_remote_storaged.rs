//! The `rola storage ls-remote-storaged` command: list the objects the other end's store holds.
//!
//! It is [`ls-storaged`](crate::storage::cmd_storage_ls_storaged) asked of the other end of an
//! exchange: where that command lists the store the run is inside, this one reaches the Vault and
//! lists what *its* store holds. What is printed is the same keys, one per line, so a line read here
//! is a hash to hand straight back.

use std::str::FromStr as _;

use librorolala::daemon::action_list_remote_async;
use librorolala::protocol::ActionError;
use librorolala::storage::Key;
use mingling::{
    Grouped, LazyRes, Wrap,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rorolala_utils_progress::Progress;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::EC_HELP;
use crate::keys::account_named;
use crate::storage::cmd_storage_ls_storaged::ResultLs;

#[help(buffer)]
pub fn help_storage_ls_remote_storaged(_: EntryStorageLsRemoteStoraged, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage_ls_remote_storaged.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorageLsRemoteStoraged)]
pub fn desc_storage_ls_remote_storaged() -> Description {
    t!("storage_ls_remote_storaged.description")
        .to_string()
        .into()
}

/// Lists the objects the store at the other end holds
///
/// `VAULT` names the Vault to reach, as [`storage sync-all`](crate::storage::cmd_storage_sync_all)
/// reads it: a name the Workspace has bound, or an ip and a port, with none naming the one the
/// Workspace reaches for by default. What is printed is what the Vault's store holds, one key per
/// line, the same keys `rola storage ls-storaged` prints of the store the run is inside.
///
/// Nothing is changed: the Vault is asked and answers, and no content moves.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries it
/// out: the run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]), the Workspace
/// has no Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or as one no scope
/// holds ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), or the exchange itself failed
/// ([`ActionError`]).
///
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "storage.ls-remote-storaged", entry = EntryStorageLsRemoteStoraged)]
pub fn storage_ls_remote_storaged(
    args: EntryStorageLsRemoteStoraged,
) -> StateStorageLsRemoteStoraged {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateStorageLsRemoteStoraged::from(named)
}

/// The state of listing the other end's store.
#[derive(Grouped, Wrap)]
pub struct StateStorageLsRemoteStoraged {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_storage_ls_remote_storaged(
    state: StateStorageLsRemoteStoraged,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let target = remote
        .get_ref()
        .vault_or_default(state.vault.unwrap_or_default())?;

    let name = current.get_ref().must_bind()?;
    let account = account_named(&name, Some(held), None)?;

    // Nothing is changed and nothing is watched, so the exchange is given no reader and the action
    // says nothing while it runs.
    let runtime = tokio::runtime::Runtime::new().map_err(|error| ActionError::Io(error.into()))?;
    let listing = runtime.block_on(action_list_remote_async(
        held,
        &account,
        target.to_string(),
        "storaged".to_owned(),
        Progress::silent(),
    ))?;

    ResultLs {
        keys: keys_of(&listing),
    }
    .into()
}

/// The keys a listing names, one a line.
pub fn keys_of(listing: &str) -> Vec<Key> {
    listing
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| Key::from_str(line).ok())
        .collect()
}

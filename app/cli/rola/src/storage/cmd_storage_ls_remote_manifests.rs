//! The `rola storage ls-remote-manifests` command: list the content the other end keeps cut.
//!
//! It is [`ls-manifests`](crate::storage::cmd_storage_ls_manifests) asked of the other end of an
//! exchange: where that command lists the manifests the store the run is inside keeps, this one
//! reaches the Vault and lists its own. What is printed is named `manifest:<digest>`, the same as
//! there, so a line read here is what [`storage extract-file`](crate::storage::cmd_storage_extract_file)
//! takes back.

use librorolala::daemon::action_list_remote_async;
use librorolala::protocol::ActionError;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResOffline, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rorolala_utils_progress::Progress;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{offer, positional, typing_flag, vault_names};
use crate::error::ErrorOffline;
use crate::exit_codes::EC_HELP;
use crate::format::ResFormat;
use crate::keys::account_named;
use crate::storage::cmd_storage_ls_manifests::{DEFAULT_FORMAT, ResultManifests};
use crate::storage::cmd_storage_ls_remote_storaged::keys_of;

#[help(buffer)]
pub fn help_storage_ls_remote_manifests(_: EntryStorageLsRemoteManifests, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage_ls_remote_manifests.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorageLsRemoteManifests)]
pub fn desc_storage_ls_remote_manifests() -> Description {
    t!("storage_ls_remote_manifests.description")
        .to_string()
        .into()
}

/// Completes what `rola storage ls-remote-manifests` can be given next.
///
/// The one word names the Vault to reach, so the names the Workspace has bound are what is offered;
/// naming none reaches for the one it reaches for.
#[completion(EntryStorageLsRemoteManifests)]
pub fn complete_storage_ls_remote_manifests(
    ctx: ShellContext,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "ls-remote-manifests") != 0 {
        return suggest!();
    }

    offer(&ctx, vault_names(remote.get_ref()))
}

/// Lists the content the store at the other end keeps as a manifest of chunks
///
/// `VAULT` names the Vault to reach, as [`storage sync-all`](crate::storage::cmd_storage_sync_all)
/// reads it: a name the Workspace has bound, or an ip and a port, with none naming the one the
/// Workspace reaches for by default. What is printed is named `manifest:<digest>`, the way
/// [`rola storage ls-manifests`](crate::storage::cmd_storage_ls_manifests) names it, so a line read
/// here can be handed straight to `rola storage extract-file`.
///
/// Nothing is changed: the Vault is asked and answers, and no content moves.
///
/// # Errors
///
/// As [`storage ls-remote-storaged`](crate::storage::cmd_storage_ls_remote_storaged).
#[command(node = "storage.ls-remote-manifests", entry = EntryStorageLsRemoteManifests)]
pub fn storage_ls_remote_manifests(
    args: EntryStorageLsRemoteManifests,
    format: &mut ResFormat,
) -> StateStorageLsRemoteManifests {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    format.default_template(DEFAULT_FORMAT);
    StateStorageLsRemoteManifests::from(named)
}

/// The state of listing the other end's manifests.
#[derive(Grouped, Wrap)]
pub struct StateStorageLsRemoteManifests {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_storage_ls_remote_manifests(
    state: StateStorageLsRemoteManifests,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    format: &mut ResFormat,
    offline: &ResOffline,
) -> Next {
    // The other end's store is the whole of what is asked for here, and an offline run may not ask
    // it.
    if **offline {
        return ErrorOffline.into();
    }

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let target = remote
        .get_ref()
        .vault_or_default(state.vault.unwrap_or_default())?;

    let name = current.get_ref().must_bind()?;
    let account = account_named(&name, Some(held), None)?;

    let runtime = tokio::runtime::Runtime::new().map_err(|error| ActionError::Io(error.into()))?;
    let listing = runtime.block_on(action_list_remote_async(
        held,
        &account,
        target.to_string(),
        "manifests".to_owned(),
        Progress::silent(),
    ))?;

    let keys = keys_of(&listing);
    format.set(
        "keys",
        keys.iter().map(|key| serde_json::json!(key)).collect(),
    );

    ResultManifests { keys }.into()
}

//! The `rola storage sync-hashes` command: make the two stores hold the named keys.
//!
//! Where [`sync-all`](crate::storage::cmd_storage_sync_all) carries everything either end holds,
//! this carries only the keys it is given — a full sync narrowed to a list. It is the same shape:
//! a Vault is reached the way `sync-all` reaches one, and the keys are the caller's to name instead
//! of the two ends working them out between them. Because what it changes is only what was named, it
//! does not ask first.

use std::str::FromStr as _;

use librorolala::daemon::action_sync_hashes_async;
use librorolala::protocol::ActionError;
use librorolala::storage::Key;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResOffline, ResRorolalaStorage, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_progress::Progress;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{offer, store_keys, typing_flag, vault_names};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_STORAGE_EXTRACT_FILE_BAD_HASH, EC_HELP};
use crate::failure::failure;
use crate::keys::account_named;

#[help(buffer)]
pub fn help_storage_sync_hashes(_: EntryStorageSyncHashes, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage_sync_hashes.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorageSyncHashes)]
pub fn desc_storage_sync_hashes() -> Description {
    t!("storage_sync_hashes.description").to_string().into()
}

/// Completes what `rola storage sync-hashes` can be given next.
///
/// Every word but the last is a key the store holds; the last may be the Vault to reach instead, so
/// both are offered wherever the word in progress is.
#[completion(EntryStorageSyncHashes)]
pub fn complete_storage_sync_hashes(
    ctx: ShellContext,
    storage: &mut LazyRes<ResRorolalaStorage>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    let mut names = store_keys(storage.get_ref().as_ref());
    names.extend(vault_names(remote.get_ref()));

    offer(&ctx, names)
}

/// Makes the stores at both ends hold the keys named
///
/// Each argument is a hash, read the way a key is read anywhere else — as hex, or with `blake3:` or
/// `manifest:` in front of it. What crosses is what only one end holds, in whichever direction that
/// is, so a key named here is on both ends by the time the run returns; a key neither end holds is
/// nothing to carry.
///
/// The last argument, when it does not read as a hash, names the Vault to reach, as
/// [`storage sync-all`](crate::storage::cmd_storage_sync_all) reads one: a name the Workspace has
/// bound, or an ip and a port, with none naming the one the Workspace reaches for by default.
///
/// # Errors
///
/// Renders [`ErrorSyncHashesHash`] when an argument does not read as a hash, and otherwise every way
/// this can fail is one the part that knows reports for itself, and `routeify` carries it out: the
/// run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]), the Workspace has no
/// Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or as one no scope holds
/// ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), or the exchange itself failed ([`ActionError`]).
///
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "storage.sync-hashes", entry = EntryStorageSyncHashes)]
pub fn storage_sync_hashes(args: EntryStorageSyncHashes, offline: &ResOffline) -> Next {
    // Moving keys between the two stores is speaking to the Vault, which an offline run may not do.
    if **offline {
        return ErrorOffline.into();
    }

    // Picking cannot fail: no positions are `None`-able but the list itself, which is empty.
    let words: Vec<String> = args.pick(&arg![Vec<String>]).unwrap_or_default();
    let (vault, hashes) = with_vault(words);

    StateStorageSyncHashes { vault, hashes }.into()
}

/// Splits the arguments into the hashes and the Vault, when one is named.
///
/// The Vault comes last, and is told apart by not reading as a hash: a hash is 64 hex digits, so a
/// name or an address — which is what a Vault is named by — is not one, and a name that happened to
/// be 64 hex digits is the one case this cannot tell apart.
fn with_vault(mut words: Vec<String>) -> (Option<String>, Vec<String>) {
    let vault = words
        .last()
        .filter(|word| Key::from_str(word).is_err())
        .cloned();

    if vault.is_some() {
        words.pop();
    }

    (vault, words)
}

/// The state of making the two stores hold the named keys.
#[derive(Grouped)]
pub struct StateStorageSyncHashes {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
    /// The hashes to sync, as they were given.
    hashes: Vec<String>,
}

#[chain(routeify)]
pub fn handle_storage_sync_hashes(
    state: StateStorageSyncHashes,
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

    // The hashes are read here, so a word that is not one is told apart from a hash before anything
    // crosses; what the action is handed is the hex it reads the same way.
    let mut hashes = Vec::with_capacity(state.hashes.len());
    for hash in &state.hashes {
        match Key::from_str(hash) {
            Ok(key) => hashes.push(key.hex()),
            Err(_) => return ErrorSyncHashesHash { hash: hash.clone() }.into(),
        }
    }

    let runtime = tokio::runtime::Runtime::new().map_err(|error| ActionError::Io(error.into()))?;
    runtime.block_on(action_sync_hashes_async(
        held,
        &account,
        target.to_string(),
        hashes.join("\n"),
        Progress::silent(),
    ))?;

    ResultSyncHashes.into()
}

/// Result: the two stores were made to hold the named keys.
#[derive(Grouped)]
pub struct ResultSyncHashes;

#[renderer(buffer)]
pub fn render_result_sync_hashes(_: ResultSyncHashes) {
    r_println!("{}", t!("storage_sync_hashes.result_synced").trim());
}

/// Error: an argument does not read as a hash.
#[derive(Grouped)]
pub struct ErrorSyncHashesHash {
    /// What was given instead of a hash.
    hash: String,
}

impl Failure for ErrorSyncHashesHash {
    fn name(&self) -> &'static str {
        "error_storage_sync_hashes_hash"
    }

    fn reason(&self) -> String {
        t!("storage_sync_hashes.err_bad_hash", hash = self.hash)
            .trim()
            .to_string()
    }
}

failure!(ErrorSyncHashesHash);

#[renderer(buffer)]
pub fn render_error_sync_hashes_hash(error: ErrorSyncHashesHash, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_sync_hashes.err_bad_hash_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_EXTRACT_FILE_BAD_HASH;
}

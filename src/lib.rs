#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::PathBuf;

use rorolala_utils_constants::WORKSPACE_KEYS_DIR;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate;
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;

use rorolala_auth::{Account, Accounts, KeyLocateRule, Member, Members};

/// Authentication
pub use rorolala_auth as auth;

/// Background daemon
pub use rorolala_daemon as daemon;

/// Errors across the C ABI
pub use rorolala_errors as errors;

/// Transport and wire protocol
pub use rorolala_protocol as protocol;

/// Client-side workspace
pub use rorolala_workspace as workspace;

/// Server-side vault
pub use rorolala_vault as vault;

/// The local key roots the current directory sits inside, highest priority first.
///
/// The Workspace comes before the Vault, since it is the copy being worked in, so a key
/// kept there shadows an equally named one in the Vault. The Vault then brings every scope
/// it is admitted to — see [`Vault::key_scopes`] — its own first and the Vaults holding it
/// after, so a member kept in a sub-vault is looked for there before the root, and is not
/// found at all by a search that starts above it. The keyring itself does not know what
/// any of those are: this is where their directories are turned into the plain roots it
/// searches.
fn local_roots(vault: Option<&Vault>, workspace: Option<&Workspace>) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    roots.extend(workspace.map(|held| held.get_root().join(WORKSPACE_KEYS_DIR)));
    if let Some(held) = vault {
        roots.extend(held.key_scopes());
    }

    roots
}

/// The Vault the current directory is inside, if it is inside one.
fn current_vault() -> Option<Vault> {
    Vault::locate(&std::env::current_dir().ok()?)
}

/// The Workspace the current directory is inside, if it is inside one.
fn current_workspace() -> Option<Workspace> {
    Workspace::locate(&std::env::current_dir().ok()?)
}

/// The local roots beside a Vault that was already found, and the Workspace the call is
/// made from.
fn roots_beside(vault: &Vault) -> Vec<PathBuf> {
    local_roots(Some(vault), current_workspace().as_ref())
}

/// The local roots the current directory sits inside.
fn roots_here() -> Vec<PathBuf> {
    let vault = current_vault();
    let workspace = current_workspace();

    local_roots(vault.as_ref(), workspace.as_ref())
}

/// Every member whose public key `rule` finds, highest priority first.
///
/// A member is named by a public key, which is meant to be shared, so it is looked for in
/// every scope the rule turns on — the local roots first, then the user's local data
/// directory, the filesystem root, and `ROLA_HOME` — with a name found higher up shadowing
/// the same name below it.
///
/// The local roots are the keys of the Workspace the call is made from and of `vault`, in
/// that order. The members cross as a set, read one index at a time; see [`Members`].
#[must_use]
#[lazyffi(export = rola_member_locate)]
pub fn member_locate(vault: &Vault, rule: &KeyLocateRule) -> Members {
    auth::locate_members(&roots_beside(vault), rule)
}

/// The member named `member_name`, if any scope the rule turns on holds its public key.
///
/// The lazy counterpart of [`member_locate`]: it stops at the first directory that has the
/// key rather than listing every one. The Vault and the Workspace are both looked for from
/// the current directory, since neither is given.
#[must_use]
#[lazyffi(export = rola_member_find)]
pub fn member_find(member_name: &str, rule: &KeyLocateRule) -> Option<Member> {
    auth::find_member(member_name, &roots_here(), rule)
}

/// Every account whose private key `rule` finds, highest priority first.
///
/// An account is named by a private key, which is not shared, so it is only ever looked
/// for in the local roots — beside the Workspace and `vault`, in that order — whatever the
/// rule says about the other scopes. The accounts cross as a set, read one index at a
/// time; see [`Accounts`].
#[must_use]
#[lazyffi(export = rola_account_locate)]
pub fn account_locate(vault: &Vault, rule: &KeyLocateRule) -> Accounts {
    auth::locate_accounts(&roots_beside(vault), rule)
}

/// The account named `account_name`, if a local root holds its private key.
///
/// The lazy counterpart of [`account_locate`], which also only looks in the local roots.
/// The Vault and the Workspace are both looked for from the current directory.
#[must_use]
#[lazyffi(export = rola_account_find)]
pub fn account_find(account_name: &str, rule: &KeyLocateRule) -> Option<Account> {
    auth::find_account(account_name, &roots_here(), rule)
}

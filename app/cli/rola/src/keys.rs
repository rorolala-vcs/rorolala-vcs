//! Where keys are looked for, and which accounts a run can act as.
//!
//! The scopes a key can live in are Rorolala's, not a command's: the listing a command
//! prints and the names completion offers are the same list, read from the same place, so
//! the two cannot come to disagree about what exists.

use std::path::PathBuf;

use librorolala::auth::{
    Account, KeyLocateRule, env_keys_dir, global_keys_dir, locate_accounts, user_keys_dir,
};
use librorolala::{vault::Vault, workspace::Workspace};
use mingling::Grouped;
use rorolala_utils_constants::{VAULT_KEYS_DIR, WORKSPACE_KEYS_DIR};
use rorolala_utils_location::Locate;

/// The directories keys are looked for in, highest priority first.
///
/// Everything Rorolala can reach: the keys beside the work at hand first — the Workspace's,
/// then the Vault's, since a key kept beside the copy being worked in shadows the same name
/// in the Vault — and after them the scopes the user keeps for themselves. A scope the
/// machine does not name is simply not there.
pub fn roots(workspace: Option<&Workspace>, vault: Option<&Vault>) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(held) = workspace {
        roots.push(held.get_root().join(WORKSPACE_KEYS_DIR));
    }
    if let Some(held) = vault {
        roots.push(held.get_root().join(VAULT_KEYS_DIR));
    }

    roots.extend(user_keys_dir());
    roots.extend(global_keys_dir());
    roots.extend(env_keys_dir());

    roots
}

/// The rule the directories above are searched under.
///
/// Every directory a command searches is one it named itself, the user's own stores
/// included, so the other scopes are turned off: turning one on would name its directory a
/// second time. `find_local` is what stays on — it is the flag for "search the directories
/// this caller handed in".
pub const fn scopes() -> KeyLocateRule {
    KeyLocateRule {
        find_global: false,
        find_local: true,
        find_user: false,
        find_env: false,
    }
}

/// Every account name any scope holds, deduplicated and in name order.
///
/// This is the one list `rola account` prints and completion offers: a name that is kept in
/// more than one scope is listed once, under the name it is known by, and the order does not
/// depend on which scope it came from.
pub fn account_names(workspace: Option<&Workspace>, vault: Option<&Vault>) -> Vec<String> {
    let mut names: Vec<String> = locate_accounts(&roots(workspace, vault), &scopes())
        .into_iter()
        .map(|account| account.name())
        .collect();

    names.sort();
    names.dedup();

    names
}

/// The account named `name`, from the first scope that holds it.
///
/// This is the account the [names](account_names) are listed under: the same search, so a
/// name that can be completed is one this finds, and one that cannot is reported rather than
/// handed back as nothing to act as.
///
/// # Errors
///
/// Returns [`ErrorAccountUnknown`] when no scope holds an account by that name.
pub fn account_named(
    name: &str,
    workspace: Option<&Workspace>,
    vault: Option<&Vault>,
) -> Result<Account, ErrorAccountUnknown> {
    locate_accounts(&roots(workspace, vault), &scopes())
        .into_iter()
        .find(|account| account.name() == name)
        .ok_or_else(|| ErrorAccountUnknown {
            name: name.to_string(),
        })
}

/// Error: the name given is not an account the work can act as.
///
/// A name is only a label, so it says nothing until a scope is found that holds the key under
/// it. [`account_named`] is where a command asks.
#[derive(Grouped)]
pub struct ErrorAccountUnknown {
    /// The name that is not an account.
    name: String,
}

impl ErrorAccountUnknown {
    /// The name that is not an account.
    ///
    /// What is said when the error is reported, so a renderer outside this module reads it
    /// through here rather than the field.
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }
}

//! Where accounts are looked for, and which names a run can act as.
//!
//! The scopes a key can live in are Rorolala's, not a caller's. The command line's account
//! listing and completion and the C ABI's write request read the same directories under the
//! same rule here, so the two cannot come to disagree about which account answers a name.

use std::path::{Path, PathBuf};

use rorolala_auth::{
    Account, KeyLocateRule, env_keys_dir, global_keys_dir, locate_accounts, user_keys_dir,
};
use rorolala_utils_constants::{VAULT_KEYS_DIR, WORKSPACE_KEYS_DIR};
use rorolala_utils_location::Locate as _;
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;

/// The directories keys are looked for in, highest priority first.
///
/// Everything Rorolala can reach: the keys beside the work at hand first, the Workspace's, then
/// the Vault's, since a key kept beside the copy being worked in shadows the same name in the
/// Vault, and after them the scopes the user keeps for themselves. A scope the machine does not
/// name is simply not there.
#[must_use]
pub fn roots(workspace: Option<&Workspace>, vault: Option<&Vault>) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(held) = workspace {
        roots.push(plain(&held.get_root().join(WORKSPACE_KEYS_DIR)));
    }
    if let Some(held) = vault {
        roots.push(plain(&held.get_root().join(VAULT_KEYS_DIR)));
    }

    roots.extend(user_keys_dir());
    roots.extend(global_keys_dir());
    roots.extend(env_keys_dir());

    roots
}

/// A path as it is meant to be read, with the `./` the layout writes taken out of it.
///
/// The scopes are named `./keys/` and the like, and joining one onto a root leaves that `./`
/// in the middle of every path built from it, a listing prints those paths, and a reader is
/// owed the place rather than the pieces it was joined from. Nothing is resolved by this: the
/// path is walked back into the components it is made of, which is what a caller would have
/// written.
fn plain(path: &Path) -> PathBuf {
    path.components().collect()
}

/// The rule the directories above are searched under.
///
/// Every directory a caller searches is one it named itself, the user's own stores included, so
/// the other scopes are turned off: turning one on would name its directory a second time.
/// `find_local` is what stays on, it is the flag for "search the directories this caller handed
/// in".
#[must_use]
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
/// This is the one list a command prints and completion offers: a name that is kept in more
/// than one scope is listed once, under the name it is known by, and the order does not depend
/// on which scope it came from.
#[must_use]
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
/// This is the same search [`account_names`] lists, so a name that can be completed is one this
/// finds. It answers nothing rather than a failure because the library does not own the
/// command's error type: a caller turns the absence into the failure it reports, and the C ABI
/// turns it into its own.
#[must_use]
pub fn account_named(
    name: &str,
    workspace: Option<&Workspace>,
    vault: Option<&Vault>,
) -> Option<Account> {
    locate_accounts(&roots(workspace, vault), &scopes())
        .into_iter()
        .find(|account| account.name() == name)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_constants::WORKSPACE_KEYS_DIR;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::{account_named, roots};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-keys-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");

        dir
    }

    /// An account is a private key file, so a file under the name is all a search reads.
    #[test]
    fn an_account_kept_in_the_workspace_is_found_by_name() {
        let dir = scratch("workspace-account");
        Workspace::create(&dir).expect("a Workspace");
        let workspace = Workspace::locate(&dir).expect("the Workspace is there");
        let keys = workspace.get_root().join(WORKSPACE_KEYS_DIR);
        fs::create_dir_all(&keys).expect("the Workspace's key directory");
        fs::write(keys.join("alice.pem"), "not really a key").expect("a key file");

        let found = account_named("alice", Some(&workspace), None).expect("the account is there");
        assert_eq!(found.name(), "alice");
        assert!(account_named("bob", Some(&workspace), None).is_none());

        // The Workspace's keys are the first root, since a key kept beside the work shadows an
        // equally named one in a scope below it.
        assert_eq!(
            roots(Some(&workspace), None).first(),
            Some(&keys.components().collect::<PathBuf>())
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

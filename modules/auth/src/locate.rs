use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate;
use rorolala_vault::Vault;
use rorolala_workspace::Workspace;

use crate::{Account, Accounts, KeyLocateRule, Member, Members};

/// Keys of a global (machine-wide) scope, under the filesystem root.
const GLOBAL_DIR: &str = ".rola/keys";

/// Keys of a user scope, under the user's local data directory.
const USER_DIR: &str = "rola/keys";

/// Keys named by the environment, under `ROLA_HOME`.
const ENV_DIR: &str = "keys";

/// The variable that names the directory an environment key set lives in.
const HOME_VAR: &str = "ROLA_HOME";

/// The extension a member's public key carries.
const PUBLIC_EXTENSION: &str = "pub";

/// The extension an account's private key carries.
const PRIVATE_EXTENSION: &str = "pem";

/// Every member whose public key `rule` finds, highest priority first.
///
/// A member is named by a **public** key, which is meant to be shared, so it is looked
/// for in every scope the rule turns on: the local scopes first, then the user's local
/// data directory, the filesystem root, and `ROLA_HOME`. That order is the precedence: a
/// name found in a higher scope shadows the same name in a lower one, and members of one
/// scope are ordered by name.
///
/// The local scope covers both places a key can sit beside the work at hand: the
/// Workspace the call is made from ([`rorolala_workspace::KEYS_DIR`]) and the Vault given
/// here ([`rorolala_vault::KEYS_DIR`]); the Workspace comes first, so a key kept there
/// shadows an equally named one in the Vault.
///
/// The members cross as a set, read one index at a time; see [`Members`].
#[must_use]
#[lazyffi(export = rola_member_locate)]
pub fn member_locate(vault: &Vault, rule: &KeyLocateRule) -> Members {
    Members::new(all_members(Some(vault), current_workspace().as_ref(), rule))
}

/// The member named `member_name`, if any scope the rule turns on holds its public key.
///
/// The lazy counterpart of [`member_locate`]: it asks each directory for one file instead
/// of listing them all, and stops at the first that has it. That is what makes asking
/// repeatedly for different names cheap, and why a caller that only wants one member
/// never pays for the whole set. The Vault and the Workspace are both looked for from the
/// current directory, since neither is given.
#[must_use]
#[lazyffi(export = rola_member_find)]
pub fn member_find(member_name: &str, rule: &KeyLocateRule) -> Option<Member> {
    let vault = current_vault();
    let workspace = current_workspace();

    one_member(member_name, vault.as_ref(), workspace.as_ref(), rule)
}

/// Every account whose private key `rule` finds, highest priority first.
///
/// An account is named by a **private** key, which is not shared, so it is only ever
/// looked for locally — beside the Workspace and the Vault, in that order. The other
/// scopes the rule turns on are places a public key can be shared from, never a private
/// one, and are not searched however they are set.
///
/// The accounts cross as a set, read one index at a time; see [`Accounts`].
#[must_use]
#[lazyffi(export = rola_account_locate)]
pub fn account_locate(vault: &Vault, rule: &KeyLocateRule) -> Accounts {
    Accounts::new(all_accounts(
        Some(vault),
        current_workspace().as_ref(),
        rule,
    ))
}

/// The account named `account_name`, if a local scope holds its private key.
///
/// The lazy counterpart of [`account_locate`], which also only looks locally. The Vault
/// and the Workspace are both looked for from the current directory, since neither is
/// given.
#[must_use]
#[lazyffi(export = rola_account_find)]
pub fn account_find(account_name: &str, rule: &KeyLocateRule) -> Option<Account> {
    let vault = current_vault();
    let workspace = current_workspace();

    one_account(account_name, vault.as_ref(), workspace.as_ref(), rule)
}

/// Every member the scopes `rule` turns on hold, highest priority first.
fn all_members(
    vault: Option<&Vault>,
    workspace: Option<&Workspace>,
    rule: &KeyLocateRule,
) -> Vec<Member> {
    merged(
        member_scopes(rule, vault, workspace)
            .iter()
            .map(|dir| members_in(dir)),
        Member::name,
    )
}

/// Every account the local scopes hold, highest priority first.
fn all_accounts(
    vault: Option<&Vault>,
    workspace: Option<&Workspace>,
    rule: &KeyLocateRule,
) -> Vec<Account> {
    merged(
        account_scopes(rule, vault, workspace)
            .iter()
            .map(|dir| accounts_in(dir)),
        Account::name,
    )
}

/// The member named `name`, from the first scope that has its public key.
fn one_member(
    name: &str,
    vault: Option<&Vault>,
    workspace: Option<&Workspace>,
    rule: &KeyLocateRule,
) -> Option<Member> {
    find_key(
        name,
        &member_scopes(rule, vault, workspace),
        PUBLIC_EXTENSION,
        Member::new,
    )
}

/// The account named `name`, from the first local scope that has its private key.
fn one_account(
    name: &str,
    vault: Option<&Vault>,
    workspace: Option<&Workspace>,
    rule: &KeyLocateRule,
) -> Option<Account> {
    find_key(
        name,
        &account_scopes(rule, vault, workspace),
        PRIVATE_EXTENSION,
        Account::new,
    )
}

/// The directories a member may be found in, highest priority first.
///
/// The order *is* the precedence, so nothing else has to know it. A directory that does
/// not exist is still named: a scope that is not set up is not an error, and reading it
/// simply finds nothing.
fn member_scopes(
    rule: &KeyLocateRule,
    vault: Option<&Vault>,
    workspace: Option<&Workspace>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if rule.find_local {
        dirs.extend(local_scopes(vault, workspace));
    }
    if rule.find_user {
        dirs.extend(user_dir());
    }
    if rule.find_global {
        dirs.extend(global_dir());
    }
    if rule.find_env {
        dirs.extend(env_dir());
    }

    dirs
}

/// The directories an account may be found in, highest priority first.
///
/// Only the local scopes: a private key is not shared, so the rules for the other scopes
/// do not reach it. `find_local` still says whether the local scopes are searched at all.
fn account_scopes(
    rule: &KeyLocateRule,
    vault: Option<&Vault>,
    workspace: Option<&Workspace>,
) -> Vec<PathBuf> {
    if rule.find_local {
        local_scopes(vault, workspace)
    } else {
        Vec::new()
    }
}

/// The two places a key sits beside the work at hand, highest priority first.
///
/// The Workspace comes first, since it is the copy being worked in, so a key kept there
/// shadows an equally named one in the Vault.
fn local_scopes(vault: Option<&Vault>, workspace: Option<&Workspace>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    dirs.extend(workspace.map(workspace_keys_dir));
    dirs.extend(vault.map(vault_keys_dir));

    dirs
}

/// Collects each scope's items, letting the first scope to name one keep it.
fn merged<T>(scopes: impl IntoIterator<Item = Vec<T>>, name: impl Fn(&T) -> String) -> Vec<T> {
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();

    for scope in scopes {
        for item in scope {
            if seen.insert(name(&item)) {
                items.push(item);
            }
        }
    }

    items
}

/// The member named by each public key directly inside `dir`, by name.
fn members_in(dir: &Path) -> Vec<Member> {
    files_in(dir, PUBLIC_EXTENSION)
        .into_iter()
        .map(|(name, key_path)| Member::new(name, key_path))
        .collect()
}

/// The account named by each private key directly inside `dir`, by name.
fn accounts_in(dir: &Path) -> Vec<Account> {
    files_in(dir, PRIVATE_EXTENSION)
        .into_iter()
        .map(|(name, key_path)| Account::new(name, key_path))
        .collect()
}

/// The name and path of each file carrying `extension` directly inside `dir`, by name.
fn files_in(dir: &Path, extension: &str) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| named_file(&entry.path(), extension))
        .collect();
    files.sort();

    files
}

/// The name a path is known by, if it names a file carrying `extension`.
fn named_file(path: &Path, extension: &str) -> Option<(String, PathBuf)> {
    let found = path.extension()?.to_str()?;
    if !path.is_file() || !found.eq_ignore_ascii_case(extension) {
        return None;
    }

    let name = path.file_stem()?.to_str()?.to_string();
    Some((name, path.to_path_buf()))
}

/// The item named `name` in the first directory that holds a file carrying `extension`.
fn find_key<T>(
    name: &str,
    dirs: &[PathBuf],
    extension: &str,
    make: impl Fn(String, PathBuf) -> T,
) -> Option<T> {
    dirs.iter().find_map(|dir| {
        let key_path = dir.join(format!("{name}.{extension}"));
        key_path.is_file().then(|| make(name.to_string(), key_path))
    })
}

/// The directory the Vault keeps its member keys in.
fn vault_keys_dir(vault: &Vault) -> PathBuf {
    vault.get_root().join(rorolala_vault::KEYS_DIR)
}

/// The directory the Workspace keeps its member keys in.
fn workspace_keys_dir(workspace: &Workspace) -> PathBuf {
    workspace.get_root().join(rorolala_workspace::KEYS_DIR)
}

/// The directory keys are kept in under the user's local data directory.
fn user_dir() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join(USER_DIR))
}

/// The directory keys are kept in under the filesystem root.
fn global_dir() -> Option<PathBuf> {
    Some(root_dir()?.join(GLOBAL_DIR))
}

/// The directory keys are kept in under `ROLA_HOME`, if it names one.
fn env_dir() -> Option<PathBuf> {
    let home = std::env::var_os(HOME_VAR).filter(|value| !value.is_empty())?;
    Some(PathBuf::from(home).join(ENV_DIR))
}

/// The filesystem root: `/` on Unix, the current drive's root on Windows.
fn root_dir() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()?
        .ancestors()
        .last()
        .map(Path::to_path_buf)
}

/// The Vault the current directory is inside, if it is inside one.
fn current_vault() -> Option<Vault> {
    Vault::locate(&std::env::current_dir().ok()?)
}

/// The Workspace the current directory is inside, if it is inside one.
fn current_workspace() -> Option<Workspace> {
    Workspace::locate(&std::env::current_dir().ok()?)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate;
    use rorolala_vault::Vault;
    use rorolala_workspace::Workspace;

    use crate::{KeyLocateRule, Member};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-auth-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A file called `name.extension` under `dir`, written so it exists.
    fn key(dir: &Path, name: &str, extension: &str) -> PathBuf {
        let path = dir.join(format!("{name}.{extension}"));
        fs::write(&path, "not really a key").unwrap();

        path
    }

    /// A public key, which names a member.
    fn public(dir: &Path, name: &str) -> PathBuf {
        key(dir, name, "pub")
    }

    /// A private key, which names an account.
    fn private(dir: &Path, name: &str) -> PathBuf {
        key(dir, name, "pem")
    }

    /// A rule that looks only at the local scopes, so a test sees only what it made.
    fn local_only() -> KeyLocateRule {
        KeyLocateRule {
            find_global: false,
            find_local: true,
            find_user: false,
            find_env: false,
        }
    }

    #[test]
    fn a_public_key_names_a_member_and_a_private_one_an_account() {
        let dir = scratch("names");
        public(&dir, "alice");
        private(&dir, "bob");
        fs::write(dir.join("notes.txt"), "neither").unwrap();
        fs::create_dir(dir.join("carol.pub")).unwrap();

        let members: Vec<String> = super::members_in(&dir).iter().map(Member::name).collect();
        let accounts: Vec<String> = super::accounts_in(&dir)
            .iter()
            .map(crate::Account::name)
            .collect();

        // A directory named like a key is not one, and neither is a file that carries
        // neither extension.
        assert_eq!(members, ["alice"]);
        assert_eq!(accounts, ["bob"]);
    }

    #[test]
    fn a_local_member_is_found_beside_its_vault() {
        let dir = scratch("vault");
        Vault::create(&dir).unwrap();
        let keys = dir.join(rorolala_vault::KEYS_DIR);
        fs::create_dir_all(&keys).unwrap();
        public(&keys, "alice");

        let vault = Vault::locate(&dir).unwrap();
        let members = super::all_members(Some(&vault), None, &local_only());

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].key_path(), keys.join("alice.pub"));
    }

    #[test]
    fn a_local_account_is_found_beside_its_vault() {
        let dir = scratch("vault-account");
        Vault::create(&dir).unwrap();
        let keys = dir.join(rorolala_vault::KEYS_DIR);
        fs::create_dir_all(&keys).unwrap();
        private(&keys, "alice");

        let vault = Vault::locate(&dir).unwrap();
        let accounts = super::all_accounts(Some(&vault), None, &local_only());

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].key_path(), keys.join("alice.pem"));
    }

    #[test]
    fn a_local_member_is_found_in_its_workspace_too() {
        let dir = scratch("workspace");
        Workspace::create(&dir).unwrap();
        let keys = dir.join(rorolala_workspace::KEYS_DIR);
        fs::create_dir_all(&keys).unwrap();
        public(&keys, "alice");

        let workspace = Workspace::locate(&dir).unwrap();
        let members = super::all_members(None, Some(&workspace), &local_only());

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].key_path(), keys.join("alice.pub"));
    }

    #[test]
    fn a_workspace_key_shadows_an_equally_named_vault_key() {
        let dir = scratch("shadow");
        Workspace::create(&dir).unwrap();
        Vault::create(&dir).unwrap();

        let workspace_keys = dir.join(rorolala_workspace::KEYS_DIR);
        fs::create_dir_all(&workspace_keys).unwrap();
        let written = public(&workspace_keys, "alice");

        let vault_keys = dir.join(rorolala_vault::KEYS_DIR);
        fs::create_dir_all(&vault_keys).unwrap();
        public(&vault_keys, "alice");

        let workspace = Workspace::locate(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        let members = super::all_members(Some(&vault), Some(&workspace), &local_only());

        // Both scopes name alice, and the Workspace is the one that keeps her, since it
        // is the copy the caller is working in.
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].key_path(), written);
    }

    #[test]
    fn an_account_is_never_looked_for_outside_the_local_scopes() {
        let dir = scratch("scopes");
        Workspace::create(&dir).unwrap();
        Vault::create(&dir).unwrap();

        let workspace = Workspace::locate(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        let everything = KeyLocateRule::new();

        // Whatever the other flags say, an account is only ever looked for locally...
        assert_eq!(
            super::account_scopes(&everything, Some(&vault), Some(&workspace)),
            super::local_scopes(Some(&vault), Some(&workspace))
        );

        // ...and a rule that turns the local scope off finds none at all.
        let nowhere = KeyLocateRule {
            find_local: false,
            ..KeyLocateRule::new()
        };
        assert!(super::account_scopes(&nowhere, Some(&vault), Some(&workspace)).is_empty());
    }

    #[test]
    fn a_shared_name_keeps_the_higher_scope() {
        let high = scratch("high");
        let low = scratch("low");
        public(&high, "alice");
        public(&high, "bob");
        public(&low, "alice");
        public(&low, "carol");

        let members = super::merged(
            [super::members_in(&high), super::members_in(&low)],
            Member::name,
        );
        let names: Vec<String> = members.iter().map(Member::name).collect();

        // Alice is shadowed in the lower scope, so only the higher one names her; bob
        // and carol are each only in one scope and both survive.
        assert_eq!(names, ["alice", "bob", "carol"]);
        assert_eq!(members[0].key_path(), high.join("alice.pub"));
    }

    #[test]
    fn finding_a_name_stops_at_the_first_scope_that_has_it() {
        let high = scratch("find-high");
        let low = scratch("find-low");
        public(&low, "alice");

        let found = super::find_key("alice", &[high, low.clone()], "pub", Member::new).unwrap();
        assert_eq!(found.key_path(), low.join("alice.pub"));

        assert!(super::find_key("nobody", &[], "pub", Member::new).is_none());
    }

    #[test]
    fn a_search_hands_back_a_set_that_is_read_by_index() {
        let dir = scratch("set");
        Vault::create(&dir).unwrap();
        let keys = dir.join(rorolala_vault::KEYS_DIR);
        fs::create_dir_all(&keys).unwrap();
        public(&keys, "alice");
        public(&keys, "bob");

        let vault = Vault::locate(&dir).unwrap();
        let found = super::member_locate(&vault, &local_only());

        assert_eq!(found.len(), 2);
        assert!(!found.is_empty());
        assert_eq!(found.at(0).unwrap().name(), "alice");
        assert_eq!(found.at(1).unwrap().name(), "bob");
        assert!(found.at(2).is_none());

        // The set is iterable in Rust, which is how a caller that does not go through C
        // reads every member without counting.
        let names: Vec<String> = found.into_iter().map(|member| member.name()).collect();
        assert_eq!(names, ["alice", "bob"]);
    }

    #[test]
    fn a_search_that_finds_nothing_hands_back_an_empty_set() {
        let dir = scratch("empty");
        Vault::create(&dir).unwrap();

        let vault = Vault::locate(&dir).unwrap();
        let found = super::member_locate(&vault, &local_only());

        assert_eq!(found.len(), 0);
        assert!(found.is_empty());
    }
}

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rorolala_utils_constants::{
    ENV_KEYS_DIR, GLOBAL_KEYS_DIR, HOME_ENV_VAR, PRIVATE_KEY_EXTENSION, PUBLIC_KEY_EXTENSION,
    USER_KEYS_DIR,
};

use crate::{Account, Accounts, KeyLocateRule, Member, Members};

/// Every member whose public key `rule` finds, highest priority first.
///
/// A member is named by a **public** key, which is meant to be shared, so it is looked
/// for in every scope the rule turns on: the local roots first, then the user's local data
/// directory, the filesystem root, and `ROLA_HOME`. That order is the precedence: a name
/// found in a higher scope shadows the same name in a lower one, and members of one scope
/// are ordered by name.
///
/// `local_roots` are where the keys of the Workspace and the Vault beside the caller sit,
/// highest priority first. They are given rather than found here, so this crate need not
/// know what a Workspace or a Vault is — the layer that does resolves them and passes
/// their key directories in.
///
/// The members cross as a set, read one index at a time; see [`Members`].
#[must_use]
pub fn locate_members(local_roots: &[PathBuf], rule: &KeyLocateRule) -> Members {
    Members::new(all_members(local_roots, rule))
}

/// The member named `member_name`, if any scope the rule turns on holds its public key.
///
/// The lazy counterpart of [`locate_members`]: it asks each directory for one file instead
/// of listing them all, and stops at the first that has it. That is what makes asking
/// repeatedly for different names cheap, and why a caller that only wants one member
/// never pays for the whole set.
#[must_use]
pub fn find_member(
    member_name: &str,
    local_roots: &[PathBuf],
    rule: &KeyLocateRule,
) -> Option<Member> {
    one_member(member_name, local_roots, rule)
}

/// Every account whose private key `rule` finds, highest priority first.
///
/// An account is named by a **private** key, which is not shared, so it is only ever
/// looked for in `local_roots` — the keys beside the Workspace and the Vault. The other
/// scopes the rule turns on are places a public key can be shared from, never a private
/// one, and are not searched however they are set.
///
/// The accounts cross as a set, read one index at a time; see [`Accounts`].
#[must_use]
pub fn locate_accounts(local_roots: &[PathBuf], rule: &KeyLocateRule) -> Accounts {
    Accounts::new(all_accounts(local_roots, rule))
}

/// The account named `account_name`, if a local root holds its private key.
///
/// The lazy counterpart of [`locate_accounts`], which also only looks in the local roots.
#[must_use]
pub fn find_account(
    account_name: &str,
    local_roots: &[PathBuf],
    rule: &KeyLocateRule,
) -> Option<Account> {
    one_account(account_name, local_roots, rule)
}

/// Every member the scopes `rule` turns on hold, highest priority first.
fn all_members(local_roots: &[PathBuf], rule: &KeyLocateRule) -> Vec<Member> {
    merged(
        member_scopes(rule, local_roots)
            .iter()
            .map(|dir| members_in(dir)),
        Member::name,
    )
}

/// Every account the local roots hold, highest priority first.
fn all_accounts(local_roots: &[PathBuf], rule: &KeyLocateRule) -> Vec<Account> {
    merged(
        account_scopes(rule, local_roots)
            .iter()
            .map(|dir| accounts_in(dir)),
        Account::name,
    )
}

/// The member named `name`, from the first scope that has its public key.
fn one_member(name: &str, local_roots: &[PathBuf], rule: &KeyLocateRule) -> Option<Member> {
    find_key(
        name,
        &member_scopes(rule, local_roots),
        PUBLIC_KEY_EXTENSION,
        Member::new,
    )
}

/// The account named `name`, from the first local root that has its private key.
fn one_account(name: &str, local_roots: &[PathBuf], rule: &KeyLocateRule) -> Option<Account> {
    find_key(
        name,
        &account_scopes(rule, local_roots),
        PRIVATE_KEY_EXTENSION,
        Account::new,
    )
}

/// The directories a member may be found in, highest priority first.
///
/// The order *is* the precedence, so nothing else has to know it. A directory that does
/// not exist is still named: a scope that is not set up is not an error, and reading it
/// simply finds nothing.
fn member_scopes(rule: &KeyLocateRule, local_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if rule.find_local {
        dirs.extend_from_slice(local_roots);
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
/// Only the local roots: a private key is not shared, so the rules for the other scopes do
/// not reach it. `find_local` still says whether the local roots are searched at all.
fn account_scopes(rule: &KeyLocateRule, local_roots: &[PathBuf]) -> Vec<PathBuf> {
    if rule.find_local {
        local_roots.to_vec()
    } else {
        Vec::new()
    }
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
    files_in(dir, PUBLIC_KEY_EXTENSION)
        .into_iter()
        .map(|(name, key_path)| Member::new(name, key_path))
        .collect()
}

/// The account named by each private key directly inside `dir`, by name.
fn accounts_in(dir: &Path) -> Vec<Account> {
    files_in(dir, PRIVATE_KEY_EXTENSION)
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

/// The directory keys are kept in under the user's local data directory.
fn user_dir() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join(USER_KEYS_DIR))
}

/// The directory keys are kept in under the filesystem root.
fn global_dir() -> Option<PathBuf> {
    Some(root_dir()?.join(GLOBAL_KEYS_DIR))
}

/// The directory keys are kept in under `ROLA_HOME`, if it names one.
fn env_dir() -> Option<PathBuf> {
    let home = std::env::var_os(HOME_ENV_VAR).filter(|value| !value.is_empty())?;
    Some(PathBuf::from(home).join(ENV_KEYS_DIR))
}

/// The filesystem root: `/` on Unix, the current drive's root on Windows.
fn root_dir() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()?
        .ancestors()
        .last()
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// A rule that looks only at the local roots, so a test sees only what it made.
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
    fn the_local_roots_are_searched_in_the_order_given() {
        let high = scratch("high");
        let low = scratch("low");
        public(&high, "alice");
        public(&high, "bob");
        public(&low, "alice");
        public(&low, "carol");

        let roots = vec![high.clone(), low];
        let members = super::locate_members(&roots, &local_only());
        let names: Vec<String> = members.iter().map(Member::name).collect();

        // Alice is shadowed in the lower root, so only the higher one names her; bob and
        // carol are each only in one root and both survive.
        assert_eq!(names, ["alice", "bob", "carol"]);
        assert_eq!(members.at(0).unwrap().key_path(), high.join("alice.pub"));
    }

    #[test]
    fn a_search_hands_back_a_set_that_is_read_by_index() {
        let dir = scratch("set");
        public(&dir, "alice");
        public(&dir, "bob");

        let found = super::locate_members(&[dir], &local_only());

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

        let found = super::locate_members(&[dir], &local_only());

        assert_eq!(found.len(), 0);
        assert!(found.is_empty());
    }

    #[test]
    fn finding_a_name_stops_at_the_first_root_that_has_it() {
        let high = scratch("find-high");
        let low = scratch("find-low");
        public(&low, "alice");

        let found = super::find_member("alice", &[high, low.clone()], &local_only()).unwrap();
        assert_eq!(found.key_path(), low.join("alice.pub"));

        assert!(super::find_member("nobody", &[], &local_only()).is_none());
    }

    #[test]
    fn an_account_is_only_looked_for_in_the_local_roots() {
        let roots = vec![PathBuf::from("/first"), PathBuf::from("/second")];
        let every_scope = KeyLocateRule::new();

        // Whatever the other flags say, an account is only ever looked for locally...
        assert_eq!(super::account_scopes(&every_scope, &roots), roots);

        // ...and a rule that turns the local roots off finds none at all.
        let nowhere = KeyLocateRule {
            find_local: false,
            ..KeyLocateRule::new()
        };
        assert!(super::account_scopes(&nowhere, &roots).is_empty());
    }
}

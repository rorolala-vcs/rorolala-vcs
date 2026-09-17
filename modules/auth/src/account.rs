use std::path::PathBuf;

use rorolala_utils_lazyffi::lazyffi;

/// A local account, as identified by the private key that holds it.
///
/// An account *is* a private key: the name is the `.pem` file's stem, and the path is
/// where that file sits. A private key is not shared, so an account is only ever looked
/// for beside the work at hand — a Workspace or a Vault — and never in a scope a public
/// key could come from.
///
/// An account is read-only: it says where its private key is, and changing that would
/// mean moving the key, which is not this type's to do.
#[lazyffi(export = RolaAccount)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Account {
    /// The account's name: the stem of its private key file.
    name: String,
    /// Where the account's private key file was found.
    key_path: PathBuf,
}

impl Account {
    /// Names an account by the private key file found at `key_path`.
    pub(crate) const fn new(name: String, key_path: PathBuf) -> Self {
        Self { name, key_path }
    }
}

#[lazyffi]
impl Account {
    /// The account's name, which is the stem of its private key file.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Where the account's private key file was found.
    ///
    /// The path is the one the search reached, so it says *which* local scope the key
    /// came from — the Workspace's or the Vault's.
    #[must_use]
    pub fn key_path(&self) -> PathBuf {
        self.key_path.clone()
    }
}

/// The accounts a search found, in the order it ranked them.
///
/// A search turns up a list, and C has no list of its own to receive one, so the list
/// crosses as a handle: how long it is, whether it is empty, and one account at a time by
/// index. The set owns its accounts, and each one is handed out as a copy, so reading the
/// same index twice yields two handles that both have to be released.
#[lazyffi(export = RolaAccounts)]
#[derive(Debug, Clone)]
pub struct Accounts {
    /// The accounts the search found, highest priority first.
    accounts: Vec<Account>,
}

impl Accounts {
    /// Wraps the accounts a search found.
    pub(crate) const fn new(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }

    /// Iterates the accounts, highest priority first.
    pub fn iter(&self) -> std::slice::Iter<'_, Account> {
        self.accounts.iter()
    }
}

#[lazyffi]
impl Accounts {
    /// How many accounts the search found.
    #[must_use]
    #[lazyffi(export = rola_accounts_len)]
    pub const fn len(&self) -> usize {
        self.accounts.len()
    }

    /// Whether the search found no accounts at all.
    #[must_use]
    #[lazyffi(export = is_rola_accounts_empty)]
    pub const fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// The account at `index`, counting from zero.
    ///
    /// The handle is a copy of the account the set holds, so releasing it releases only
    /// that copy — the set is read again for the next one.
    #[must_use]
    #[lazyffi(export = read_rola_accounts_by_index)]
    pub fn at(&self, index: usize) -> Option<Account> {
        self.accounts.get(index).cloned()
    }
}

impl IntoIterator for Accounts {
    type Item = Account;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.accounts.into_iter()
    }
}

impl<'a> IntoIterator for &'a Accounts {
    type Item = &'a Account;
    type IntoIter = std::slice::Iter<'a, Account>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

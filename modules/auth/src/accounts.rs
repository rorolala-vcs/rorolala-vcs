use rorolala_utils_lazyffi::lazyffi;

use crate::Account;

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

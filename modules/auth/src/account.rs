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

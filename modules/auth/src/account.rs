use std::fs;
use std::path::PathBuf;

use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

use crate::{Error, PublicKey, SigningKey};

/// A local account, as identified by the private key that holds it.
///
/// An account *is* a private key: the name is the `.pem` file's stem, and the path is
/// where that file sits. A private key is not shared, so an account is only ever looked
/// for beside the work at hand — a Workspace or a Vault — and never in a scope a public
/// key could come from.
///
/// A `.pem` is required of an account; a `.pub` beside it is not. When one is there, a
/// client holding the account has an identity to hold its peer to, and can require the
/// peer to prove it before anything runs. When there is not, the client has only its own
/// private key: it can still prove *itself* when challenged, but it has nothing to check
/// an answer against.
///
/// An account is read-only: it says where its keys are, and changing that would mean
/// moving them, which is not this type's to do.
#[lazyffi(export = RolaAccount)]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Account {
    /// The account's name: the stem of its private key file.
    name: String,
    /// Where the account's private key file was found.
    key_path: PathBuf,
    /// Where the public key found beside the private one sits, if there was one.
    public_path: Option<PathBuf>,
}

impl Account {
    /// Names an account by the private key file found at `key_path`, and the public key
    /// found beside it, if any.
    pub(crate) const fn new(name: String, key_path: PathBuf, public_path: Option<PathBuf>) -> Self {
        Self {
            name,
            key_path,
            public_path,
        }
    }

    /// Reads the account's private key from the file it was found at.
    ///
    /// The key is read afresh, so an account names a file rather than holding a copy of
    /// one: whatever the file holds when the account is asked is what the account is.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the key file cannot be read, and [`Error::Malformed`] if
    /// it does not hold a private key this build understands.
    pub fn get_key(&self) -> Result<SigningKey, Error> {
        let pem = fs::read_to_string(&self.key_path)?;

        SigningKey::from_pem(&pem)
    }

    /// The public half of the account's private key.
    ///
    /// An account is known to a peer by this key, not by its name: a name is only a
    /// label, and the key is what says which identity an action runs as.
    ///
    /// # Errors
    ///
    /// Returns what [`get_key`](Self::get_key) does.
    pub fn get_pub_key(&self) -> Result<PublicKey, Error> {
        Ok(self.get_key()?.public_key())
    }

    /// The public key a client holding this account holds its peer to, if the account
    /// came with one.
    ///
    /// The `.pem` is what an account proves itself *with*; a `.pub` beside it is what it
    /// asks its peer to prove. It only asks when the two say different things: a `.pub`
    /// holding the account's own key is the pair a key generator wrote beside the `.pem`, not
    /// a word about the peer, and holding a peer to it would be asking the peer to be this
    /// account. An account without one has nothing to check an answer against, so a client is
    /// only challenged rather than challenging back.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if a key file is there but cannot be read, and
    /// [`Error::Malformed`] if one does not hold a key this build understands.
    pub fn peer_key(&self) -> Result<Option<PublicKey>, Error> {
        let Some(path) = &self.public_path else {
            return Ok(None);
        };

        let pem = fs::read_to_string(path)?;
        let found = PublicKey::from_pem(&pem)?;

        if found == self.get_pub_key()? {
            return Ok(None);
        }

        Ok(Some(found))
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use ed25519_dalek::SigningKey as Ed25519SigningKey;
    use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
    use ed25519_dalek::pkcs8::{EncodePrivateKey as _, EncodePublicKey as _};

    use super::Account;
    use crate::Error;

    /// A path in a directory of the test's own, made if it is not there yet.
    fn path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rorolala-auth-account-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        dir.join(name)
    }

    #[test]
    fn an_account_reads_the_private_key_its_file_holds() {
        let signing = Ed25519SigningKey::from_bytes(&[3; 32]);
        let file = path("alice.pem");
        let pem = signing.to_pkcs8_pem(LineEnding::LF).unwrap();
        fs::write(&file, pem.as_str()).unwrap();

        let account = Account::new("alice".to_string(), file, None);

        let public = account.get_pub_key().unwrap();
        assert_eq!(account.get_key().unwrap().public_key(), public);
        assert_eq!(public.as_bytes(), signing.verifying_key().to_bytes());
    }

    #[test]
    fn an_account_whose_public_key_beside_it_is_its_own_holds_no_peer_to_it() {
        // A key generator writes a pair of the same identity, so the `.pub` says what the
        // account *is* rather than what its peer must be.
        let signing = Ed25519SigningKey::from_bytes(&[5; 32]);
        let file = path("carol.pem");
        let public = path("carol.pub");
        let pem = signing.to_pkcs8_pem(LineEnding::LF).unwrap();
        let spki = signing
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        fs::write(&file, pem.as_str()).unwrap();
        fs::write(&public, spki).unwrap();

        let account = Account::new("carol".to_string(), file, Some(public));

        assert!(account.peer_key().unwrap().is_none());
    }

    #[test]
    fn an_account_with_a_peer_public_key_beside_it_holds_its_peer_to_it() {
        let signing = Ed25519SigningKey::from_bytes(&[7; 32]);
        let peer = Ed25519SigningKey::from_bytes(&[8; 32]);
        let file = path("erin.pem");
        let public = path("erin.pub");
        let pem = signing.to_pkcs8_pem(LineEnding::LF).unwrap();
        let spki = peer
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        fs::write(&file, pem.as_str()).unwrap();
        fs::write(&public, spki).unwrap();

        let account = Account::new("erin".to_string(), file, Some(public));

        assert_eq!(
            account.peer_key().unwrap().unwrap().as_bytes(),
            peer.verifying_key().to_bytes()
        );
    }

    #[test]
    fn an_account_without_a_public_key_has_nothing_to_hold_its_peer_to() {
        let signing = Ed25519SigningKey::from_bytes(&[6; 32]);
        let file = path("dave.pem");
        let pem = signing.to_pkcs8_pem(LineEnding::LF).unwrap();
        fs::write(&file, pem.as_str()).unwrap();

        let account = Account::new("dave".to_string(), file, None);

        assert!(account.peer_key().unwrap().is_none());
    }

    #[test]
    fn an_account_whose_key_file_is_missing_says_so() {
        let account = Account::new("nobody".to_string(), path("nobody.pem"), None);

        assert!(matches!(account.get_key(), Err(Error::Io(_))));
    }
}

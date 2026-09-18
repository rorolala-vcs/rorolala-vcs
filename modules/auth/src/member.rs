use std::fs;
use std::path::PathBuf;

use rorolala_utils_lazyffi::lazyffi;
use serde::{Deserialize, Serialize};

use crate::{Error, PublicKey};

/// A member, as identified by the public key it is known by.
///
/// A member *is* its public key — a `.pub` file: the name is the file's stem, and the
/// path is where that file was found. A public key is meant to be shared, so a member can
/// be found in any scope a search covers, and nothing about a Vault's own record of a
/// member is kept here — the Vault is asked for that. A `.pub` that names no member yet
/// is therefore not an error, and a member can be found before one is made of it.
///
/// A member is read-only: it says where its public key is, and changing that would change
/// the member's identity, which is not this type's to do.
#[lazyffi(export = RolaMember)]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Member {
    /// The member's name: the stem of its public key file.
    name: String,
    /// Where the member's public key file was found.
    key_path: PathBuf,
}

impl Member {
    /// Names a member by the key file found at `key_path`.
    pub(crate) const fn new(name: String, key_path: PathBuf) -> Self {
        Self { name, key_path }
    }

    /// Reads the member's public key from the file it was found at.
    ///
    /// The key is read afresh, so a member names a file rather than holding a copy of
    /// one: whatever the file holds when the member is asked is what the member is.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the key file cannot be read, and [`Error::Malformed`] if
    /// it does not hold a public key this build understands.
    pub fn get_key(&self) -> Result<PublicKey, Error> {
        let pem = fs::read_to_string(&self.key_path)?;

        PublicKey::from_pem(&pem)
    }
}

#[lazyffi]
impl Member {
    /// The member's name, which is the stem of its public key file.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Where the member's public key file was found.
    ///
    /// The path is the one the search reached, so it says *which* directory a key came
    /// from — which is how a caller tells a local member from a user or global one.
    #[must_use]
    pub fn key_path(&self) -> PathBuf {
        self.key_path.clone()
    }
}

/// The members a search found, in the order it ranked them.
///
/// A search turns up a list, and C has no list of its own to receive one, so the list
/// crosses as a handle: how long it is, whether it is empty, and one member at a time by
/// index. The set owns its members, and each one is handed out as a copy, so reading the
/// same index twice yields two handles that both have to be released.
#[lazyffi(export = RolaMembers)]
#[derive(Debug, Clone)]
pub struct Members {
    /// The members the search found, highest priority first.
    members: Vec<Member>,
}

impl Members {
    /// Wraps the members a search found.
    pub(crate) const fn new(members: Vec<Member>) -> Self {
        Self { members }
    }

    /// Iterates the members, highest priority first.
    pub fn iter(&self) -> std::slice::Iter<'_, Member> {
        self.members.iter()
    }
}

#[lazyffi]
impl Members {
    /// How many members the search found.
    #[must_use]
    #[lazyffi(export = rola_members_len)]
    pub const fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the search found no members at all.
    #[must_use]
    #[lazyffi(export = is_rola_members_empty)]
    pub const fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// The member at `index`, counting from zero.
    ///
    /// The handle is a copy of the member the set holds, so releasing it releases only
    /// that copy — the set is read again for the next one.
    #[must_use]
    #[lazyffi(export = read_rola_members_by_index)]
    pub fn at(&self, index: usize) -> Option<Member> {
        self.members.get(index).cloned()
    }
}

impl IntoIterator for Members {
    type Item = Member;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.into_iter()
    }
}

impl<'a> IntoIterator for &'a Members {
    type Item = &'a Member;
    type IntoIter = std::slice::Iter<'a, Member>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use ed25519_dalek::SigningKey as Ed25519SigningKey;
    use ed25519_dalek::pkcs8::EncodePublicKey as _;
    use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;

    use super::Member;
    use crate::Error;

    /// A path in a directory of the test's own, made if it is not there yet.
    fn path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rorolala-auth-member-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        dir.join(name)
    }

    #[test]
    fn a_member_reads_the_public_key_its_file_holds() {
        let signing = Ed25519SigningKey::from_bytes(&[4; 32]);
        let file = path("bob.pub");
        let pem = signing
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        fs::write(&file, pem).unwrap();

        let member = Member::new("bob".to_string(), file);

        assert_eq!(
            member.get_key().unwrap().as_bytes(),
            signing.verifying_key().to_bytes()
        );
    }

    #[test]
    fn a_member_whose_key_file_is_missing_says_so() {
        let member = Member::new("nobody".to_string(), path("nobody.pub"));

        assert!(matches!(member.get_key(), Err(Error::Io(_))));
    }
}

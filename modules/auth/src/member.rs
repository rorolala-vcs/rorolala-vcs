use std::path::PathBuf;

use rorolala_utils_lazyffi::lazyffi;

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

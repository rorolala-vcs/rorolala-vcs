//! What a layout keeps for one entry.

use rorolala_storage::Blake3Hash;

/// What a Layout keeps for one [`Uuid`](uuid::Uuid): who holds it, what version it is at, and
/// what it says.
///
/// It is the whole of what may change about an entry. The content a version names is stored
/// elsewhere and is immutable; what changes is which version an entry is at, and who says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableData {
    /// Who holds it: an account's name, the same one `rola account` writes. `None` is an entry
    /// no account holds yet.
    owner: Option<String>,

    /// The version it is at, as the hash of that version.
    version: Blake3Hash,

    /// What it says about itself.
    description: String,
}

impl MutableData {
    /// Names an entry held by `owner`, at the version whose hash is `version`, described by
    /// `description`.
    #[must_use]
    pub fn new(
        owner: Option<String>,
        version: impl Into<Blake3Hash>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            owner,
            version: version.into(),
            description: description.into(),
        }
    }

    /// Who holds it, if an account does.
    #[must_use]
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    /// The version it is at, as the hash of that version.
    #[must_use]
    pub const fn version(&self) -> [u8; 32] {
        self.version
    }

    /// What it says about itself.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

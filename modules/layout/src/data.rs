//! What a layout keeps for one entry.

use rorolala_storage::Blake3Hash;

/// What a Layout keeps for one [`Uuid`](uuid::Uuid): who holds it, what version it is at, what it
/// says, and the cut its content is written with.
///
/// It is the whole of what may change about an entry. The content a version names is stored
/// elsewhere and is immutable; what changes is which version an entry is at, who says so, and — a
/// preference rather than a fact — how that content should be cut the next time it is written.
///
/// The preference is a name and nothing more: this crate does not know what a chunking policy is,
/// and the name is the one thing both sides can agree on without one of them depending on the other.
/// A name nothing answers to is a refusal where the name is *used*, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableData {
    /// Who holds it: an account's name, the same one `rola account` writes. `None` is an entry
    /// no account holds yet.
    owner: Option<String>,

    /// The version it is at, as the hash of that version.
    version: Blake3Hash,

    /// What it says about itself.
    description: String,

    /// The chunking policy this entry's content is written with, by its registered name.
    preference: Option<String>,
}

impl MutableData {
    /// Names an entry held by `owner`, at the version whose hash is `version`, described by
    /// `description`, and cut by no preference in particular.
    ///
    /// An entry that is *made* wants this: a preference is something an entry comes to have, not
    /// something it starts with. An entry being *changed* wants [`renewed`](Self::renewed), which
    /// keeps what this one would drop.
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
            preference: None,
        }
    }

    /// This entry's data, at `version`, held by `owner`, saying and preferring what it already did.
    ///
    /// This is how an entry is changed. Rebuilding one from its parts with [`new`](Self::new) is how
    /// its preference would be lost — a loss nothing reports, because the entry would go on being
    /// written, only cut another way than its owner asked for.
    #[must_use]
    pub fn renewed(&self, owner: Option<String>, version: impl Into<Blake3Hash>) -> Self {
        Self {
            owner,
            version: version.into(),
            description: self.description.clone(),
            preference: self.preference.clone(),
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

    /// The chunking policy this entry's content is written with, by its registered name.
    #[must_use]
    pub fn preference(&self) -> Option<&str> {
        self.preference.as_deref()
    }

    /// This entry's data with its preference set, or cleared when `preference` is `None`.
    #[must_use]
    pub fn with_preference(mut self, preference: Option<String>) -> Self {
        self.preference = preference;

        self
    }
}

#[cfg(test)]
mod tests {
    use super::MutableData;

    /// An entry with a preference, as a version recorded by `rola track --policy` would be.
    fn preferred() -> MutableData {
        MutableData::new(Some("alice".to_owned()), [1; 32], "first".to_owned())
            .with_preference(Some("maya-binary".to_owned()))
    }

    #[test]
    fn a_new_entry_prefers_nothing() {
        let data = MutableData::new(None, [0; 32], String::new());

        assert_eq!(data.preference(), None);
    }

    #[test]
    fn a_renewed_entry_keeps_what_it_said_and_what_it_prefers() {
        let data = preferred();
        let renewed = data.renewed(Some("bob".to_owned()), [2; 32]);

        assert_eq!(renewed.owner(), Some("bob"));
        assert_eq!(renewed.version(), [2; 32]);
        // The version moved and the preference did not: a write is not a request to cut differently.
        assert_eq!(renewed.description(), "first");
        assert_eq!(renewed.preference(), Some("maya-binary"));
    }

    #[test]
    fn a_preference_can_be_taken_back() {
        let data = preferred().with_preference(None);

        assert_eq!(data.preference(), None);
    }
}

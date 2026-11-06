//! What a layout keeps for one entry.

use rorolala_storage::Blake3Hash;

/// What kind of thing a Layout entry names.
///
/// A file entry names content whose version points at a stored file. A tree entry names a directory
/// whose version points at a stored tree structure, see `__PLAN_TREE_SYSTEM/01-data-model.md`. The
/// kind is written down with the entry, so a reader knows which without reading the content.
///
/// The kind does not change over an entry's life: a path that is recorded as one kind and becomes
/// the other is a new entry rather than an old one with a new kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum EntryKind {
    /// A file, whose version names stored file content.
    #[default]
    File = 0,

    /// A tree, whose version names stored tree structure.
    Tree = 1,
}

impl EntryKind {
    /// The number this kind is written down by.
    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// The kind written down by `number`, if this build knows one.
    ///
    /// # Errors
    ///
    /// Returns `None` for a number this build does not know. A caller reading one is reading
    /// something this build did not write, and the bytes after it may mean anything.
    #[must_use]
    pub const fn from_id(number: u8) -> Option<Self> {
        match number {
            0 => Some(Self::File),
            1 => Some(Self::Tree),
            _ => None,
        }
    }
}

/// What a Layout keeps for one [`Uuid`](uuid::Uuid): who holds it, what version it is at, what it
/// says, the cut its content is written with, and whether it names a file or a tree.
///
/// It is the whole of what may change about an entry. The content a version names is stored
/// elsewhere and is immutable; what changes is which version an entry is at, who says so, and how
/// that content should be cut the next time it is written, which is a preference rather than a
/// fact.
///
/// The preference is a name and nothing more: this crate does not know what a chunking policy is,
/// and the name is the one thing both sides can agree on without one of them depending on the other.
/// A name nothing answers to is a refusal where the name is *used*, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableData {
    /// Whether this entry names a file or a tree.
    kind: EntryKind,

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
    /// Names a file entry held by `owner`, at the version whose hash is `version`, described by
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
            kind: EntryKind::File,
            owner,
            version: version.into(),
            description: description.into(),
            preference: None,
        }
    }

    /// This entry's data, at `version`, held by `owner`, saying and preferring what it already did.
    ///
    /// This is how an entry is changed. Rebuilding one from its parts with [`new`](Self::new) is how
    /// its preference would be lost, which nothing reports, because the entry would go on being
    /// written, only cut another way than its owner asked for. The kind is kept for the same reason:
    /// an entry is one kind for its whole life.
    #[must_use]
    pub fn renewed(&self, owner: Option<String>, version: impl Into<Blake3Hash>) -> Self {
        Self {
            kind: self.kind,
            owner,
            version: version.into(),
            description: self.description.clone(),
            preference: self.preference.clone(),
        }
    }

    /// This entry's data, made to name a tree rather than a file.
    ///
    /// It is the only way a tree entry is made: the constructor makes files, and a tree is what
    /// `tree init` and the first tree commit build. A kind that could be flipped again would let a
    /// recorded file become a tree without a version that says so.
    #[must_use]
    pub const fn with_kind(mut self, kind: EntryKind) -> Self {
        self.kind = kind;

        self
    }

    /// Whether this entry names a file or a tree.
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    /// Who holds it, if an account does.
    #[must_use]
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
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

    /// The version it is at, as the hash of that version.
    #[must_use]
    pub const fn version(&self) -> [u8; 32] {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::{EntryKind, MutableData};

    /// An entry with a preference, as a version recorded by `rola track --policy` would be.
    fn preferred() -> MutableData {
        MutableData::new(Some("alice".to_owned()), [1; 32], "first".to_owned())
            .with_preference(Some("maya-binary".to_owned()))
    }

    #[test]
    fn a_new_entry_prefers_nothing_and_is_a_file() {
        let data = MutableData::new(None, [0; 32], String::new());

        assert_eq!(data.preference(), None);
        assert_eq!(data.kind(), EntryKind::File);
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
    fn a_renewed_tree_entry_is_still_a_tree() {
        let data = MutableData::new(None, [0; 32], String::new()).with_kind(EntryKind::Tree);
        let renewed = data.renewed(Some("bob".to_owned()), [2; 32]);

        assert_eq!(renewed.kind(), EntryKind::Tree);
    }

    #[test]
    fn a_preference_can_be_taken_back() {
        let data = preferred().with_preference(None);

        assert_eq!(data.preference(), None);
    }

    #[test]
    fn every_kind_round_trips_through_its_number() {
        for kind in [EntryKind::File, EntryKind::Tree] {
            assert_eq!(EntryKind::from_id(kind.id()), Some(kind));
        }

        assert_eq!(EntryKind::from_id(u8::MAX), None);
    }
}

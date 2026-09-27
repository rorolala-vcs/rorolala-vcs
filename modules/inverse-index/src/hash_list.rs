//! A list of hashes, as it crosses into C.
//!
//! FFI has no list of its own, and a hash list that crossed as nothing but a `char *` could not be
//! told from one hash crossing as a `char *`. So a list has a type of its own, carried as one string
//! with a hash a line, in the order the answer gives them — the same way a Creator or a Message is
//! carried, and released the same way.

use rorolala_utils_lazyffi::lazyffi;
use rorolala_vcs::Hash;

/// A list of hashes, one a line
#[lazyffi(export = RolaHashList)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashList(String);

impl HashList {
    /// The hashes, one a line.
    #[must_use]
    pub fn read_to_string(&self) -> &str {
        &self.0
    }

    /// How many hashes there are.
    #[must_use]
    pub fn count(&self) -> usize {
        if self.0.is_empty() {
            0
        } else {
            self.0.lines().count()
        }
    }

    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The hashes, in the order the answer gives them.
    pub fn hashes(&self) -> impl Iterator<Item = &str> {
        self.0.lines()
    }
}

impl From<Vec<Hash>> for HashList {
    fn from(hashes: Vec<Hash>) -> Self {
        Self(hashes.iter().map(Hash::hex).collect::<Vec<_>>().join("\n"))
    }
}

impl From<HashList> for String {
    fn from(list: HashList) -> Self {
        list.0
    }
}

#[cfg(test)]
mod tests {
    use rorolala_vcs::Hash;

    use super::HashList;

    #[test]
    fn a_list_is_hashes_one_a_line() {
        let list = HashList::from(vec![Hash::new([0x0f; 32]), Hash::new([0xab; 32])]);

        assert_eq!(list.count(), 2);
        assert!(!list.is_empty());
        assert_eq!(
            list.hashes().collect::<Vec<_>>(),
            vec!["0f".repeat(32), "ab".repeat(32)]
        );

        // Nothing to list is an empty string, not a string of one empty hash.
        assert!(HashList::from(Vec::<Hash>::new()).is_empty());
    }
}

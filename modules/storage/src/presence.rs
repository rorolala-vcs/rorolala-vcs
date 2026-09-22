use std::ops::{Deref, DerefMut};

/// Which of a batch of objects the other end holds.
///
/// A presence is what [`contains_keys`](crate::StorageBackend::contains_keys) answers with: **one
/// byte per key that was asked about**, in the order it was asked, holding
/// [`HELD`](Self::HELD) where the object is there and [`MISSING`](Self::MISSING) where it is
/// not. It is exactly as long as the batch it answers — a question about `n` keys comes back
/// as `n` bytes — so a position here and a key there always name the same object.
///
/// It reads as the bytes it holds, so a caller can index it or walk it as the slice it is, and
/// the checked ways of reading it — [`held`](Self::held), [`iter`](Self::iter) — say what a
/// byte means instead of leaving the caller to remember.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presence {
    /// One byte per key: `1` where the object is held, `0` where it is not.
    flags: Vec<u8>,
}

impl Presence {
    /// The byte that says an object is held.
    pub const HELD: u8 = 1;

    /// The byte that says an object is not held.
    pub const MISSING: u8 = 0;

    /// A presence from one byte per key.
    ///
    /// What each byte means is [`HELD`](Self::HELD) and [`MISSING`](Self::MISSING); anything
    /// else reads as held, since a byte that is neither is not something a peer can mean.
    #[must_use]
    pub const fn new(flags: Vec<u8>) -> Self {
        Self { flags }
    }

    /// A presence that holds nothing, as long as the batch it is to answer.
    ///
    /// This is where an answer starts: as long as the batch of keys, marked missing one position
    /// at a time.
    #[must_use]
    pub fn all_missing(len: usize) -> Self {
        Self {
            flags: vec![Self::MISSING; len],
        }
    }

    /// Whether the object at `index` is held.
    ///
    /// A position past the end of the batch is not held: it names no object that was asked
    /// about.
    #[must_use]
    pub fn held(&self, index: usize) -> bool {
        self.flags
            .get(index)
            .is_some_and(|flag| *flag != Self::MISSING)
    }

    /// Says whether the object at `index` is held.
    ///
    /// # Panics
    ///
    /// Panics if `index` is past the end of the batch: a presence is as long as the batch it
    /// answers, so marking a position outside it is a mistake in the caller and not a state
    /// this can be in.
    pub fn set_held(&mut self, index: usize, held: bool) {
        self.flags[index] = if held { Self::HELD } else { Self::MISSING };
    }

    /// Whether each object is held, in the order it was asked about.
    pub fn iter(&self) -> impl Iterator<Item = bool> {
        self.flags.iter().map(|flag| *flag != Self::MISSING)
    }
}

impl Deref for Presence {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.flags
    }
}

impl DerefMut for Presence {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.flags
    }
}

impl From<Vec<u8>> for Presence {
    fn from(flags: Vec<u8>) -> Self {
        Self { flags }
    }
}

impl From<&[u8]> for Presence {
    fn from(flags: &[u8]) -> Self {
        Self {
            flags: flags.to_vec(),
        }
    }
}

impl From<Presence> for Vec<u8> {
    fn from(presence: Presence) -> Self {
        presence.flags
    }
}

use rorolala_utils_lazyffi::lazyffi;

use crate::Member;

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

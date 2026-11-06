//! Merging two tree structures over the version they split from.
//!
//! A tree merge is decided path by path, and a path is content the structure names rather than bytes
//! to line up: a side that did not touch a path keeps what the other side did, a path both sides took
//! the same way is that way, and a path both changed differently is a conflict. What a conflict keeps
//! is this side's entry; the other side is named so the run can put it beside the work as a copy.

use std::collections::BTreeMap;

use crate::structure::{TreeEntry, TreeStructure};

/// What a merge of two structures over their shared version came to.
pub struct Merged {
    /// The structure the two sides make together, with each conflict at this side's entry.
    pub structure: TreeStructure,

    /// The paths both sides changed differently, sorted.
    pub conflicts: Vec<String>,
}

/// Merges `ours` and `theirs`, both taken from `base`.
///
/// The description is not merged: it is a version's own words, and the version the merge commits is
/// written after the work is settled.
#[must_use]
pub fn merge(base: &TreeStructure, ours: &TreeStructure, theirs: &TreeStructure) -> Merged {
    let base = by_path(base);
    let ours_by_path = by_path(ours);
    let theirs_by_path = by_path(theirs);

    let mut paths: Vec<String> = base
        .keys()
        .chain(ours_by_path.keys())
        .chain(theirs_by_path.keys())
        .cloned()
        .collect();
    paths.sort_unstable();
    paths.dedup();

    let mut entries: Vec<TreeEntry> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();

    for path in paths {
        let base_entry = base.get(&path);
        let ours_entry = ours_by_path.get(&path);
        let theirs_entry = theirs_by_path.get(&path);

        let kept = if ours_entry == base_entry {
            // This side did not touch it, so the other side's answer is the answer.
            theirs_entry
        } else if theirs_entry == base_entry || ours_entry == theirs_entry {
            // The other side did not touch it, or both took it the same way.
            ours_entry
        } else {
            conflicts.push(path.clone());
            ours_entry
        };

        if let Some(entry) = kept {
            entries.push(entry.clone());
        }
    }

    let structure = TreeStructure::build(None, entries).unwrap_or_else(|_| TreeStructure::empty());

    Merged {
        structure,
        conflicts,
    }
}

/// The entries of a structure, by path.
fn by_path(structure: &TreeStructure) -> BTreeMap<String, TreeEntry> {
    structure
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::merge;
    use crate::structure::{FileMode, TreeEntry, TreeStructure};
    use rorolala_storage::Key;

    fn key(seed: u8) -> Key {
        Key::new([seed; 32])
    }

    fn structure(entries: impl IntoIterator<Item = TreeEntry>) -> TreeStructure {
        TreeStructure::build(None, entries).expect("a structure")
    }

    fn file(path: &str, seed: u8) -> TreeEntry {
        TreeEntry::file(path, FileMode::Regular, key(seed))
    }

    /// A path one side changed and the other left alone is the changed one.
    #[test]
    fn a_side_that_did_not_change_a_path_keeps_the_other_side() {
        let base = structure([file("a.psd", 1), file("b.psd", 2)]);
        let ours = structure([file("a.psd", 9), file("b.psd", 2)]);
        let theirs = structure([file("a.psd", 1), file("b.psd", 2)]);

        let merged = merge(&base, &ours, &theirs);

        assert_eq!(merged.conflicts, Vec::<String>::new());
        assert_eq!(
            merged.structure.entry("a.psd").unwrap().storage(),
            Some(&key(9))
        );
    }

    /// A path both sides changed the same way is not a conflict.
    #[test]
    fn the_same_change_on_both_sides_is_not_a_conflict() {
        let base = structure([file("a.psd", 1)]);
        let ours = structure([file("a.psd", 7)]);
        let theirs = structure([file("a.psd", 7)]);

        let merged = merge(&base, &ours, &theirs);

        assert_eq!(merged.conflicts, Vec::<String>::new());
        assert_eq!(
            merged.structure.entry("a.psd").unwrap().storage(),
            Some(&key(7))
        );
    }

    /// A path both sides changed differently keeps this side and names the conflict.
    #[test]
    fn two_different_changes_are_a_conflict() {
        let base = structure([file("a.psd", 1)]);
        let ours = structure([file("a.psd", 2)]);
        let theirs = structure([file("a.psd", 3)]);

        let merged = merge(&base, &ours, &theirs);

        assert_eq!(merged.conflicts, vec!["a.psd".to_owned()]);
        assert_eq!(
            merged.structure.entry("a.psd").unwrap().storage(),
            Some(&key(2))
        );
    }

    /// A deletion on one side and a change on the other is a conflict too.
    #[test]
    fn a_deletion_against_a_change_is_a_conflict() {
        let base = structure([file("a.psd", 1)]);
        let ours = structure(Vec::new());
        let theirs = structure([file("a.psd", 3)]);

        let merged = merge(&base, &ours, &theirs);

        assert_eq!(merged.conflicts, vec!["a.psd".to_owned()]);
        assert!(merged.structure.entry("a.psd").is_none());
    }

    /// A file one side added and the other did not see is that side's addition.
    #[test]
    fn an_addition_on_one_side_is_kept() {
        let base = structure([file("a.psd", 1)]);
        let ours = structure([file("a.psd", 1), file("c.psd", 5)]);
        let theirs = structure([file("a.psd", 1)]);

        let merged = merge(&base, &ours, &theirs);

        assert_eq!(merged.conflicts, Vec::<String>::new());
        assert_eq!(
            merged.structure.entry("c.psd").unwrap().storage(),
            Some(&key(5))
        );
    }
}

//! How a tree's work stands beside what it records and what it has staged.

use std::collections::BTreeMap;

use crate::consts::CONFLICT_MARK;
use crate::structure::{EntryKind, TreeEntry, TreeStructure};

/// What happened to one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// The path is in the later structure and not the earlier one.
    Added,
    /// The path is in both, and what it holds changed.
    Modified,
    /// The path is in both and holds the same content, and its mode or its link target changed.
    Metadata,
    /// The path is in the earlier structure and not the later one.
    Deleted,
    /// The path is in both, and what it is changed.
    TypeChanged,
    /// The path moved: it is at another path in the later structure.
    Renamed,
}

/// One path that differs between two structures.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Change {
    /// The path in the later structure, or the path it was at for a deletion.
    pub path: String,

    /// What the path is in the later structure, or was for a deletion.
    pub kind: EntryKind,

    /// What happened to it.
    pub change: ChangeKind,

    /// Where it was, for a rename.
    pub old_path: Option<String>,
}

impl Change {
    /// The name of what happened, as data names it.
    ///
    /// It is the word a reading is asked for rather than a sentence a person is shown, so it does
    /// not change with the language the run is in.
    #[must_use]
    pub const fn status(&self) -> &'static str {
        match self.change {
            ChangeKind::Added => "added",
            ChangeKind::Modified => "modified",
            ChangeKind::Metadata => "metadata",
            ChangeKind::Deleted => "deleted",
            ChangeKind::TypeChanged => "type_changed",
            ChangeKind::Renamed => "renamed",
        }
    }
}

/// One path a merge left in conflict.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Conflict {
    /// The path the two sides disagree about.
    pub path: String,

    /// The conflict copy that holds the other side.
    pub theirs: String,
}

/// One path the tree holds and nothing staged has.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Untracked {
    /// The path.
    pub path: String,

    /// What it is.
    pub kind: EntryKind,
}

/// How a tree stands: what is staged, what is changed but not staged, and what a merge left.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct TreeStatus {
    /// Paths that differ between the recorded version and the staged structure.
    pub staged: Vec<Change>,

    /// Paths that differ between the staged structure and the work on disk.
    pub unstaged: Vec<Change>,

    /// Paths the work holds and the staged structure does not.
    pub untracked: Vec<Untracked>,

    /// Paths a merge left in conflict.
    pub conflicts: Vec<Conflict>,

    /// Whether the tree's own Layout entry is a change that must not be recorded.
    ///
    /// A tree is one entry of the Layout, so a base another side has moved past, or a lock another account
    /// holds, makes every change under it the wrong change rather than this account's work. The tree's own
    /// reading does not know the Layout, so the answer is filled in by the reader that does.
    pub wrong: bool,
}

impl TreeStatus {
    /// Whether nothing is staged, changed or in conflict.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.staged.is_empty()
            && self.unstaged.is_empty()
            && self.untracked.is_empty()
            && self.conflicts.is_empty()
            && !self.wrong
    }
}

/// How the work stands beside the recorded version and the staged structure.
///
/// `conflicts` is the paths a merge left, which the caller read from the tree's own list. A
/// conflict copy is not reported as untracked: it is the merge's, and it is named by the conflict.
#[must_use]
pub fn compare(
    recorded: &TreeStructure,
    staged: &TreeStructure,
    work: &TreeStructure,
    conflicts: &[String],
) -> TreeStatus {
    let mut staged_changes = difference(recorded, staged);
    detect_renames(&mut staged_changes, recorded, staged, false);

    let mut work_changes = difference(staged, work);
    let untracked = detect_renames(&mut work_changes, staged, work, true);

    TreeStatus {
        staged: staged_changes,
        unstaged: work_changes,
        untracked,
        conflicts: conflicts
            .iter()
            .map(|path| Conflict {
                path: path.clone(),
                theirs: conflict_copy(path),
            })
            .collect(),
        wrong: false,
    }
}

/// The changes from `before` to `after`, path by path.
///
/// It is what two versions of a tree hold apart: a path added, gone, holding other content, another
/// kind, or another mode. A move is not read as a move here, since two versions name their paths
/// rather than the work, so a path that moved is a deletion and an addition.
#[must_use]
pub fn diff(before: &TreeStructure, after: &TreeStructure) -> Vec<Change> {
    difference(before, after)
}

/// The changes from `before` to `after`, without paths that only moved.
fn difference(before: &TreeStructure, after: &TreeStructure) -> Vec<Change> {
    let before = by_path(before);
    let after = by_path(after);
    let mut changes = Vec::new();

    for (path, held) in &before {
        match after.get(path) {
            None => changes.push(Change {
                path: path.to_string(),
                kind: held.kind(),
                change: ChangeKind::Deleted,
                old_path: None,
            }),
            Some(other) => {
                if let Some(change) = changed(path, held, other) {
                    changes.push(change);
                }
            }
        }
    }

    for (path, held) in &after {
        if !before.contains_key(path) {
            changes.push(Change {
                path: path.to_string(),
                kind: held.kind(),
                change: ChangeKind::Added,
                old_path: None,
            });
        }
    }

    changes.sort_by(|left, right| left.path.cmp(&right.path));

    changes
}

/// How `after` differs from `before`, when it does.
fn changed(path: &str, before: &TreeEntry, after: &TreeEntry) -> Option<Change> {
    if before.kind() != after.kind() {
        return Some(Change {
            path: path.to_string(),
            kind: after.kind(),
            change: ChangeKind::TypeChanged,
            old_path: None,
        });
    }

    match after.kind() {
        EntryKind::File => {
            if before.storage() != after.storage() {
                Some(Change {
                    path: path.to_string(),
                    kind: after.kind(),
                    change: ChangeKind::Modified,
                    old_path: None,
                })
            } else if before.mode() != after.mode() {
                Some(Change {
                    path: path.to_string(),
                    kind: after.kind(),
                    change: ChangeKind::Metadata,
                    old_path: None,
                })
            } else {
                None
            }
        }
        EntryKind::Symlink => (before.target() != after.target()).then(|| Change {
            path: path.to_string(),
            kind: after.kind(),
            change: ChangeKind::Modified,
            old_path: None,
        }),
        EntryKind::Directory => None,
    }
}

/// Pairs the paths that disappeared with paths that appeared holding the same content.
///
/// The same storage under two paths is the same file moved: a move is a fact for the bytes a store
/// holds, and it costs one comparison. A text file edited while it moved is not a strong move and
/// stays a deletion and an addition here; guessing at it is the tree's own likeness reading, which
/// lives outside this comparison.
///
/// What appeared and was not paired with a path that went is answered as untracked when
/// `added_are_untracked` is set, and taken out of the changes; a staged addition is a change of the
/// version rather than something the work holds untracked.
fn detect_renames(
    changes: &mut Vec<Change>,
    before: &TreeStructure,
    after: &TreeStructure,
    added_are_untracked: bool,
) -> Vec<Untracked> {
    let before = by_path(before);
    let after = by_path(after);

    let mut deleted: BTreeMap<[u8; 32], Vec<String>> = BTreeMap::new();
    for change in changes.iter() {
        if change.change != ChangeKind::Deleted {
            continue;
        }

        if let Some(entry) = before.get(change.path.as_str())
            && entry.kind() == EntryKind::File
            && let Some(storage) = entry.storage()
        {
            deleted
                .entry(*storage.digest())
                .or_default()
                .push(change.path.clone());
        }
    }

    let mut paired_from: Vec<String> = Vec::new();
    let mut paired_to: Vec<String> = Vec::new();
    let mut renames: Vec<Change> = Vec::new();

    for change in changes.iter() {
        if change.change != ChangeKind::Added {
            continue;
        }

        let Some(entry) = after.get(change.path.as_str()) else {
            continue;
        };
        let Some(storage) = entry.storage() else {
            continue;
        };

        let Some(list) = deleted.get_mut(storage.digest()) else {
            continue;
        };
        let Some(from) = list.pop() else {
            continue;
        };

        renames.push(Change {
            path: change.path.clone(),
            kind: entry.kind(),
            change: ChangeKind::Renamed,
            old_path: Some(from.clone()),
        });
        paired_from.push(from);
        paired_to.push(change.path.clone());
    }

    changes
        .retain(|change| !paired_from.contains(&change.path) && !paired_to.contains(&change.path));
    changes.append(&mut renames);
    changes.sort_by(|left, right| left.path.cmp(&right.path));

    if !added_are_untracked {
        return Vec::new();
    }

    let untracked: Vec<Untracked> = changes
        .iter()
        .filter(|change| change.change == ChangeKind::Added)
        .map(|change| Untracked {
            path: change.path.clone(),
            kind: change.kind,
        })
        .collect();

    changes.retain(|change| change.change != ChangeKind::Added);

    untracked
}

/// The conflict copy beside `path`: its name with the conflict marker before its extension.
///
/// `textures/raw.psd` becomes `textures/raw~theirs.psd`, a name with no extension gets the marker at
/// its end, and a name that begins with a dot keeps that dot as part of its name.
#[must_use]
pub fn conflict_copy(path: &str) -> String {
    let (directory, name) = path
        .rsplit_once('/')
        .map_or(("", path), |(directory, name)| (directory, name));

    let copy = match name.rsplit_once('.') {
        Some((stem, suffix)) if !stem.is_empty() && !suffix.is_empty() => {
            format!("{stem}{CONFLICT_MARK}.{suffix}")
        }
        _ => format!("{name}{CONFLICT_MARK}"),
    };

    if directory.is_empty() {
        copy
    } else {
        format!("{directory}/{copy}")
    }
}

/// The entries of `structure`, by path.
fn by_path(structure: &TreeStructure) -> BTreeMap<&str, &TreeEntry> {
    structure
        .entries()
        .iter()
        .map(|entry| (entry.path(), entry))
        .collect()
}

#[cfg(test)]
mod tests {
    use rorolala_storage::Key;

    use super::{ChangeKind, compare, conflict_copy};
    use crate::structure::{FileMode, TreeEntry, TreeStructure};

    /// A key standing for content a test does not need to store.
    fn key(seed: u8) -> Key {
        Key::new([seed; 32])
    }

    /// A structure of files, each holding the content its seed names.
    fn tree(files: &[(&str, u8)]) -> TreeStructure {
        TreeStructure::build(
            None,
            files
                .iter()
                .map(|(path, seed)| TreeEntry::file(*path, FileMode::Regular, key(*seed))),
        )
        .unwrap()
    }

    #[test]
    fn a_path_that_appeared_is_staged_as_added() {
        let recorded = tree(&[("a.psd", 1)]);
        let staged = tree(&[("a.psd", 1), ("b.psd", 2)]);
        let status = compare(&recorded, &staged, &staged, &[]);

        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.staged[0].path, "b.psd");
        assert_eq!(status.staged[0].change, ChangeKind::Added);
        assert_eq!(status.unstaged, Vec::new());
        assert_eq!(status.untracked, Vec::new());
    }

    #[test]
    fn a_path_whose_content_changed_but_was_not_staged_is_unstaged_modified() {
        let recorded = tree(&[("a.psd", 1)]);
        let work = tree(&[("a.psd", 3)]);
        let status = compare(&recorded, &recorded, &work, &[]);

        assert_eq!(status.staged, Vec::new());
        assert_eq!(status.unstaged.len(), 1);
        assert_eq!(status.unstaged[0].path, "a.psd");
        assert_eq!(status.unstaged[0].change, ChangeKind::Modified);
    }

    #[test]
    fn a_path_the_work_holds_and_nothing_staged_does_is_untracked() {
        let recorded = tree(&[("a.psd", 1)]);
        let work = tree(&[("a.psd", 1), ("new.psd", 2)]);
        let status = compare(&recorded, &recorded, &work, &[]);

        assert_eq!(status.untracked.len(), 1);
        assert_eq!(status.untracked[0].path, "new.psd");
        assert_eq!(status.unstaged, Vec::new());
    }

    #[test]
    fn a_file_that_moved_holds_the_same_content_and_is_a_rename() {
        let recorded = tree(&[("a.psd", 1)]);
        let staged = tree(&[("b.psd", 1)]);
        let status = compare(&recorded, &staged, &staged, &[]);

        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.staged[0].change, ChangeKind::Renamed);
        assert_eq!(status.staged[0].path, "b.psd");
        assert_eq!(status.staged[0].old_path.as_deref(), Some("a.psd"));
        assert_eq!(status.untracked, Vec::new());
    }

    #[test]
    fn a_mode_change_on_the_same_content_is_metadata() {
        let recorded =
            TreeStructure::build(None, [TreeEntry::file("run.sh", FileMode::Regular, key(1))])
                .unwrap();
        let staged = TreeStructure::build(
            None,
            [TreeEntry::file("run.sh", FileMode::Executable, key(1))],
        )
        .unwrap();
        let status = compare(&recorded, &staged, &staged, &[]);

        assert_eq!(status.staged[0].change, ChangeKind::Metadata);
    }

    #[test]
    fn a_conflict_is_named_beside_the_path_it_is_about() {
        let recorded = tree(&[("textures/b.psd", 1)]);
        let status = compare(
            &recorded,
            &recorded,
            &recorded,
            &["textures/b.psd".to_owned()],
        );

        assert_eq!(status.conflicts.len(), 1);
        assert_eq!(status.conflicts[0].theirs, "textures/b~theirs.psd");
        assert!(!status.is_clean());
    }

    #[test]
    fn a_conflict_copy_is_the_name_with_the_marker_before_its_extension() {
        assert_eq!(conflict_copy("raw.psd"), "raw~theirs.psd");
        assert_eq!(conflict_copy("textures/raw.psd"), "textures/raw~theirs.psd");
        assert_eq!(conflict_copy("readme"), "readme~theirs");
        assert_eq!(conflict_copy(".gitignore"), ".gitignore~theirs");
        assert_eq!(conflict_copy("a.b.c"), "a.b~theirs.c");
    }

    #[test]
    fn a_directory_that_appeared_is_an_addition() {
        let recorded = tree(&[("a.psd", 1)]);
        let staged = TreeStructure::build(
            None,
            [
                TreeEntry::file("a.psd", FileMode::Regular, key(1)),
                TreeEntry::directory("renders"),
            ],
        )
        .unwrap();
        let status = compare(&recorded, &staged, &staged, &[]);

        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.staged[0].path, "renders");
        assert_eq!(
            status.staged[0].kind,
            crate::structure::EntryKind::Directory
        );
    }
}

#[cfg(test)]
mod shape_tests {
    use super::*;
    use crate::structure::EntryKind;

    /// The reading as data is the shape a caller of the C ABI reads: the four parts, and what a path
    /// is and what happened to it as words.
    #[test]
    fn a_status_serializes_as_the_data_shape() {
        let status = TreeStatus {
            staged: vec![Change {
                path: "a.psd".to_owned(),
                kind: EntryKind::File,
                change: ChangeKind::Modified,
                old_path: None,
            }],
            unstaged: Vec::new(),
            untracked: vec![Untracked {
                path: "b.psd".to_owned(),
                kind: EntryKind::File,
            }],
            conflicts: vec![Conflict {
                path: "c.psd".to_owned(),
                theirs: "c~theirs.psd".to_owned(),
            }],
            wrong: false,
        };

        let value = serde_json::to_value(&status).expect("the reading as data");

        assert_eq!(value["staged"][0]["kind"], "file");
        assert_eq!(value["staged"][0]["change"], "modified");
        assert_eq!(value["untracked"][0]["path"], "b.psd");
        assert_eq!(value["conflicts"][0]["theirs"], "c~theirs.psd");
    }
}

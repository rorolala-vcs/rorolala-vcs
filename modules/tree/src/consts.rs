//! The names a tree directory and its own data go by.

/// The directory whose presence makes a directory a tree.
pub const MARK: &str = ".rola-tree";

/// The Workspace's own data directory, which is not part of a tree's work.
pub const DATA: &str = ".rola";

/// The Layout entry id the tree keeps for itself, inside the mark.
///
/// It is what a reading uses to tell a tree that moved from a tree that was lost and another that
/// appeared: the directory carries the id wherever it goes.
pub const ENTRY_ID: &str = rorolala_utils_constants::TREE_ENTRY_ID_FILE;

/// The tree's staging store, inside the mark.
pub const STAGING: &str = "staging";

/// The staged tree structure, inside the staging store.
pub const STAGED_TREE: &str = "tree";

/// The tree's baseline, inside the mark.
pub const BASELINE: &str = "base";

/// The recorded structure copy, inside the baseline.
pub const BASELINE_TREE: &str = "tree";

/// The Layout version the baseline copy belongs to, inside the baseline.
pub const BASELINE_VERSION: &str = "version";

/// The digest cache of the files the tree holds, inside the baseline.
pub const BASELINE_ENTRIES: &str = "entries";

/// The description of the next version, inside the mark.
pub const DESCRIPTION: &str = "description";

/// The paths of the merge conflicts waiting in the tree, inside the mark.
pub const CONFLICTS: &str = "conflicts";

/// The lock a run holds while it changes the staging area, inside the mark.
pub const LOCK: &str = "lock";

/// The file of ignore rules a tree may keep at its root, which is part of its work.
pub const IGNORE: &str = ".rolaignore";

/// The marker in the name of a conflict copy.
pub const CONFLICT_MARK: &str = "~theirs";

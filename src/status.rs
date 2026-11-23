//! How the work stands, as the C ABI reads it.
//!
//! The reading itself is the status module's (see [`Status`], which is `rorolala_status::Status`)
//! and what is added here is the one thing a module cannot reach: which account the work acts as,
//! which belongs to the user rather than to a Vault or a Workspace, and which therefore sits with
//! the crate that ties the modules together.
//!
//! What a C caller gets is the reading and never a failure: a directory no Workspace holds, a
//! Workspace working in no Layout, and a tree or a Layout that cannot be read are all answered as
//! nothing, because a `NULL` is the whole of what this surface can say about any of them.

use std::path::Path;

use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::Layout;
use rorolala_tree::consts::{DESCRIPTION, ENTRY_ID, MARK};
use rorolala_tree::{
    Change as TreeChangeRecord, Conflict as TreeConflictRecord, EntryKind, Tree,
    TreeStatus as TreeStatusRecord, Untracked as TreeUntrackedRecord,
};
use rorolala_utils_constants::WORKSPACE_IGNORE_RULES_DIR;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;

pub use rorolala_status::*;

use crate::auth;

/// Reads how the work under `directory` stands beside the Layout it works from.
///
/// The Workspace is searched for upwards from the directory, so any directory inside the work reads
/// the same tree; the Layout read is the one the Workspace is working in, and the account is the one
/// the work acts as, which is what tells a change of this account's from one that is not its to
/// make. The index of the Vault or Workspace the directory is inside is read too, since a variant
/// file that moved is found again by what the index holds.
///
/// # FFI
///
/// A `RolaStatus` the caller releases with `free_rola_status`, or null when there is nothing to
/// read: no Workspace holds the directory, the Workspace works in no Layout, or the tree cannot be
/// read beside the one it does.
#[must_use]
#[lazyffi(export = rola_status_locate)]
pub fn locate_status(directory: &Path) -> Option<Status> {
    let workspace = Workspace::locate(directory)?;
    let layout = working_layout(&workspace)?;
    let index = VCSIndex::locate(directory);

    // The rules the Workspace ignores by are its own, kept beside its data; a set that cannot be
    // read is an empty one, since the reading is the thing being answered and the rules are a
    // refinement of it.
    let rules = IgnoreRuleSet::load(
        &workspace.get_root().join(WORKSPACE_IGNORE_RULES_DIR),
        workspace.get_root(),
    )
    .unwrap_or_else(|_| IgnoreRuleSet::empty());

    Status::read(
        &workspace,
        &layout,
        auth::current_account().as_deref(),
        index.as_ref(),
        &rules,
    )
    .ok()
}

/// The Layout a Workspace is working in, when it is working in one.
fn working_layout(workspace: &Workspace) -> Option<Layout> {
    workspace.layout_readonly().ok().flatten()
}

/// Whether a tree's own Layout entry is a change that must not be recorded.
///
/// A tree is one entry of the Layout, so its owner and the version it is based on are facts about the tree
/// rather than about the paths inside it. The tree's own reading does not know the Layout, and this is the
/// side that does: a tree another account holds, or one based on a version upstream has moved past, is a
/// wrong change whatever its contents say.
///
/// # Errors
///
/// Nothing: a tree whose Layout path cannot be read, or a reading that cannot be made, is answered as not
/// wrong rather than as a failure, because the caller is already showing the tree's own reading.
#[must_use]
pub fn tree_is_wrong(
    workspace: &Workspace,
    layout: &Layout,
    tree: &Tree,
    me: Option<&str>,
    index: Option<&VCSIndex>,
    runtime: &tokio::runtime::Runtime,
) -> bool {
    let Ok(path) = tree.layout_path() else {
        return false;
    };
    let Some(copy) = rorolala_status::vault_copy(workspace) else {
        return false;
    };
    let Some(id) = layout.id_of(&path) else {
        return false;
    };

    if me.is_some_and(|me| copy.entry(id).is_some_and(|data| data.owner() != Some(me))) {
        return true;
    }

    let Some(index) = index else {
        return false;
    };

    !rorolala_status::wrong_base(layout, &copy, index, runtime, std::slice::from_ref(&path))
        .is_empty()
}

/// How the rola-tree holding `directory` stands.
///
/// The reading is the tree's own: what its staging area holds beside the version the Layout records, what
/// the work under it changed without being staged, and what a merge left in conflict. A caller that read
/// `.rola-tree` itself would be keeping a second copy of the format, so the answer crosses as entries.
///
/// # FFI
///
/// A `RolaTreeStatus *` the caller releases with `free_rola_tree_status`, or null when there is nothing to
/// read: no Workspace holds the directory, the directory is not inside a tree, the Workspace works in no
/// Layout, the index or the store is not there, or the tree cannot be read.
#[must_use]
#[lazyffi(export = rola_tree_status)]
pub fn tree_status(directory: &Path) -> Option<TreeStatus> {
    read_tree_status(directory).map(|status| TreeStatus::from_record(&status))
}

/// How a tree stands beside the version it records.
#[lazyffi(export = RolaTreeStatus)]
#[derive(Debug, Clone, Default)]
pub struct TreeStatus {
    /// Paths that differ between the recorded version and the staged structure.
    staged: Vec<TreeChange>,
    /// Paths that differ between the staged structure and the work on disk.
    unstaged: Vec<TreeChange>,
    /// Paths the work holds and the staged structure does not.
    untracked: Vec<TreeUntracked>,
    /// Paths a merge left in conflict.
    conflicts: Vec<TreeConflict>,
    /// Whether the tree's own Layout entry is a change that must not be recorded.
    wrong: bool,
}

impl TreeStatus {
    /// The status of a reading, with every record taken out of the tree's own types.
    fn from_record(record: &TreeStatusRecord) -> Self {
        Self {
            staged: record.staged.iter().map(TreeChange::from_record).collect(),
            unstaged: record
                .unstaged
                .iter()
                .map(TreeChange::from_record)
                .collect(),
            untracked: record
                .untracked
                .iter()
                .map(TreeUntracked::from_record)
                .collect(),
            conflicts: record
                .conflicts
                .iter()
                .map(TreeConflict::from_record)
                .collect(),
            wrong: record.wrong,
        }
    }
}

#[lazyffi(export = rola_)]
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

    /// Whether the tree's own Layout entry makes the reading the wrong one to record.
    #[must_use]
    pub const fn wrong(&self) -> bool {
        self.wrong
    }

    /// How many paths differ between the recorded version and the staged structure.
    #[must_use]
    pub const fn staged_count(&self) -> usize {
        self.staged.len()
    }

    /// One staged change, or null past the end.
    #[must_use]
    pub fn staged_at(&self, index: usize) -> Option<TreeChange> {
        self.staged.get(index).cloned()
    }

    /// How many paths differ between the staged structure and the work on disk.
    #[must_use]
    pub const fn unstaged_count(&self) -> usize {
        self.unstaged.len()
    }

    /// One unstaged change, or null past the end.
    #[must_use]
    pub fn unstaged_at(&self, index: usize) -> Option<TreeChange> {
        self.unstaged.get(index).cloned()
    }

    /// How many paths the work holds and the staged structure does not.
    #[must_use]
    pub const fn untracked_count(&self) -> usize {
        self.untracked.len()
    }

    /// One untracked path, or null past the end.
    #[must_use]
    pub fn untracked_at(&self, index: usize) -> Option<TreeUntracked> {
        self.untracked.get(index).cloned()
    }

    /// How many paths a merge left in conflict.
    #[must_use]
    pub const fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

    /// One conflict, or null past the end.
    #[must_use]
    pub fn conflict_at(&self, index: usize) -> Option<TreeConflict> {
        self.conflicts.get(index).cloned()
    }
}

/// One path that differs between two structures.
#[lazyffi(export = RolaTreeChange)]
#[derive(Debug, Clone)]
pub struct TreeChange {
    /// The path in the later structure, or the path it was at for a deletion.
    path: String,
    /// What the path is: `file`, `directory` or `symlink`.
    kind: String,
    /// What happened: `added`, `modified`, `metadata`, `deleted`, `type_changed` or `renamed`.
    change: String,
    /// Where it was, for a rename, or an empty string.
    old_path: String,
}

impl TreeChange {
    /// The change a record says, with its words already read.
    fn from_record(record: &TreeChangeRecord) -> Self {
        Self {
            path: record.path.clone(),
            kind: kind_word(record.kind),
            change: record.status().to_owned(),
            old_path: record.old_path.clone().unwrap_or_default(),
        }
    }
}

#[lazyffi(export = rola_)]
impl TreeChange {
    /// The path in the later structure, or where it was for a deletion.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// What the path is.
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    /// What happened to it.
    #[must_use]
    pub fn change(&self) -> String {
        self.change.clone()
    }

    /// Where it was, for a rename, or an empty string.
    #[must_use]
    pub fn old_path(&self) -> String {
        self.old_path.clone()
    }
}

/// One path the tree holds and nothing staged has.
#[lazyffi(export = RolaTreeUntracked)]
#[derive(Debug, Clone)]
pub struct TreeUntracked {
    /// The path.
    path: String,
    /// What it is: `file`, `directory` or `symlink`.
    kind: String,
}

impl TreeUntracked {
    /// The untracked path a record says.
    fn from_record(record: &TreeUntrackedRecord) -> Self {
        Self {
            path: record.path.clone(),
            kind: kind_word(record.kind),
        }
    }
}

#[lazyffi(export = rola_)]
impl TreeUntracked {
    /// The path.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// What it is.
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }
}

/// One path a merge left in conflict.
#[lazyffi(export = RolaTreeConflict)]
#[derive(Debug, Clone)]
pub struct TreeConflict {
    /// The path the two sides disagree about.
    path: String,
    /// The conflict copy that holds the other side.
    theirs: String,
}

impl TreeConflict {
    /// The conflict a record says.
    fn from_record(record: &TreeConflictRecord) -> Self {
        Self {
            path: record.path.clone(),
            theirs: record.theirs.clone(),
        }
    }
}

#[lazyffi(export = rola_)]
impl TreeConflict {
    /// The path the two sides disagree about.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The conflict copy that holds the other side.
    #[must_use]
    pub fn theirs(&self) -> String {
        self.theirs.clone()
    }
}

/// What a path is, as the word the status uses.
fn kind_word(kind: EntryKind) -> String {
    match kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
        EntryKind::Symlink => "symlink",
    }
    .to_owned()
}

/// The status of the tree holding `directory`, when there is one to read.
fn read_tree_status(directory: &Path) -> Option<TreeStatusRecord> {
    let workspace = Workspace::locate(directory)?;
    let layout = working_layout(&workspace)?;
    let index = VCSIndex::locate(directory)?;
    let set = workspace.storage_set().ok()?;
    let rules = IgnoreRuleSet::load(
        &workspace.get_root().join(WORKSPACE_IGNORE_RULES_DIR),
        workspace.get_root(),
    )
    .unwrap_or_else(|_| IgnoreRuleSet::empty());
    let tree = Tree::locate(workspace.get_root(), directory).ok()?;
    let runtime = tokio::runtime::Runtime::new().ok()?;
    let wrong = tree_is_wrong(
        &workspace,
        &layout,
        &tree,
        auth::current_account().as_deref(),
        Some(&index),
        &runtime,
    );

    let mut status = runtime
        .block_on(tree.status(&layout, &index, &set, rules, false))
        .ok()?;
    status.wrong = wrong;

    Some(status)
}

/// Whether `directory` is the root of a tree.
///
/// The answer is the mark alone: a tree is a directory holding a `.rola-tree` directory, and
/// nothing about the Workspace, the Layout, the index or the store is read. That is what makes it
/// the cheap question a listing asks about every directory it draws, beside
/// [`tree_status`], which is the whole reading of one.
///
/// # FFI
///
/// `true` when the directory holds the tree's mark, `false` otherwise, including when the path is
/// not a directory.
#[must_use]
#[lazyffi(export = rola_tree_is_tree)]
pub fn is_tree(directory: &Path) -> bool {
    directory.join(MARK).is_dir()
}

/// The id of a tree and the description staged for it.
///
/// A tree's id is what tells a tree that moved from one that was lost and another that appeared;
/// the description is what the next version is to say. A caller that read `.rola-tree` itself
/// would be keeping a second copy of the format, which is what [`tree_status`] exists to avoid for
/// the status; this is the same answer for the two fields the status does not carry.
///
/// # FFI
///
/// A `RolaTreeMark *` the caller releases with `free_rola_tree_mark`, or null when the directory is
/// not the root of a tree. The id is empty when the file is there and cannot be read, and the
/// description is empty when none is staged.
#[must_use]
#[lazyffi(export = rola_tree_mark)]
pub fn tree_mark(directory: &Path) -> Option<TreeMark> {
    read_tree_mark(directory)
}

/// The id of a tree and the description staged for it.
#[lazyffi(export = RolaTreeMark)]
#[derive(Debug, Clone, Default)]
pub struct TreeMark {
    /// The Layout entry id the tree keeps, empty when the file says nothing.
    id: String,
    /// Whether a description is staged for the next version.
    described: bool,
    /// The description itself, empty when none is staged.
    description: String,
}

#[lazyffi(export = rola_)]
impl TreeMark {
    /// The Layout entry id the tree keeps, or an empty string when the file says nothing.
    #[must_use]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// Whether a description is staged for the next version.
    #[must_use]
    pub const fn described(&self) -> bool {
        self.described
    }

    /// The description itself, or an empty string when none is staged.
    #[must_use]
    pub fn description(&self) -> String {
        self.description.clone()
    }
}

/// The mark of the tree at `directory`, when the directory is the root of one.
fn read_tree_mark(directory: &Path) -> Option<TreeMark> {
    let mark = directory.join(MARK);

    if !mark.is_dir() {
        return None;
    }

    let id = std::fs::read_to_string(mark.join(ENTRY_ID))
        .map_or_else(|_| String::new(), |id| id.trim().to_owned());
    let description = std::fs::read_to_string(mark.join(DESCRIPTION)).ok();

    Some(TreeMark {
        id,
        described: description.is_some(),
        description: description.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString, c_char};
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::LayoutPath;
    use rorolala_utils_lazyffi::free_string;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{RolaStatus, Status, free_rola_status, is_tree, rola_status_locate, tree_mark};

    /// A directory of its own, emptied first so a rerun starts clean.
    ///
    /// The system temp is used when it can be. A machine whose temp is not writable, a Windows
    /// profile can be, falls back to the directory this test's own program sits in, which cargo has
    /// just written there and so is writable wherever the temp is not.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let name = format!(
            "rorolala-locate-status-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );

        for base in [Some(std::env::temp_dir()), program_dir()] {
            let Some(base) = base else {
                continue;
            };
            let dir = base.join(&name);
            let _ = fs::remove_dir_all(&dir);

            if fs::create_dir_all(&dir).is_ok() {
                return dir;
            }
        }

        panic!("no writable directory to make {name} in");
    }

    /// The directory this test's program sits in, or nothing when it cannot be found.
    fn program_dir() -> Option<PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
    }

    /// A Workspace working in one Layout that names `gone.txt`.
    fn held(label: &str) -> PathBuf {
        let dir = scratch(label);

        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layout = workspace.layout().unwrap();

        layout
            .create_path(&LayoutPath::new("gone.txt").unwrap(), Uuid::from_u128(1))
            .unwrap();

        dir
    }

    /// The status the ABI hands out for a directory, which the caller releases.
    fn read(directory: &Path) -> *mut RolaStatus {
        let raw = CString::new(directory.to_str().unwrap()).unwrap();

        // SAFETY: `raw` is a NUL-terminated C string that outlives the call, which is all the export
        // reads it for.
        unsafe { rola_status_locate(raw.as_ptr().cast_mut()) }
    }

    /// What a string the ABI handed out says, released once it has been read.
    ///
    /// # Safety
    ///
    /// `text` must be an owned C string the ABI handed out and has not been released.
    unsafe fn taken(text: *mut c_char) -> String {
        // SAFETY: the caller guarantees the string, as the export's contract says.
        let read = unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned();

        // SAFETY: the same string, released once and not used again.
        unsafe { free_string(text) };

        read
    }

    #[test]
    fn locating_nothing_hands_back_null() {
        let dir = scratch("absent");

        assert!(read(&dir).is_null(), "{dir:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_without_the_mark_is_no_tree() {
        let dir = scratch("not-tree");

        assert!(!is_tree(&dir));
        assert!(tree_mark(&dir).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_trees_id_and_staged_description_cross_as_data() {
        let dir = scratch("tree-mark");
        let mark = dir.join(".rola-tree");
        fs::create_dir_all(&mark).unwrap();
        fs::write(mark.join("id"), "1a2b3c4d").unwrap();

        assert!(is_tree(&dir));

        let reading = tree_mark(&dir).unwrap();
        assert_eq!(reading.id(), "1a2b3c4d");
        assert!(!reading.described());
        assert_eq!(reading.description(), "");

        fs::write(mark.join("description"), "the next version").unwrap();

        let reading = tree_mark(&dir).unwrap();
        assert!(reading.described());
        assert_eq!(reading.description(), "the next version");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_holding_no_layout_hands_back_null() {
        let dir = scratch("no-layout");
        Workspace::create(&dir).unwrap();

        // A Workspace has one Layout, and a reading has nothing to read when there is none: what is
        // taken away here is what a Workspace with nothing to adopt from reads as.
        fs::remove_dir_all(dir.join(".rola/layout")).unwrap();

        assert!(read(&dir).is_null(), "{dir:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_reading_crosses_as_a_handle_its_owner_releases() {
        let dir = held("reading");
        fs::write(dir.join("new.txt"), b"new").unwrap();

        let status = read(&dir);
        assert!(!status.is_null(), "{dir:?}");

        // SAFETY: `status` came from the export above and is read through the accessors it names,
        // in this one scope, and is released at the end of it.
        unsafe {
            assert_eq!(Status::rola_status_lost_count(status), 1);
            assert_eq!(Status::rola_status_untagged_count(status), 1);

            assert_eq!(taken(Status::rola_status_lost_at(status, 0)), "gone.txt");
            assert_eq!(taken(Status::rola_status_untagged_at(status, 0)), "new.txt");

            // Past the count there is nothing to answer with, which is an empty string and never a
            // path: a path a Layout names is never empty.
            assert_eq!(taken(Status::rola_status_lost_at(status, 1)), "");

            free_rola_status(status);
        }

        let _ = fs::remove_dir_all(&dir);
    }
}

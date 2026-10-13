//! How the working tree stands beside the Layout it works from.
//!
//! One reading, four answers about it: what the Layout names and the tree has lost, what the tree
//! holds and the Layout does not name, what changed where it was, and what moved. Beside those sit
//! the two things that are not the work's own — the changes the fetched copy of the Vault's Layout
//! says another account holds, and the variants waiting to be joined into files — because a reader
//! that acted on the first would be acting past the one thing it is not to do, and one that missed
//! the second would record a merge over work that is not finished.
//!
//! What is *done* about any of it is the reader's: this says how the work stands and nothing about
//! which command settles it.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use rorolala_layout::{Layout, LayoutError, LayoutPath};
use rorolala_storage::Key;
use rorolala_tree_analyze::{TreeDiffError, entry_of, tree_diff};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vcs::VCSIndex;
use rorolala_workspace::{Merging, Pending, Workspace};

use crate::base;
use crate::base::Relation;
use crate::{tracked_vault, unowned, variant_moves, vault_copy};

/// How alike two text files have to be to count as the same file moved, when nothing is said.
const DEFAULT_ALIKE: f32 = 0.6;

/// What one path of the work is, as a reading says it in a word.
///
/// A reading reports lists of paths, and a reader looking at one path — a file in a directory, a row of a
/// table — has one question about it rather than six. This is that question answered: the state the path is
/// in, which is the state a reader acts on first when a path is more than one of them at once.
///
/// # FFI
///
/// One variant by value, and no others: nothing about a path is [`PathState::Unknown`].
#[lazyffi(export = RolaPathState)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathState {
    /// The reading says nothing about the path: it is not a path of the work the Layout describes.
    Unknown,

    /// The Layout names it and the tree does not hold it, so nothing is there to show.
    ///
    /// A path a file moved away from is one of these: the reading reports the move rather than a loss and a
    /// gain, and what is at the path the Layout names is nothing either way.
    Lost,

    /// The tree holds it and the Layout does not name it.
    Untagged,

    /// Its content is not what the Layout agreed with, and it is this account's to record.
    Modified,

    /// Its content is not what the Layout agreed with, and it is not this account's to record: another
    /// account holds it, or the version it is based on is one upstream has moved past.
    Wrong,

    /// It is where a file the Layout names at another path now sits.
    Renamed,

    /// The same, where the move is one their likeness suggests rather than one their bytes make.
    RenamedLikely,

    /// A variant is waiting to be joined into it.
    Merging,
}

/// One path that moved, as the reading names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    /// The path the Layout names it by.
    pub from: String,
    /// The path the tree holds it at.
    pub to: String,
    /// Whether it was edited as well as moved.
    pub modified: bool,
    /// Whether the move is one the two being the same bytes makes rather than one their likeness
    /// suggests.
    pub strong: bool,
}

/// One variant waiting to be joined into one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    /// The file it is to be joined into.
    pub target: String,
    /// The variant that was checked in, as the hex it is named by.
    pub variant: String,
    /// The path its file lies at, or lay at before a move.
    pub path: String,
    /// Where the tree holds its file now, when a move was found.
    pub moved: Option<String>,
}

/// Why a merge in progress cannot go on.
///
/// A record that does not stand is not dropped quietly — a merge that cannot be carried out is
/// worse unseen — so it is answered as what it is rather than among the merges that are. What is
/// said here is the reason alone: the words for it belong to the program that shows it, and a
/// library has no language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Broken {
    /// The file the variant is for is no longer named by the Layout.
    NoTarget {
        /// The variant waiting, as the hex it is named by.
        variant: String,
    },
    /// Nowhere here can hold its variant file.
    NoPlace {
        /// The variant waiting, as the hex it is named by.
        variant: String,
        /// The file the variant is for, which the Layout still names.
        target: String,
    },
    /// The variant file is gone from where the record says it lies.
    FileGone {
        /// The file the variant is for.
        target: String,
        /// The path the variant file lies at.
        path: String,
    },
    /// This reading has no index to read the variant from.
    NoIndex {
        /// The file the variant is for.
        target: String,
        /// The path the variant file lies at.
        path: String,
    },
    /// The index does not hold that variant.
    NoVariant {
        /// The variant waiting, as the hex it is named by.
        variant: String,
        /// The file the variant is for.
        target: String,
    },
    /// The variant file could not be read.
    Unreadable {
        /// The file the variant is for.
        target: String,
        /// The path the variant file lies at.
        path: String,
    },
    /// The variant file no longer holds the variant's content.
    Changed {
        /// The file the variant is for.
        target: String,
        /// The path the variant file lies at.
        path: String,
    },
}

impl Broken {
    /// What the reason is about: the variant, or the path its file lies at.
    #[must_use]
    pub fn what(&self) -> &str {
        match self {
            Self::NoTarget { variant }
            | Self::NoPlace { variant, .. }
            | Self::NoVariant { variant, .. } => variant,
            Self::FileGone { path, .. }
            | Self::NoIndex { path, .. }
            | Self::Unreadable { path, .. }
            | Self::Changed { path, .. } => path,
        }
    }

    /// The file the variant is for, or an empty string when the Layout no longer names one.
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::NoTarget { .. } => "",
            Self::NoPlace { target, .. }
            | Self::FileGone { target, .. }
            | Self::NoIndex { target, .. }
            | Self::NoVariant { target, .. }
            | Self::Unreadable { target, .. }
            | Self::Changed { target, .. } => target,
        }
    }
}

/// How the working tree stands beside the Layout it works from.
///
/// # FFI
///
/// Opaque: C takes a pointer from `locate_rola_status` and gives it back with `free_rola_status`.
/// What it holds — the four lists, the moves among them, and the merges — is read one entry at a
/// time, by the accessors beside it.
#[lazyffi(export = RolaStatus)]
#[derive(Debug, Clone)]
pub struct Status {
    /// Where the Workspace the reading was made of is rooted, normalised.
    ///
    /// The paths reported here are relative to it, and a caller that has paths of its own — a listing,
    /// which names files the way the system does — needs to know what they are relative to before it can
    /// ask about one.
    root: PathBuf,

    /// Paths the Layout names that the tree does not hold.
    pub lost: Vec<String>,

    /// Paths the tree holds that the Layout does not name.
    pub untagged: Vec<String>,

    /// Paths that changed where they were, including the destinations of moves that were also
    /// edited, and never the ones another account holds.
    pub modified: Vec<String>,

    /// Paths that moved.
    pub renamed: Vec<Rename>,

    /// Paths the fetched copy of the Vault's Layout says another account holds.
    pub unowned: Vec<String>,

    /// Files a variant is waiting to be joined into, and the variant waiting.
    pub merging: Vec<Merge>,

    /// Merges that cannot go on, one reason each.
    pub broken: Vec<Broken>,

    /// Paths whose content changed while the version recorded for them is not the one upstream is at.
    ///
    /// These are the changes based on a version the other side has moved past — behind, or apart from
    /// it — so what is recorded here would be based on something the Vault does not have. It is a subset
    /// of [`modified`](Self::modified), and a path is in it only when the index could say how the two
    /// versions stand: a version this side never had is not a base known to be wrong.
    pub wrong_base: Vec<String>,

    /// Every path this Layout records a version for, and how that version stands beside the one the
    /// fetched copy of the Vault's Layout records.
    ///
    /// It is what the two sides being in step or not is read from, which is not the same question as
    /// whether a change is based on something wrong: a path that is ahead is ordinary work the other side
    /// has not been told of, and one that is behind is work it has not been given.
    ///
    /// A path either side names without a version is not here — nothing was ever recorded for it, so there
    /// is nothing to stand beside anything — and neither is a path upstream does not name at all. A reading
    /// made without the copy, the index or a runtime has nothing here, since the walk is what answers it.
    standing: Vec<(String, base::Standing)>,
}

impl Status {
    /// Reads how the tree a Workspace holds stands beside `layout`.
    ///
    /// `me` is the account the reading is made as, which is what tells a change of this account's
    /// from one that is not its to make; a reading with no account bound counts every content
    /// change as its own, since there is nothing to say otherwise. `index` is the version control
    /// index, which is what a variant file that moved is found again by, and what tells a version
    /// that is ahead of upstream from one that is behind; without one the merge records are still
    /// read and say what they can, and no base is called wrong.
    ///
    /// # Errors
    ///
    /// Returns [`StatusError::Tree`] when the tree cannot be read against the Layout — a Layout
    /// that cannot be read back, or a tree that cannot be walked — and [`StatusError::Layout`] when
    /// a fetched copy of the Vault's Layout that is there cannot be read.
    pub fn read(
        workspace: &Workspace,
        layout: &Layout,
        me: Option<&str>,
        index: Option<&VCSIndex>,
    ) -> Result<Self, StatusError> {
        let root = workspace.get_root();
        let diff = tree_diff(layout, workspace, DEFAULT_ALIKE)?;

        // What is not this account's to change is read before anything is said, and kept out of the
        // content changes: a reader that took them as its own work would be reading past the one
        // thing it is not to do.
        let tracked = tracked_vault(workspace, layout);
        let unowned = unowned(workspace, layout, tracked.as_deref(), me, &diff)?;
        let not_mine: BTreeSet<&str> = unowned.iter().map(String::as_str).collect();

        let modified: Vec<LayoutPath> = diff
            .modified
            .iter()
            .filter(|path| !not_mine.contains(path.as_str()))
            .cloned()
            .collect();

        // Which of the moves were edited as well: a move's destination is among the modified when
        // what it holds is not what the Layout agreed with at the path it came from.
        let edited: BTreeSet<&str> = diff.modified.iter().map(LayoutPath::as_str).collect();

        // One runtime for the whole reading, since both the merge records and the base each changed path
        // stands on are read through the index. A runtime that cannot be made is no reading of either:
        // what is left is a reading that says nothing about merges or bases rather than one that fails.
        let runtime = tokio::runtime::Runtime::new().ok();

        let merging = Merging::read(root);
        let (merging, broken, moved) = merging_view(
            &merging,
            layout,
            root,
            index,
            &diff.untagged,
            runtime.as_ref(),
        );

        // Where every recorded path stands beside the Vault's own, which is one walk of the chain per path.
        // A reading without the copy, the index or a runtime has none of them: how two versions stand is not
        // known without the chain, and a reading that guessed would be saying something it does not hold.
        let standing = match (index, runtime.as_ref()) {
            (Some(index), Some(runtime)) => vault_copy(workspace, layout)
                .map_or_else(Vec::new, |copy| base::every(layout, &copy, index, runtime)),
            _ => Vec::new(),
        };

        Ok(Self {
            root: normalize(workspace.get_root()),
            lost: diff
                .lost
                .iter()
                .map(LayoutPath::as_str)
                .map(str::to_owned)
                .collect(),
            untagged: diff
                .untagged
                .iter()
                .filter(|path| !moved.contains(*path))
                .map(LayoutPath::as_str)
                .map(str::to_owned)
                .collect(),
            modified: modified
                .iter()
                .map(LayoutPath::as_str)
                .map(str::to_owned)
                .collect(),
            renamed: diff
                .renamed
                .iter()
                .map(|rename| Rename {
                    from: rename.from.as_str().to_owned(),
                    to: rename.to.as_str().to_owned(),
                    modified: edited.contains(rename.to.as_str()),
                    strong: rename.strong,
                })
                .collect(),
            unowned,
            merging,
            broken,
            wrong_base: wrong_base(workspace, layout, index, runtime.as_ref(), &modified),
            standing: standing
                .into_iter()
                .map(|(path, standing)| (path.as_str().to_owned(), standing))
                .collect(),
        })
    }

    /// How the version recorded for `path` stands beside the one the Vault's copy records.
    ///
    /// The path is one a caller names, as [`state_of`](Self::state_of) takes it, and the answer is
    /// [`Relation::Unknown`] with no distance for a path this reading holds no standing for — one it is not
    /// about, one nothing was ever recorded for, and one upstream does not name. A caller that cannot tell
    /// those apart from a path in step is asking for the distance beside it: in step is a distance of zero
    /// and nothing else is.
    #[must_use]
    pub fn standing_of(&self, path: &Path) -> (Relation, Option<u64>) {
        let Some(at) = self.named(path) else {
            return (Relation::Unknown, None);
        };

        self.standing
            .iter()
            .find(|(named, _)| named == at.as_str())
            .map_or((Relation::Unknown, None), |(_, standing)| {
                (standing.relation, standing.distance)
            })
    }

    /// What the reading says about the entry at `path`, in one word.
    ///
    /// The path is one a caller names — a file of a listing, an entry of a table, as the system spells it —
    /// and what the reading names is a path relative to the Workspace it was made of, so the root is what
    /// joins the two: a path outside it is nothing this reading is about.
    ///
    /// The answer is the state a reader acts on first when a path is more than one of them at once: a change
    /// that must not be recorded comes before a merge waiting to be joined, which comes before a path the
    /// tree has lost, which comes before one the Layout does not name, which comes before a move — a move is
    /// a disagreement about where a file is, and that is read before what its content came to — which comes
    /// before a change of this account's own.
    #[must_use]
    pub fn state_of(&self, path: &Path) -> PathState {
        let Some(at) = self.named(path) else {
            return PathState::Unknown;
        };
        let at = at.as_str();

        if self.unowned.iter().any(|path| path == at)
            || self.wrong_base.iter().any(|path| path == at)
        {
            return PathState::Wrong;
        }

        if self.merging.iter().any(|merge| merge.target == at) {
            return PathState::Merging;
        }

        if self.lost.iter().any(|path| path == at)
            || self.renamed.iter().any(|rename| rename.from == at)
        {
            return PathState::Lost;
        }

        if self.untagged.iter().any(|path| path == at) {
            return PathState::Untagged;
        }

        if let Some(rename) = self.renamed.iter().find(|rename| rename.to == at) {
            return if rename.strong {
                PathState::Renamed
            } else {
                PathState::RenamedLikely
            };
        }

        if self.modified.iter().any(|path| path == at) {
            return PathState::Modified;
        }

        PathState::Unknown
    }

    /// Where the Workspace the reading was made of is rooted.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path as the reading names it, or nothing when the reading is about no such path.
    ///
    /// Normalised on both sides, so a path spelled with a `.` or a `..`, or reached through the Workspace's
    /// own root, is the same place rather than a second one — the same way the Vault's copy is asked about a
    /// path, and for the same reason.
    fn named(&self, path: &Path) -> Option<LayoutPath> {
        let path = normalize(path);
        let relative = path.strip_prefix(&self.root).ok()?;

        LayoutPath::from_relative(relative).ok()
    }
}

/// The changed paths whose version is behind or apart from the one upstream is at.
///
/// It is asked of a Workspace rather than of a Layout because the copy that answers it lives under the
/// Workspace's cache. Nothing is answered without all three of the copy, the index and a runtime: a base
/// is called wrong only when the two versions are there to be compared and the chain is there to walk.
fn wrong_base(
    workspace: &Workspace,
    layout: &Layout,
    index: Option<&VCSIndex>,
    runtime: Option<&tokio::runtime::Runtime>,
    modified: &[LayoutPath],
) -> Vec<String> {
    let (Some(index), Some(runtime), Some(copy)) = (index, runtime, vault_copy(workspace, layout))
    else {
        return Vec::new();
    };

    base::wrong_base(layout, &copy, index, runtime, modified)
}

/// What a reading of the tree failed with.
#[derive(Debug)]
pub enum StatusError {
    /// The tree could not be read against the Layout.
    Tree(TreeDiffError),

    /// The fetched copy of the Vault's Layout could not be read.
    Layout(LayoutError),
}

impl fmt::Display for StatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tree(error) => write!(formatter, "{error}"),
            Self::Layout(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for StatusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Tree(error) => Some(error),
            Self::Layout(error) => Some(error),
        }
    }
}

impl From<TreeDiffError> for StatusError {
    fn from(error: TreeDiffError) -> Self {
        Self::Tree(error)
    }
}

impl From<LayoutError> for StatusError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

/// What the merge records say, read against the Layout, the tree and the index.
///
/// A record is a merge only while all of it stands: the target is still named, the variant file is
/// still there — or is found again where a move put it — the index still holds the variant and the
/// file still holds that variant's content. One that does not stand is answered as the reason it
/// does not.
///
/// The paths the variant files were found at are handed back with it, so a reading that was told to
/// leave variant files out can leave these out too.
fn merging_view(
    merging: &Merging,
    layout: &Layout,
    root: &Path,
    index: Option<&VCSIndex>,
    untagged: &[LayoutPath],
    runtime: Option<&tokio::runtime::Runtime>,
) -> (Vec<Merge>, Vec<Broken>, BTreeSet<LayoutPath>) {
    let Some(runtime) = runtime else {
        return (Vec::new(), Vec::new(), BTreeSet::new());
    };

    let moves = variant_moves(merging, layout, root, index, runtime, untagged);
    let mut waiting = Vec::new();
    let mut broken = Vec::new();
    let mut claimed = BTreeSet::new();

    for pending in merging.iter() {
        // A file that moved is not one to report as gone: the record still stands, and what is to be
        // confirmed is where the file now lies.
        if let Some(moved) = moves.iter().find(|moved| moved.target == pending.target())
            && let Some(target) = layout.path_of(pending.target())
        {
            claimed.insert(moved.to.clone());
            waiting.push(Merge {
                target: target.as_str().to_owned(),
                variant: hex(pending.variant()),
                path: moved.from.as_str().to_owned(),
                moved: Some(moved.to.as_str().to_owned()),
            });

            continue;
        }

        match merging_item(merging, layout, root, index, runtime, pending) {
            Ok(item) => waiting.push(item),
            Err(cause) => broken.push(cause),
        }
    }

    (waiting, broken, claimed)
}

/// One record of the merge in progress, read as a merge that can go on or as why it cannot.
fn merging_item(
    merging: &Merging,
    layout: &Layout,
    root: &Path,
    index: Option<&VCSIndex>,
    runtime: &tokio::runtime::Runtime,
    pending: &Pending,
) -> Result<Merge, Broken> {
    let variant = hex(pending.variant());

    let Some(target) = layout.path_of(pending.target()) else {
        return Err(Broken::NoTarget { variant });
    };
    let target = target.as_str().to_owned();

    let Some(named) = merging.variant_path(layout, pending) else {
        return Err(Broken::NoPlace { variant, target });
    };
    let path = named.as_str().to_owned();

    let disk = root.join(named.to_path_buf());
    if !disk.is_file() {
        return Err(Broken::FileGone { target, path });
    }

    let Some(index) = index else {
        return Err(Broken::NoIndex { target, path });
    };
    let Some(joined) = base::read_variant(index, runtime, pending.variant()) else {
        return Err(Broken::NoVariant { variant, target });
    };

    let Ok(entry) = entry_of(&disk) else {
        return Err(Broken::Unreadable { target, path });
    };
    if entry.digest() != *joined.storage_hash() {
        return Err(Broken::Changed { target, path });
    }

    Ok(Merge {
        target,
        variant,
        path,
        moved: None,
    })
}

/// The hex a hash is named by.
fn hex(digest: &[u8; 32]) -> String {
    Key::new(*digest).hex()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::sleep;
    use std::time::Duration;

    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use rorolala_tree_analyze::remember_moved;
    use rorolala_utils_constants::{DEFAULT_LAYOUT_NAME, VAULT_LAYOUT_NAME};
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{PathState, Status};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-status-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);

        dir
    }

    /// A Workspace in a directory of its own, working in one Layout.
    fn workspace(label: &str) -> (PathBuf, Workspace) {
        let dir = scratch(label);

        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layouts = workspace.layouts();
        layouts.create(DEFAULT_LAYOUT_NAME).unwrap();
        layouts.set_current(DEFAULT_LAYOUT_NAME).unwrap();

        (dir, workspace)
    }

    /// The Layout a Workspace works in.
    fn working(workspace: &Workspace) -> Layout {
        workspace
            .layouts()
            .get(DEFAULT_LAYOUT_NAME)
            .unwrap()
            .unwrap()
    }

    /// Makes the Layout name `text`, under an id of its own and with a version.
    fn track(workspace: &Workspace, id: u128, text: &str) {
        let layout = working(workspace);
        layout
            .create_entry(
                Uuid::from_u128(id),
                MutableData::new(None, [1; 32], "first".to_owned()),
            )
            .unwrap();
        layout
            .create_path(&LayoutPath::new(text).unwrap(), Uuid::from_u128(id))
            .unwrap();
    }

    /// Reads a Workspace's work as a run with no account bound and no index.
    fn read(workspace: &Workspace) -> Status {
        Status::read(workspace, &working(workspace), None, None).unwrap()
    }

    /// What a reading says about a tree that holds what the Layout names.
    #[test]
    fn a_tree_holding_what_the_layout_names_has_nothing_to_say() {
        let (dir, workspace) = workspace("clean");
        track(&workspace, 1, "kept.txt");
        fs::write(dir.join("kept.txt"), b"kept").unwrap();

        let status = read(&workspace);

        assert_eq!(status.lost, Vec::<String>::new(), "{status:?}");
        assert_eq!(status.untagged, Vec::<String>::new(), "{status:?}");
        assert_eq!(status.modified, Vec::<String>::new(), "{status:?}");
        assert_eq!(status.renamed, Vec::new(), "{status:?}");
        assert_eq!(status.merging, Vec::new(), "{status:?}");
        assert_eq!(status.broken, Vec::new(), "{status:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    /// What the Layout names and the tree does not hold is lost, and what the tree holds and the
    /// Layout does not name is untagged.
    #[test]
    fn what_is_gone_is_lost_and_what_appeared_is_untagged() {
        let (dir, workspace) = workspace("structural");
        track(&workspace, 1, "kept.txt");
        track(&workspace, 2, "gone.txt");
        fs::write(dir.join("kept.txt"), b"kept").unwrap();
        fs::write(dir.join("new.txt"), b"new").unwrap();

        let status = read(&workspace);

        assert_eq!(status.lost, vec!["gone.txt".to_owned()], "{status:?}");
        assert_eq!(status.untagged, vec!["new.txt".to_owned()], "{status:?}");
        assert_eq!(status.modified, Vec::<String>::new(), "{status:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    /// What changed where it was is a content change, which is what `rola track` records — and the
    /// first reading of a file is what it agrees with, so a change is only a change after one.
    #[test]
    fn what_changed_where_it_was_is_a_content_change() {
        let (dir, workspace) = workspace("modified");
        track(&workspace, 1, "kept.txt");
        fs::write(dir.join("kept.txt"), b"before\n").unwrap();

        assert_eq!(read(&workspace).modified, Vec::<String>::new());

        // Long enough that the change is a change in time as well as in content.
        sleep(Duration::from_millis(20));
        fs::write(dir.join("kept.txt"), b"after\n").unwrap();

        assert_eq!(read(&workspace).modified, vec!["kept.txt".to_owned()]);

        // A reading with no index has no chain to walk, so no base is called wrong: what cannot be
        // judged is left unjudged rather than guessed at.
        assert_eq!(read(&workspace).wrong_base, Vec::<String>::new());

        let _ = fs::remove_dir_all(&dir);
    }

    /// A file that moved is read as one move rather than as a loss and a gain, which is what makes a
    /// destination something to confirm instead of two separate answers.
    #[test]
    fn a_file_that_moved_is_one_move() {
        let (dir, workspace) = workspace("moved");
        track(&workspace, 1, "a.txt");
        fs::write(dir.join("a.txt"), b"the same content\n").unwrap();

        // A first reading writes down what the file holds, so the move can be told next time.
        let _ = read(&workspace);

        fs::rename(dir.join("a.txt"), dir.join("b.txt")).unwrap();

        let status = read(&workspace);

        assert_eq!(status.renamed.len(), 1, "{status:?}");
        assert_eq!(status.renamed[0].from, "a.txt");
        assert_eq!(status.renamed[0].to, "b.txt");
        assert!(status.renamed[0].strong);
        assert_eq!(status.lost, Vec::<String>::new(), "{status:?}");
        assert_eq!(status.untagged, Vec::<String>::new(), "{status:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    /// What a reading says about one path, which is the one question a listing beside a name asks.
    #[test]
    fn a_path_is_answered_by_the_state_a_reader_acts_on_first() {
        let (dir, workspace) = workspace("states");
        track(&workspace, 1, "kept.txt");
        track(&workspace, 2, "gone.txt");
        fs::write(dir.join("kept.txt"), b"before\n").unwrap();
        fs::write(dir.join("new.txt"), b"new").unwrap();

        let status = read(&workspace);

        // What the tree holds and the Layout does not name, and what the Layout names and the tree has lost
        // — the second of which a listing never asks about, and a caller that does is told what became of it.
        assert_eq!(status.state_of(&dir.join("new.txt")), PathState::Untagged);
        assert_eq!(status.state_of(&dir.join("gone.txt")), PathState::Lost);

        // A path spelled through the root is the same place, which is what makes a listing's paths — named
        // the way the system names them — answerable at all.
        assert_eq!(
            status.state_of(&dir.join(".").join("new.txt")),
            PathState::Untagged
        );

        // Nothing is said about a file the reading has nothing to say about, about a place rather than a
        // file, or about a path outside the Workspace.
        assert_eq!(status.state_of(&dir.join("kept.txt")), PathState::Unknown);
        assert_eq!(status.state_of(&dir), PathState::Unknown);
        assert_eq!(
            status.state_of(&dir.join("..").join("outside.txt")),
            PathState::Unknown
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A file that moved is answered where it now sits, and where it was is a path the tree has lost.
    #[test]
    fn a_moved_file_is_answered_where_it_now_sits() {
        let (dir, workspace) = workspace("states-moved");
        track(&workspace, 1, "a.txt");
        fs::write(dir.join("a.txt"), b"the same content\n").unwrap();

        // A first reading writes down what the file holds, so the move can be told next time.
        let _ = read(&workspace);

        fs::rename(dir.join("a.txt"), dir.join("b.txt")).unwrap();

        let status = read(&workspace);

        assert_eq!(status.state_of(&dir.join("b.txt")), PathState::Renamed);
        assert_eq!(status.state_of(&dir.join("a.txt")), PathState::Lost);

        let _ = fs::remove_dir_all(&dir);
    }

    /// A path another account holds is wrong before it is a change of content at all.
    #[test]
    fn a_path_another_account_holds_is_wrong_before_it_is_changed() {
        let (dir, workspace) = workspace("states-held");
        track(&workspace, 1, "hero.psd");
        workspace
            .layouts()
            .set_track(DEFAULT_LAYOUT_NAME, "vault")
            .unwrap();

        // The copy says somebody else holds the entry, which is what makes changing it wrong.
        let copy_dir = workspace.readonly_layout_dir("vault", VAULT_LAYOUT_NAME);
        fs::create_dir_all(&copy_dir).unwrap();
        let copy = Layout::open(&copy_dir).unwrap();
        copy.create_entry(
            Uuid::from_u128(1),
            MutableData::new(Some("alice".to_owned()), [1; 32], String::new()),
        )
        .unwrap();

        fs::write(dir.join("hero.psd"), b"before\n").unwrap();
        let _ = read(&workspace);

        sleep(Duration::from_millis(20));
        fs::write(dir.join("hero.psd"), b"after\n").unwrap();

        let status = Status::read(&workspace, &working(&workspace), Some("bob"), None).unwrap();

        assert_eq!(status.unowned, vec!["hero.psd".to_owned()], "{status:?}");
        assert_eq!(status.modified, Vec::<String>::new(), "{status:?}");
        assert_eq!(status.state_of(&dir.join("hero.psd")), PathState::Wrong);

        let _ = fs::remove_dir_all(&dir);
    }

    /// A file that was changed and then moved is still a change, wherever it is now.
    ///
    /// What the Layout agrees with is the entry rather than the path it is named at, so a move carries that
    /// agreement with it — which is what `rola fs-ops mv` writes down, since by the time a reading looks, the
    /// path that would tell it is gone. Written down as agreement with what the file holds now, a change made
    /// before a move would be a change no reading reports: what is checked here is the run that hid one.
    #[test]
    fn a_file_that_changed_and_then_moved_is_still_a_change() {
        let (dir, workspace) = workspace("moved-changed");
        track(&workspace, 1, "a.txt");
        fs::write(dir.join("a.txt"), b"before\n").unwrap();

        // A first reading writes down what the file holds, which is what the Layout then agrees with.
        let _ = read(&workspace);

        sleep(Duration::from_millis(20));
        fs::write(dir.join("a.txt"), b"after\n").unwrap();

        assert_eq!(read(&workspace).modified, vec!["a.txt".to_owned()]);

        // The move: what the file system did, what the Layout says about it, and what the run writes down.
        let from = LayoutPath::new("a.txt").unwrap();
        let to = LayoutPath::new("b.txt").unwrap();
        let layout = working(&workspace);

        fs::rename(dir.join("a.txt"), dir.join("b.txt")).unwrap();
        layout.move_path(&from, &to).unwrap();
        remember_moved(&dir, &layout, &from, &to).unwrap();

        let status = read(&workspace);

        // What changed is still what changed: the path the Layout names is where the file is, so there is no
        // move left to be seen, and the change is the reading's own to report.
        assert_eq!(status.modified, vec!["b.txt".to_owned()], "{status:?}");
        assert_eq!(status.renamed, Vec::new(), "{status:?}");
        assert_eq!(status.state_of(&dir.join("b.txt")), PathState::Modified);

        let _ = fs::remove_dir_all(&dir);
    }
}

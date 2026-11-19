//! Recording the content of files as versions, and naming them in the Layout.
//!
//! The one-file recording is [`track_file`]: a path, a store, an index and a Layout, and what comes
//! out is the version the Layout now names. The batch workflow is [`batch::plan`] and [`batch::apply`], and it is
//! what a command calls when a run names many paths, a directory, or a tree: a plan reads every file
//! and every tree the run named and says what changed, what did not and what it refuses, without
//! writing anything, and an apply records what the plan named once the caller has said what each
//! subject did.
//!
//! The split is what lets the message be asked for the files that will actually be recorded: the
//! plan reads the content and settles every gate before an editor is opened, so a run whose editor is
//! left empty has written nothing, and the list the editor shows is the list apply will write.

use std::path::{Path, PathBuf};

use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_storage::{Key, StorageSet};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_vault::Vault;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use serde::Serialize;
use uuid::Uuid;

use crate::write::{
    Cancellation, WriteError, WriteProgress,
    index::write_file_version,
    layout::{create_path, write_entry},
    storage::put_file,
    workspace::rebuild_tree_cache,
};

/// What tracking one file wrote.
#[derive(Debug, Clone, Serialize)]
pub struct TrackedFile {
    /// The Layout entry the path names.
    pub id: Uuid,
    /// The content's key.
    pub storage: Key,
    /// The variant the version points at.
    pub variant: Key,
    /// The version the Layout records.
    pub version: Key,
}

/// How a file is to be tracked.
#[derive(Debug, Clone, Default)]
pub struct TrackOptions {
    /// The chunking policy to write with, or empty for the store's own choice.
    pub policy: String,
    /// The account that holds the entry, or nothing to keep the existing owner.
    pub owner: Option<String>,
    /// What the entry says about itself.
    pub description: String,
}

/// The batch workflow: read a plan, then carry it out, and record what the plan named.
pub mod batch {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use rorolala_errors::Failure as _;
    use rorolala_ignore::IgnoreRuleSet;
    use rorolala_layout::{Batch, EntryKind, Layout, LayoutPath, MutableData};
    use rorolala_storage::{
        Chunking, Key, Lockable as _, StorageBackend as _, StorageSet, key_of_file,
    };
    use rorolala_tree::{Staging, Tree};
    use rorolala_utils_constants::WORKSPACE_IGNORE_RULES_DIR;
    use rorolala_utils_location::{Locate as _, normalize};
    use rorolala_utils_progress::Progress;
    use rorolala_vcs::{Creator, Message, VCSIndex, VCSIndexBatch, Variant, Version};
    use rorolala_workspace::{DATA_DIR, Merging, Pending, Workspace};
    use rorolala_workspace_analyze::{
        Cache, PathRename, cache_path, entry_of, tree_diff, tree_diff_remembering, walk,
    };
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    use crate::write::request::Common;
    use crate::write::{Cancellation, WriteError};

    /// How alike two text files have to be to count as the same file moved.
    const ALIKE: f32 = 0.6;

    /// How many loose objects a recording leaves before it packs the index.
    ///
    /// What a pack costs is a rewrite of every object the index holds, so packing after every run is a
    /// run whose cost grows with the index for a saving that does not. A few hundred loose objects is
    /// when one pack is worth the rewrite; until then they are valid loose objects a later run settles.
    const LOOSE_BEFORE_PACK: usize = 256;

    /// How many packs the index may hold before a recording gathers them into one.
    ///
    /// A run writes what it makes as one pack, so a run that never gathered would leave a pack per run
    /// behind, and every lookup walks every pack. A handful is what a run of them is worth; past that the
    /// walk is the reading rather than the lookup, and one rewrite is cheaper than it.
    const PACKS_BEFORE_REPACK: usize = 8;

    /// What a batch recording may do beside recording what changed.
    // The flags are independent answers a caller gave: whether another account's file is recorded
    // anyway, whether the record is amended rather than added to, whether an ignored file may be named,
    // and whether the words of the version being replaced are kept. Making each a two-variant enum would
    // say no more than the bool does while writing every use of it out twice.
    #[derive(Debug, Clone, Copy, Default)]
    #[allow(
        clippy::struct_excessive_bools,
        reason = "each flag is an independent choice a caller makes about one recording"
    )]
    pub struct Options {
        /// Record a file another account holds in the Vault's fetched copy anyway.
        pub force: bool,
        /// Replace each file's version rather than adding one after it.
        pub replace: bool,
        /// Record a file the ignore rules govern when the run names it.
        pub include_ignored: bool,
        /// Carry the words of the version being replaced, since the run gave none of its own.
        pub keep_words: bool,
        /// The cut every file this run writes is to be made with, by its registered name.
        pub policy: Option<&'static str>,
    }

    /// The words a batch recording draws itself with.
    ///
    /// The library names no progress of its own: what a bar says is the caller's language, and a bar the
    /// library titled would be the one place a command could not translate.
    #[derive(Debug, Clone)]
    pub struct Messages {
        /// What reading every candidate file is drawn under.
        pub enumerating: String,
        /// What writing their content is drawn under.
        pub writing: String,
        /// What making their versions is drawn under.
        pub recording: String,
    }

    impl Default for Messages {
        fn default() -> Self {
            Self {
                enumerating: "reading the files".to_owned(),
                writing: "writing the content".to_owned(),
                recording: "recording".to_owned(),
            }
        }
    }

    /// What one batch recording works over.
    pub struct Carry<'a> {
        /// The Workspace being worked in.
        pub workspace: &'a Workspace,
        /// The Layout being worked in.
        pub layout: &'a Layout,
        /// The stores a file's content is written to.
        pub store: &'a StorageSet,
        /// The index the versions are written to.
        pub index: &'a VCSIndex,
        /// The account the versions are made as.
        pub creator: &'a str,
        /// The ignore rules the tree is read with.
        pub rules: &'a IgnoreRuleSet,
        /// The words the run is drawn with.
        pub messages: &'a Messages,
    }

    /// Why a batch recording will not do what it was asked.
    ///
    /// A refusal is a decision, not a failure: the command turns one into its own words and exit code,
    /// while [`WriteError`] is what the library could not do at all.
    #[derive(Debug, Clone)]
    pub enum Refusal {
        /// A name is not a file or a directory inside the Workspace.
        File {
            /// The name as it was written.
            path: String,
            /// What is wrong with it.
            kind: FileKind,
        },
        /// The names are ones the ignore rules govern, so they cannot be selected.
        Ignored {
            /// What was named, as it was written.
            paths: Vec<String>,
        },
        /// A file's recorded chunking preference names a policy this build does not carry.
        Preference {
            /// The path whose preference it is.
            path: String,
            /// The name the preference holds.
            name: String,
        },
        /// The Layout names paths the tree does not hold.
        Lost {
            /// The paths the Layout names and the tree does not hold.
            paths: Vec<String>,
        },
        /// The changed files are ones the Vault's fetched copy says another account holds.
        Unowned {
            /// The paths that are not this account's to record.
            paths: Vec<String>,
        },
        /// A file `--replace` names has no version the Layout records.
        Unrecorded {
            /// The path named.
            path: String,
        },
        /// The version `--replace` names is a merge, and replacing it would drop what was joined in.
        Merge {
            /// The path named.
            path: String,
        },
        /// A variant is waiting to be joined into the file, and an amend would drop that join.
        Merging {
            /// The path named.
            path: String,
        },
        /// A variant file no longer holds the variant's own content.
        MergeChanged {
            /// The variant file that changed.
            path: String,
        },
        /// A tree cannot be recorded with `--replace` or `--policy`.
        TreeUnsupported,
        /// A tree has a conflict that was never settled.
        TreeConflict {
            /// The tree.
            path: String,
        },
        /// A Layout change names a path that is already there.
        LayoutExists,
        /// A Layout change names a path that is not there.
        LayoutMissing,
        /// The Layout failed for another reason.
        LayoutFailed {
            /// What the Layout said.
            cause: String,
        },
    }

    /// The ways a name is not a file a run may record.
    #[derive(Debug, Clone, Copy)]
    pub enum FileKind {
        /// It is not a file.
        NotAFile,
        /// It is not inside the Workspace.
        OutsideWorkspace,
        /// It is part of the Workspace's own data rather than its work.
        InsideData,
    }

    /// One thing a batch recording asks a message for: a file, or a tree taken as one file.
    #[derive(Debug, Clone)]
    pub struct Subject {
        /// Whether the subject is a tree's structure rather than a file's content.
        pub tree: bool,
        /// The key the editing file names the subject's section by.
        pub key: String,
        /// The path as a person knows it.
        pub path: String,
        /// The digest of the content the subject was read from, as lowercase hex, or empty for a
        /// tree.
        ///
        /// A file's digest is the content key its bytes hash to, so an apply can tell whether the
        /// content it planned is still the content on disk. A tree is not content a key names: what
        /// it records is its staged structure and the description waiting beside it, and there is
        /// no staging read here that would speak for both, so the field is empty rather than made
        /// to mean less than it looks.
        pub digest: String,
    }

    /// What happened to one subject a run recorded.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Action {
        /// It was not recorded before, and now is.
        Added,
        /// It was recorded, and its content changed.
        Updated,
        /// The version it was at was replaced by a new one.
        Replaced,
    }

    /// One subject a run recorded, as the command reports it.
    #[derive(Debug, Clone)]
    pub struct Item {
        /// Where it sits in the Layout.
        pub path: String,
        /// What happened to it.
        pub action: Action,
    }

    /// What carrying a plan out came to.
    #[derive(Debug, Clone, Default)]
    pub struct Outcome {
        /// One item per subject recorded, in the order the run named them.
        pub items: Vec<Item>,
        /// What stopped the run after the plan was made, one refusal at most.
        pub refused: Vec<Refusal>,
    }

    /// What a batch recording found and means to write.
    ///
    /// A plan is what a caller reads to say what each subject did: it lists the subjects a message is
    /// asked for, in the order the run named their paths, and the files behind them are only the plan's
    /// own business, which is why they are not exposed.
    pub struct Plan {
        /// The files whose content is to be recorded, in the order the run named their paths.
        files: Vec<Changed>,
        /// The trees whose staged work is to be recorded.
        trees: Vec<TreeChange>,
        /// The subjects a message is asked for, in the order the run named them.
        subjects: Vec<Subject>,
        /// The words a replacing run keeps, one per subject.
        prefill: Vec<String>,
        /// How many files hold what the Layout already names.
        unchanged: usize,
        /// What stops the run before anything is written, when anything does.
        refused: Option<Refusal>,
    }

    impl Plan {
        /// A plan that stops before it read anything.
        const fn stopped(refusal: Refusal) -> Self {
            Self {
                files: Vec::new(),
                trees: Vec::new(),
                subjects: Vec::new(),
                prefill: Vec::new(),
                unchanged: 0,
                refused: Some(refusal),
            }
        }

        /// What stops the run, when anything does.
        #[must_use]
        pub const fn refused(&self) -> Option<&Refusal> {
            self.refused.as_ref()
        }

        /// The subjects a message is asked for, in the order the run named them.
        #[must_use]
        pub fn subjects(&self) -> &[Subject] {
            &self.subjects
        }

        /// What each subject is to be asked with, one per subject.
        ///
        /// A replacing run that gave no words of its own opens on the words the version being replaced
        /// had; every other run opens on nothing, since it has nothing to keep.
        #[must_use]
        pub fn prefill(&self) -> &[String] {
            &self.prefill
        }

        /// How many files are to be recorded.
        #[must_use]
        pub const fn changed(&self) -> usize {
            self.files.len()
        }

        /// How many files hold what the Layout already names.
        #[must_use]
        pub const fn unchanged(&self) -> usize {
            self.unchanged
        }
    }

    /// One file whose content is to be recorded.
    struct Changed {
        /// Which of the run's names brought this file in, so messages are asked in the run's order.
        named_at: usize,
        /// Where it sits in the Layout.
        path: LayoutPath,
        /// Where it sits on disk.
        disk: PathBuf,
        /// The digest of its content as `enumerate` read it, as lowercase hex.
        ///
        /// The reading that says whether it changed is the reading that hashes it, so the digest is
        /// kept there rather than asked of the file a second time.
        digest: String,
        /// The `Uuid` it is known by: the one it holds, or the one it is about to be given.
        id: Uuid,
        /// What the Layout holds for it, when it holds anything.
        data: Option<MutableData>,
        /// The cut it is to be written with, by its registered name, when one was settled.
        policy: Option<&'static str>,
        /// Where the reading found it, when naming it confirmed a move.
        moved_from: Option<LayoutPath>,
        /// The version it is at now, when it was recorded before.
        recorded: Option<Recorded>,
    }

    /// One tree whose staged work is to be recorded.
    struct TreeChange {
        /// Which of the run's names brought this tree in.
        named_at: usize,
        /// The tree itself.
        tree: Tree,
    }

    /// One candidate file, read and weighed before anything is asked or written.
    struct Enumerated {
        /// The file itself.
        file: Changed,
        /// Whether what it holds is a change from what the Layout names.
        changed: bool,
    }

    /// One file whose content has been put into the store, and what was found about it.
    struct Staged {
        /// The file itself.
        file: Changed,
        /// The key its content is stored under.
        storage: Key,
    }

    /// A file's recorded version, and what its variant holds.
    ///
    /// The variant is kept whole rather than only the base hash because amending reads the base from it
    /// and writes a variant of its own without needing the version object it was made from.
    struct Recorded {
        /// The version the Layout names now.
        version: Version,
        /// The variant that version points at.
        variant: Variant,
        /// The key the version's content is stored under.
        storage: [u8; 32],
    }

    /// Why an internal step stopped: a refusal the command words, or a write that failed.
    enum Stop {
        /// The run cannot be done as asked, for a reason the command words.
        Refused(Refusal),
        /// The run stopped for a reason the library reports.
        Failed(WriteError),
    }

    impl From<Refusal> for Stop {
        fn from(refusal: Refusal) -> Self {
            Self::Refused(refusal)
        }
    }

    impl From<WriteError> for Stop {
        fn from(error: WriteError) -> Self {
            Self::Failed(error)
        }
    }

    /// Reads what a plan is made of, and keeps the refusal the command is to word.
    fn plan_run(
        carry: &Carry<'_>,
        cwd: &Path,
        names: &[String],
        options: Options,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Plan, WriteError> {
        match planned(carry, cwd, names, options, progress, cancellation) {
            Ok(plan) => Ok(plan),
            Err(Stop::Refused(refusal)) => Ok(Plan::stopped(refusal)),
            Err(Stop::Failed(error)) => Err(error),
        }
    }

    /// The order is the plan: the gates a run may refuse whole are read before any candidate is hashed,
    /// and the trees are read after the files because a tree is one unit of work recorded after them.
    #[allow(
        clippy::too_many_lines,
        reason = "the plan is one reading of every gate in the order the gates have to be read"
    )]
    fn planned(
        carry: &Carry<'_>,
        cwd: &Path,
        names: &[String],
        options: Options,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Plan, Stop> {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled.into());
        }

        let root = normalize(carry.workspace.get_root());

        // A variant file is content waiting to be joined rather than work to record, so where it lies is
        // left out of every reading here, and a file waiting for one is recorded as the merge it is.
        let merging = Merging::read(&root);
        let ignored = merging.ignored(carry.layout);

        // The reading is the one that does not write: a plan is read before the editor is opened, so it
        // must leave the cache as it found it, and what the run does record is written down by `apply`.
        let diff =
            tree_diff(carry.layout, carry.workspace, ALIKE, carry.rules).map_err(|error| {
                WriteError::Layout {
                    reason: error.to_string(),
                }
            })?;

        // Recording works on a tree that is settled: a path the Layout names and the tree does not hold
        // is something to settle first, with `rola align`, rather than something to record around. A
        // move is not one of these, naming a path where it is now is how a move is confirmed.
        if !diff.lost.is_empty() {
            return Err(Refusal::Lost {
                paths: diff
                    .lost
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect(),
            }
            .into());
        }

        // A path that is a tree, or inside one, is recorded as that tree rather than as the files under
        // it; what is left is what prepares as files.
        let file_names: Vec<(usize, String)> = names
            .iter()
            .enumerate()
            .filter(|(_, given)| !names_tree(&root, cwd, given))
            .map(|(at, given)| (at, given.clone()))
            .collect();

        let prepared = prepare(
            carry.layout,
            &root,
            cwd,
            &file_names,
            &diff.renamed,
            &ignored,
            carry.rules,
            options.include_ignored,
            options.policy,
            cancellation,
        )?;

        // A file the Vault's fetched copy says another account holds is not one to record: the version
        // made here is one this side cannot push, so recording it makes the conflict rather than
        // avoiding it. Only what the tree says changed is asked about, a file that did not change records
        // nothing, and a Vault that tracks nothing, a copy that is not there and a file the copy does not
        // name are all answered with nothing to refuse.
        if !options.force {
            let tracked = crate::status::tracked_vault(carry.workspace);
            let unowned = crate::status::unowned(
                carry.workspace,
                carry.layout,
                tracked.as_deref(),
                Some(carry.creator),
                &diff,
            )
            .map_err(|error| WriteError::Layout {
                reason: error.to_string(),
            })?;
            let selected: BTreeSet<&str> = prepared.iter().map(|file| file.path.as_str()).collect();
            let blocked: Vec<String> = unowned
                .into_iter()
                .filter(|path| selected.contains(path.as_str()))
                .collect();

            if !blocked.is_empty() {
                return Err(Refusal::Unowned { paths: blocked }.into());
            }
        }

        // What is worth reading is what the tree says changed, plus what `--replace` names and what the
        // reading could not look at: a file the Layout agrees with holds content the store already has, so
        // it is not read at all.
        let recordable: BTreeSet<LayoutPath> = diff
            .modified
            .iter()
            .chain(diff.untagged.iter())
            .cloned()
            .collect();

        // A file the reading could not look at is not one it can say did not change, so it is read again
        // here rather than skipped: what it then finds is either its content recorded or the failure.
        let unreadable: BTreeSet<&str> = diff
            .failed
            .iter()
            .map(|failed| failed.path.as_str())
            .collect();

        let before = prepared.len();
        let candidates: Vec<Changed> = prepared
            .into_iter()
            .filter(|file| {
                options.replace
                    || recordable.contains(&file.path)
                    || unreadable.contains(file.path.as_str())
            })
            .collect();
        let mut unchanged = before.saturating_sub(candidates.len());

        let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        let enumerated = enumerate(
            carry.index,
            &runtime,
            progress,
            carry.messages,
            candidates,
            options.replace,
            cancellation,
        )?;

        let mut changing = Vec::new();
        for file in enumerated {
            if file.changed {
                changing.push(file.file);
            } else {
                unchanged += 1;
            }
        }

        // A tree named, or standing under a directory named, is one subject of the recording, taken as
        // one file: a file's own message and the group's cover it the same way. One with nothing a
        // commit would record is not a subject at all, the way a file that did not change is not.
        let mut trees = Vec::new();
        for tree in trees_named(names, carry.workspace, carry.layout, cwd, cancellation)? {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }
            if tree_needs_commit(&tree.tree, carry.layout, carry.index, carry.store, &runtime)? {
                trees.push(tree);
            }
        }

        // A tree has no content to cut and no version to amend, so the flags that act on those are not
        // a tree's to carry: a run naming one is told so before anything is asked or written.
        if (options.replace || options.policy.is_some()) && !trees.is_empty() {
            return Err(Refusal::TreeUnsupported.into());
        }

        let subjects = subjects_for(&changing, &trees, names.len());

        // An amend with no new words keeps the ones the replaced version said, so they are what the
        // editor opens on and what a run with no editor records as they stand. They are read from the
        // index only in that case, so a run that says what it wants never pays for them.
        let prefill = if options.replace && options.keep_words {
            replaced_messages(carry.index, &runtime, &changing, cancellation)?
        } else {
            vec![String::new(); subjects.len()]
        };

        Ok(Plan {
            files: changing,
            trees,
            subjects,
            prefill,
            unchanged,
            refused: None,
        })
    }

    /// Resolves each name against the Layout and the tree, or refuses.
    ///
    /// A name is written the way a shell writes one, relative to where the run was made or absolute, and
    /// what a Layout names is a path relative to the Workspace root, so one becomes the other here. A
    /// directory is taken recursively through the same walk the tree reading uses, so what the Workspace
    /// keeps for itself and what another Workspace holds are left out.
    ///
    /// Each file's cut is settled here as well: the run's `policy` if it named one, else the preference
    /// the entry already has, else nothing. Settling it before anything is asked or written is what lets
    /// a preference this build cannot honour stop the run with the Workspace untouched.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "preparing a name needs the Layout, the readings it is judged by, the rules and the cut, and the steps are one pass over the names"
    )]
    fn prepare(
        layout: &Layout,
        root: &Path,
        cwd: &Path,
        names: &[(usize, String)],
        renames: &[PathRename],
        ignored: &BTreeSet<LayoutPath>,
        rules: &IgnoreRuleSet,
        include_ignored: bool,
        policy: Option<&'static str>,
        cancellation: &Cancellation,
    ) -> Result<Vec<Changed>, Stop> {
        // What each name stands for: a file, or a directory whose files are taken in its place.
        let mut standing = Vec::with_capacity(names.len());

        // What a name was refused for being the ignore rules' business, named as it was written.
        let mut refused = Vec::new();

        for &(at, ref given) in names {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            let disk = normalize(&resolve(cwd, given));

            if !disk.exists() {
                return Err(Refusal::File {
                    path: given.clone(),
                    kind: FileKind::NotAFile,
                }
                .into());
            }

            let Ok(relative) = disk.strip_prefix(root) else {
                return Err(Refusal::File {
                    path: given.clone(),
                    kind: FileKind::OutsideWorkspace,
                }
                .into());
            };

            if disk.is_dir() {
                // A directory the rules ignore is not this run's to record either: what it holds is
                // reached by the same walk the tree reading uses, and the rules govern what that gains.
                if !include_ignored && rules.is_ignore(relative, true) {
                    refused.push(given.clone());
                    continue;
                }

                // A directory is read where it is rather than by walking the whole Workspace and
                // filtering, so naming `src/` reads `src/` alone.
                check_directory(relative, given)?;
                standing.push(Named::Directory(disk, at));
            } else if disk.is_file() {
                // A variant file is content waiting to be joined rather than work to record, so a run
                // that names one is asking for nothing and is answered with nothing.
                let path = file_path(relative, given)?;
                if ignored.contains(&path) {
                    continue;
                }

                // An ignored file cannot be selected, but a move's destination is the work's own file
                // wearing a new place, and a rule is not written against that.
                if !include_ignored && !selectable(rules, &path, renames) {
                    refused.push(given.clone());
                    continue;
                }

                standing.push(Named::File(path, disk, at));
            } else {
                return Err(Refusal::File {
                    path: given.clone(),
                    kind: FileKind::NotAFile,
                }
                .into());
            }
        }

        if !refused.is_empty() {
            return Err(Refusal::Ignored { paths: refused }.into());
        }

        // A directory is read through the same walk the tree reading uses, so what it leaves out, the
        // Workspace's own `.rola`, a Workspace nested inside it, a symbolic link, is left out here too,
        // and so is every variant file among what it found.
        let mut files: Vec<(usize, LayoutPath, PathBuf)> = Vec::new();
        for name in standing {
            match name {
                Named::File(path, disk, at) => files.push((at, path, disk)),
                Named::Directory(disk, at) => {
                    // A walk started inside the tree does not see the directories above it, so whether
                    // the whole-tree walk would have descended into this one is told here; a directory
                    // it would not have reached holds work that is not this Workspace's.
                    if !reachable(root, &disk) {
                        continue;
                    }

                    let mut under: Vec<(usize, LayoutPath, PathBuf)> = walk(&disk)
                        .0
                        .into_iter()
                        // A directory that is a tree is one unit of work, not a directory of files, so
                        // expanding a directory over it turns it into one entry elsewhere.
                        .filter(|found| found.kind == EntryKind::File)
                        .filter_map(|found| {
                            let relative = found.disk.strip_prefix(root).ok()?;
                            let path = LayoutPath::from_relative(relative).ok()?;

                            if ignored.contains(&path) {
                                return None;
                            }

                            // What the rules ignore is not selected by a directory's expansion either:
                            // it was never named, so there is nothing to refuse, it is simply left out.
                            if !include_ignored && !selectable(rules, &path, renames) {
                                return None;
                            }

                            Some((at, path, found.disk))
                        })
                        .collect();
                    under.sort_by(|left, right| left.1.cmp(&right.1));
                    files.extend(under);
                }
            }
        }

        // A file named twice (under a directory that was named as well, or as itself) is one file, so it
        // is recorded once.
        let mut seen = BTreeSet::new();
        files.retain(|(_, path, _)| seen.insert(path.clone()));

        let mut prepared = Vec::with_capacity(files.len());

        for (named_at, path, disk) in files {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            // A path the tree moved still has its entry under where it was, so the entry is read there.
            // The move itself is made only as the file is recorded, a move the file did not change under
            // is not recorded at all, and is left to `rola align`, so a run that is refused before then,
            // for a message it could not be given, has changed nothing.
            let moved_from = renames
                .iter()
                .find(|rename| rename.to == path)
                .map(|rename| rename.from.clone());
            let source = moved_from.as_ref().unwrap_or(&path);

            let (id, data) = match layout
                .id_of(source)
                .and_then(|id| layout.entry(id).map(|data| (id, data)))
            {
                Some((id, data)) => (id, Some(data)),
                None => (Uuid::new_v4(), None),
            };

            let chosen = settle(policy, data.as_ref(), path.as_str())?;

            prepared.push(Changed {
                named_at,
                path,
                disk,
                digest: String::new(),
                id,
                data,
                policy: chosen,
                moved_from,
                recorded: None,
            });
        }

        Ok(prepared)
    }

    /// The cut one file is to be written with: the run's, else the entry's preference, else none.
    ///
    /// A preference this build cannot honour is a refusal rather than a fall back. Falling back would
    /// write the file cut by something other than what it asks for, which is not a smaller failure than
    /// stopping: it is a write that does not deduplicate against what is already stored for the file,
    /// made silently, and discovered only when someone measures the store months later.
    fn settle(
        policy: Option<&'static str>,
        data: Option<&MutableData>,
        path: &str,
    ) -> Result<Option<&'static str>, Stop> {
        if let Some(name) = policy {
            return Ok(Some(name));
        }

        let Some(name) = data.and_then(MutableData::preference) else {
            return Ok(None);
        };

        rorolala_storage::policy(name).map_or_else(
            || {
                Err(Refusal::Preference {
                    path: path.to_owned(),
                    name: name.to_owned(),
                }
                .into())
            },
            |found| Ok(Some(found.name)),
        )
    }

    /// What one name on the command line stands for.
    enum Named {
        /// A file, where it sits on disk, and which name brought it in.
        File(LayoutPath, PathBuf, usize),
        /// A directory, where it sits on disk, and which name brought it in.
        Directory(PathBuf, usize),
    }

    /// The path `name` names, made absolute against the directory the run was made in.
    fn resolve(cwd: &Path, name: &str) -> PathBuf {
        if Path::new(name).is_absolute() {
            PathBuf::from(name)
        } else {
            cwd.join(name)
        }
    }

    /// Refuses a directory that cannot be named in the Layout: one outside the Workspace, or one that is
    /// part of the Workspace's own data. The Workspace root itself names nothing and is always allowed.
    fn check_directory(relative: &Path, given: &str) -> Result<(), Stop> {
        if relative.as_os_str().is_empty() {
            return Ok(());
        }

        file_path(relative, given)?;

        Ok(())
    }

    /// The path `relative` names, or the refusal for why it cannot be named in the Layout.
    fn file_path(relative: &Path, given: &str) -> Result<LayoutPath, Stop> {
        let Ok(path) = LayoutPath::from_relative(relative) else {
            return Err(Refusal::File {
                path: given.to_owned(),
                kind: FileKind::OutsideWorkspace,
            }
            .into());
        };

        if path.as_str().split('/').any(|part| part == DATA_DIR) {
            return Err(Refusal::File {
                path: given.to_owned(),
                kind: FileKind::InsideData,
            }
            .into());
        }

        Ok(path)
    }

    /// Whether the whole-tree walk would have descended into `dir`.
    ///
    /// The walk never descends into a directory that keeps a data directory of its own, that directory
    /// is a Workspace of its own, and a walk started inside the tree does not see the directories above
    /// it. So whether any of them is such a Workspace is asked here; a directory under one holds work
    /// that is not this Workspace's to record.
    fn reachable(root: &Path, dir: &Path) -> bool {
        let mut at = Some(dir);

        while let Some(current) = at {
            if current == root {
                return true;
            }
            if current.join(DATA_DIR).is_dir() {
                return false;
            }

            at = current.parent();
        }

        false
    }

    /// Whether `path` may be selected by a run reading with `rules`: not ignored, or a move's
    /// destination, which a rule is not written against.
    fn selectable(rules: &IgnoreRuleSet, path: &LayoutPath, renames: &[PathRename]) -> bool {
        if renames.iter().any(|rename| rename.to == *path) {
            return true;
        }

        !rules.is_ignore(Path::new(path.as_str()), false)
    }

    /// Whether the path `given` names is a tree, or is inside one.
    fn names_tree(root: &Path, cwd: &Path, given: &str) -> bool {
        Tree::locate(root, &resolve(cwd, given)).is_ok()
    }

    /// Reads every candidate file and says which of them changed, before anything is asked or written.
    ///
    /// The content is read here, the whole of every candidate, because whether a file is worth a message
    /// and a version is whether what is on disk differs from what the Layout names, and that is only
    /// known by hashing it. The bar is drawn over that reading, since it is the part a reader waits
    /// through.
    ///
    /// # Errors
    ///
    /// Returns a [`Refusal`] when `--replace` names a file the Layout records no version for or one whose
    /// version is a merge, and [`WriteError::Storage`] or [`WriteError::Index`] when a file or a version
    /// cannot be read.
    fn enumerate(
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
        progress: &Progress,
        messages: &Messages,
        candidates: Vec<Changed>,
        replace: bool,
        cancellation: &Cancellation,
    ) -> Result<Vec<Enumerated>, Stop> {
        let bytes: u64 = candidates.iter().map(|file| size_of(&file.disk)).sum();
        let mut task = (!candidates.is_empty())
            .then(|| progress.begin(messages.enumerating.clone(), Some(bytes)));

        let mut enumerated = Vec::with_capacity(candidates.len());

        for mut file in candidates {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            let size = size_of(&file.disk);

            let recorded = match &file.data {
                Some(data) => Some(read_recorded(index, runtime, data.version())?),
                None => None,
            };

            if replace {
                let Some(recorded) = &recorded else {
                    return Err(Refusal::Unrecorded {
                        path: file.path.as_str().to_owned(),
                    }
                    .into());
                };

                // Replacing a merge would drop what was joined in: a version has one base, and a merge
                // is the record of there being two.
                if recorded.variant.join().is_some() {
                    return Err(Refusal::Merge {
                        path: file.path.as_str().to_owned(),
                    }
                    .into());
                }
            }

            let key = key_of_file(&file.disk).map_err(|error| WriteError::Storage {
                reason: error.to_string(),
            })?;
            file.digest = key.hex();
            let changed = replace
                || recorded
                    .as_ref()
                    .is_none_or(|recorded| recorded.storage != *key.digest());

            if let Some(task) = &mut task {
                task.advance_by(size);
            }

            file.recorded = recorded;
            enumerated.push(Enumerated { file, changed });
        }

        Ok(enumerated)
    }

    /// How large a file is, or nothing when it cannot be looked at.
    ///
    /// Used for the bars alone, which are measured in bytes: a size that cannot be read is nothing to
    /// draw rather than a failure, since the write that follows reports the failure in its own words.
    fn size_of(path: &Path) -> u64 {
        std::fs::metadata(path).map_or(0, |data| data.len())
    }

    /// Puts the content of every file that is to be recorded into the store.
    ///
    /// These are the files [`enumerate`] found changed, so nothing else is written: a file whose content
    /// the store already has has nothing to put there. Each is read again, which is what makes this a
    /// pass of its own and what its bar is drawn over, the price of having asked for the message before
    /// writing.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Storage`] when the store refuses to write a file, and
    /// [`WriteError::Cancelled`] when the caller asked the run to stop.
    fn write(
        store: &StorageSet,
        runtime: &tokio::runtime::Runtime,
        progress: &Progress,
        messages: &Messages,
        changing: Vec<Changed>,
        cancellation: &Cancellation,
    ) -> Result<Vec<Staged>, Stop> {
        let bytes: u64 = changing.iter().map(|file| size_of(&file.disk)).sum();
        let mut task =
            (!changing.is_empty()).then(|| progress.begin(messages.writing.clone(), Some(bytes)));

        let mut staged = Vec::with_capacity(changing.len());

        for file in changing {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            let size = size_of(&file.disk);

            // What a file is cut with is settled per file, so the write is told which cut to
            // make. A file nothing settled is written by the store's own choosing.
            let cut = file.policy.map(Chunking::Policy);

            let storage = runtime
                .block_on(store.put_file(&file.disk, cut, None))
                .map_err(|error| WriteError::Storage {
                    reason: error.to_string(),
                })?;

            if let Some(task) = &mut task {
                task.advance_by(size);
            }

            staged.push(Staged { file, storage });
        }

        Ok(staged)
    }

    /// Carries a plan out, keeping the refusal the command is to word.
    fn apply_run(
        carry: &Carry<'_>,
        plan: Plan,
        messages: &[String],
        options: Options,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Outcome, WriteError> {
        match applied(carry, plan, messages, options, progress, cancellation) {
            Ok(outcome) => Ok(outcome),
            Err(Stop::Refused(refusal)) => Ok(Outcome {
                items: Vec::new(),
                refused: vec![refusal],
            }),
            Err(Stop::Failed(error)) => Err(error),
        }
    }

    /// Carries a plan out, keeping the refusal the command is to word.
    /// The order is the plan: content before the index that names it, the index before the Layout, and
    /// the trees after every file, because a tree is one unit of work recorded as a version of its own.
    #[allow(
        clippy::too_many_lines,
        reason = "the apply is one sequence of writes whose order is the whole of what it does"
    )]
    fn applied(
        carry: &Carry<'_>,
        plan: Plan,
        messages: &[String],
        options: Options,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Outcome, Stop> {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled.into());
        }

        let Plan {
            files,
            trees,
            subjects,
            prefill: _,
            unchanged: _,
            refused,
        } = plan;

        if let Some(refusal) = refused {
            return Err(refusal.into());
        }

        // The caller is the one that pairs a message with a subject, so a list that does not is a caller
        // bug rather than a refusal the Workspace can act on.
        if messages.len() != subjects.len() {
            return Err(WriteError::Internal {
                reason: "the messages do not line up with the subjects".to_owned(),
            }
            .into());
        }

        let root = normalize(carry.workspace.get_root());
        let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        // What a recording says about a merge is read again here rather than carried from the plan: a
        // variant may have been brought in between the two, and the apply is what has to agree with the
        // workspace it is writing into.
        let mut merging = Merging::read(&root);

        // The messages split back to what each makes: a version of a file's content, or a version of a
        // tree's structure. The positions are kept so the run reports what it did in the order it was
        // asked, whatever order the writes then take.
        let mut file_texts: Vec<&str> = Vec::new();
        let mut file_slots: Vec<usize> = Vec::new();
        let mut tree_texts: Vec<&str> = Vec::new();
        let mut tree_slots: Vec<usize> = Vec::new();

        for (at, subject) in subjects.iter().enumerate() {
            if subject.tree {
                tree_texts.push(messages[at].as_str());
                tree_slots.push(at);
            } else {
                file_texts.push(messages[at].as_str());
                file_slots.push(at);
            }
        }

        let mut item_slots: Vec<Option<Item>> = Vec::with_capacity(subjects.len());
        item_slots.resize_with(subjects.len(), || None);

        let staged = write(
            carry.store,
            &runtime,
            progress,
            carry.messages,
            files,
            cancellation,
        )?;

        let mut committed = Vec::with_capacity(staged.len());

        // A version is one thing made per file, so the bar for making them counts files rather than the
        // bytes the content above was measured in.
        let mut placing = (!staged.is_empty()).then(|| {
            progress.begin(
                carry.messages.recording.clone(),
                Some(u64::try_from(staged.len()).unwrap_or(u64::MAX)),
            )
        });

        // What the run does to the Layout is gathered up and written when it is done, so a run that
        // records many files pays one read of what others wrote and one flushed write per shard rather
        // than the same for each file. The index is gathered the same way: one pack for the whole run.
        let mut batch = carry.layout.batch();
        let mut writes = carry.index.batch();

        for (at, file) in staged.iter().enumerate() {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            let text = file_texts.get(at).copied().unwrap_or_default();

            // A file a variant is waiting for is recorded as the merge it is, and the waiting is over
            // once it is: what the merge was is read here, where the version is made.
            let pending = merging.at(file.file.id);

            commit(
                carry.layout,
                carry.index,
                &runtime,
                carry.creator,
                &mut writes,
                &mut batch,
                file,
                text,
                options.replace,
                pending,
                &merging,
                &root,
            )?;

            if let Some(pending) = pending {
                if let Some(path) = merging.variant_path(carry.layout, pending) {
                    let disk = root.join(path.to_path_buf());

                    // A variant file that is already gone is one the merge can still be recorded from:
                    // the content that was brought in is what the version names, and the file was only
                    // how it was shown.
                    if disk.is_file() {
                        std::fs::remove_file(&disk).map_err(|error| WriteError::Internal {
                            reason: error.to_string(),
                        })?;
                    }
                }

                merging.remove(file.file.id);
            }

            committed.push((
                file.file.path.clone(),
                file.file.disk.clone(),
                *file.storage.digest(),
            ));
            item_slots[file_slots[at]] = Some(Item {
                path: file.file.path.as_str().to_owned(),
                action: if options.replace {
                    Action::Replaced
                } else if file.file.data.is_some() {
                    Action::Updated
                } else {
                    Action::Added
                },
            });

            if let Some(task) = &mut placing {
                task.advance_by(1);
            }
        }

        // What the run made in the index is written before the Layout that names it: a Layout naming a
        // version the index does not hold is a version nothing reads, so the objects are made first and
        // the Layout follows them. One pack for the whole run, and nothing the run wrote is left loose.
        runtime
            .block_on(writes.publish())
            .map_err(|error| WriteError::Internal {
                reason: error.to_string(),
            })?;

        // What was recorded is what the tree and the Layout now agree on, so the reading is told so
        // before the whole cache is written below: a path recorded here must keep the digest just
        // written, not the one the cache held before the file changed.
        batch
            .commit()
            .map_err(|error| Stop::Refused(layout_refusal(error)))?;

        drop(placing);

        remember(carry.layout, &root, &committed)?;

        merging.write(&root).map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        // The reading this run made is what the next reading starts from, so it is written here, in
        // the write: a plan writes nothing, and a file it saw but did not record, because the Layout
        // already agreed with it, still has to be remembered for a later reading to recognise where
        // it went. The agreement written above is what the whole-tree reading carries through, so a
        // path recorded here keeps the digest just recorded rather than the one before the change.
        tree_diff_remembering(carry.layout, carry.workspace, ALIKE, carry.rules).map_err(
            |error| WriteError::Internal {
                reason: error.to_string(),
            },
        )?;

        // What the run wrote into the index is laid out with it, so a Workspace does not gather a loose
        // object per version it records. A run that recorded nothing has nothing to lay out.
        if !committed.is_empty() {
            pack_index(carry.index, &runtime);
        }

        // A tree is one unit of work: it is recorded after the files, since recording it writes a
        // version of its own rather than a version per file. What it records is the message its own
        // section in the same editor was asked for.
        for ((slot, tree), text) in tree_slots
            .iter()
            .zip(trees.iter())
            .zip(tree_texts.iter().copied())
        {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            let (path, added) = commit_tree(
                carry.layout,
                carry.index,
                carry.store,
                &runtime,
                carry.creator,
                &tree.tree,
                text,
                carry.rules,
                options.include_ignored,
            )?;

            item_slots[*slot] = Some(Item {
                path,
                action: if added {
                    Action::Added
                } else {
                    Action::Updated
                },
            });
        }

        let items = item_slots.into_iter().flatten().collect();

        Ok(Outcome {
            items,
            refused: Vec::new(),
        })
    }

    /// Lays the index's objects out as one pack, when this run has left enough of them to gather.
    ///
    /// A run writes what it makes as a pack already, so what gathers between packings is a pack per run
    /// rather than a loose file per object. What a gathering costs is a rewrite of every object the index
    /// holds, though, which grows with the index while what it saves does not, so a run gathers only when
    /// the loose objects it left number [`LOOSE_BEFORE_PACK`], or when the packs it left number more than
    /// [`PACKS_BEFORE_REPACK`], and until then they are valid where they are: a later run or `rola pack`
    /// settles them. It is best effort, a pack that cannot be made because another run holds the lock
    /// leaves an index that is correct and merely ungathered, and a failure here does not fail a
    /// recording that is already durable.
    fn pack_index(index: &VCSIndex, runtime: &tokio::runtime::Runtime) {
        let Ok(guard) = runtime.block_on(index.lock()) else {
            return;
        };

        let _ = runtime.block_on(guard.repack_when_due(LOOSE_BEFORE_PACK, PACKS_BEFORE_REPACK));
    }

    /// Writes down what a recording left the tree and the Layout agreeing on.
    ///
    /// The digest carried is the one just recorded, not the one the cache held before the file
    /// changed: a plan writes nothing, so the cache a run reads is from before its own reading, and
    /// holding the old digest would make the file read as modified again on the very next status.
    ///
    /// # Errors
    ///
    /// Returns a message when the cache cannot be read or written.
    fn remember(
        layout: &Layout,
        root: &Path,
        committed: &[(LayoutPath, PathBuf, [u8; 32])],
    ) -> Result<(), Stop> {
        if committed.is_empty() {
            return Ok(());
        }

        let path = cache_path(root, layout);
        let mut cache = Cache::read(&path);

        for (layout_path, disk, recorded) in committed {
            // A cached entry whose digest is already the recorded one can be taken as it stands; one
            // from before the change is read again, since only the file says what it holds now.
            let entry = match cache.get(layout_path) {
                Some(read) if read.digest() == *recorded => {
                    read.clone().with_agreed(Some(*recorded))
                }
                _ => entry_of(disk)
                    .map_err(|error| WriteError::Internal {
                        reason: error.to_string(),
                    })?
                    .with_agreed(Some(*recorded)),
            };
            cache.insert(layout_path.clone(), entry);
        }

        cache.write(&path).map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        Ok(())
    }

    /// Makes a version of the staged content and makes the Layout name it.
    ///
    /// `replace` builds the new version on what the version being replaced was built on, rather than on
    /// that version: the one that was there stands as if it had never been made, which is what an amend
    /// is. That base is carried by the replaced variant as the hash it is, so the version object it
    /// names is never read back to make the new one.
    #[allow(
        clippy::too_many_arguments,
        reason = "one version needs the file, the merge waiting for it and the batches the run gathers"
    )]
    fn commit(
        layout: &Layout,
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
        creator_name: &str,
        writes: &mut VCSIndexBatch,
        batch: &mut Batch<'_>,
        file: &Staged,
        text: &str,
        replace: bool,
        pending: Option<&Pending>,
        merging: &Merging,
        root: &Path,
    ) -> Result<(), Stop> {
        if replace && pending.is_some() {
            return Err(Refusal::Merging {
                path: file.file.path.as_str().to_owned(),
            }
            .into());
        }

        let creator =
            Creator::try_from(creator_name.to_owned()).map_err(|error| WriteError::Internal {
                reason: error.to_string(),
            })?;
        let message = Message::try_from(text.to_owned()).map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        let creator_key = writes.add(&creator);
        let message_key = writes.add(&message);

        let variant = match (&file.file.recorded, replace) {
            (Some(recorded), true) => Variant::new_bare_variant(
                *file.storage.digest(),
                *recorded.variant.base_version(),
                None,
                *creator_key.digest(),
                *message_key.digest(),
                recorded.variant.base_version_num(),
            ),
            (Some(recorded), false) => {
                let base = recorded.version.new_variant(
                    *file.storage.digest(),
                    *creator_key.digest(),
                    *message_key.digest(),
                );

                match pending {
                    None => base,
                    Some(pending) => {
                        let joined = runtime
                            .block_on(index.read(Key::new(*pending.variant())))
                            .map_err(|error| WriteError::Index {
                                reason: error.reason(),
                            })?
                            .expect_variant()
                            .map_err(|error| WriteError::Internal {
                                reason: error.to_string(),
                            })?;

                        verify_variant_file(layout, merging, root, pending, &joined)?;

                        // Content that is exactly the variant's own is no merge but a replacement: the
                        // file now holds what was brought in, and what is recorded is that, on the chain
                        // the file was already on. Only content of its own is joined.
                        if *joined.storage_hash() == *file.storage.digest() {
                            base
                        } else {
                            base.new_merge_variant(*file.storage.digest(), &joined)
                        }
                    }
                }
            }
            (None, _) => {
                let root = Version::root();
                writes.add(&root);
                root.new_variant(
                    *file.storage.digest(),
                    *creator_key.digest(),
                    *message_key.digest(),
                )
            }
        };
        let version = variant.new_version();

        writes.add(&variant);
        let version_key = writes.add(&version);

        // What the version goes on to prefer, whatever this write did: the preference is what the next
        // write is cut by, so it is recorded from what was settled rather than from what the store did.
        // A file nothing was settled for keeps saying nothing, and the next write chooses again.
        let data = MutableData::new(
            Some(creator_name.to_owned()),
            *version_key.digest(),
            String::new(),
        )
        .with_preference(file.file.policy.map(str::to_owned));

        // A moved file is brought to where it is now: the move is the last thing done, so that a failure
        // before it, writing the version the change is, leaves the Layout as it was. The variant file
        // waiting for it, if there is one, moves with it.
        if let Some(from) = &file.file.moved_from {
            batch
                .move_path(from, &file.file.path)
                .map_err(|error| Stop::Refused(layout_refusal(error)))?;

            merging
                .follow(root, file.file.id, from, &file.file.path)
                .map_err(|error| WriteError::Internal {
                    reason: error.to_string(),
                })?;
        }

        if file.file.data.is_some() {
            batch
                .update_entry(file.file.id, data)
                .map_err(|error| Stop::Refused(layout_refusal(error)))?;

            return Ok(());
        }

        batch
            .create_file(file.file.id, data, &file.file.path)
            .map_err(|error| Stop::Refused(layout_refusal(error)))?;

        Ok(())
    }

    /// Reads the variant file waiting for a file, refusing when it no longer holds the variant's content.
    ///
    /// What a recording says is that this variant was joined; a file that no longer holds what the
    /// variant names would record a merge of something else, which is worse than recording nothing.
    fn verify_variant_file(
        layout: &Layout,
        merging: &Merging,
        root: &Path,
        pending: &Pending,
        joined: &Variant,
    ) -> Result<(), Stop> {
        let Some(path) = merging.variant_path(layout, pending) else {
            return Ok(());
        };
        let disk = root.join(path.to_path_buf());

        // A variant file that is already gone is one the merge can still be recorded from: what was
        // brought in is what the version names, and the file was only how it was shown.
        if !disk.is_file() {
            return Ok(());
        }

        let entry = entry_of(&disk).map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;
        if entry.digest() != *joined.storage_hash() {
            return Err(Refusal::MergeChanged {
                path: path.as_str().to_owned(),
            }
            .into());
        }

        Ok(())
    }

    /// What each changing file's version being replaced says, read from the index.
    ///
    /// It is read only for a run that amends without writing new words, since that is the only run the
    /// words are read for.
    fn replaced_messages(
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
        files: &[Changed],
        cancellation: &Cancellation,
    ) -> Result<Vec<String>, Stop> {
        let mut messages = Vec::with_capacity(files.len());

        for file in files {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            // `enumerate` refuses a file the Layout records nothing for when `--replace` was asked for,
            // so every changing file here has a version whose words can be read.
            let Some(recorded) = file.recorded.as_ref() else {
                return Err(WriteError::Internal {
                    reason: "a replaced file has no recorded version".to_owned(),
                }
                .into());
            };

            let message = runtime
                .block_on(index.read(Key::new(*recorded.variant.message())))
                .map_err(|error| WriteError::Index {
                    reason: error.reason(),
                })?
                .expect_message()
                .map_err(|error| WriteError::Internal {
                    reason: error.to_string(),
                })?;

            messages.push(message.read_to_string().to_owned());
        }

        Ok(messages)
    }

    /// The `Version` stored under `hash`, with the variant it points at and the content key that names.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Index`] when the index cannot be read, and [`WriteError::Internal`] when what
    /// it holds under the version is not a version or a variant.
    fn read_recorded(
        index: &VCSIndex,
        runtime: &tokio::runtime::Runtime,
        hash: [u8; 32],
    ) -> Result<Recorded, Stop> {
        let version = runtime
            .block_on(index.read(Key::new(hash)))
            .map_err(|error| WriteError::Index {
                reason: error.reason(),
            })?
            .expect_version()
            .map_err(|error| WriteError::Internal {
                reason: error.to_string(),
            })?;

        let variant = runtime
            .block_on(index.read(Key::new(*version.variant())))
            .map_err(|error| WriteError::Index {
                reason: error.reason(),
            })?
            .expect_variant()
            .map_err(|error| WriteError::Internal {
                reason: error.to_string(),
            })?;

        Ok(Recorded {
            version,
            storage: *variant.storage_hash(),
            variant,
        })
    }

    /// The trees the run named, directly or under a directory it named.
    ///
    /// A tree named as a path is that tree. A directory brings every tree the Layout records under it,
    /// and every directory holding a tree mark that the workspace reading finds under it; what is under
    /// a tree is that tree's own work, so a nested tree is recorded by naming it or its tree.
    fn trees_named(
        named: &[String],
        workspace: &Workspace,
        layout: &Layout,
        cwd: &Path,
        cancellation: &Cancellation,
    ) -> Result<Vec<TreeChange>, Stop> {
        let root = workspace.get_root();
        let mut trees: BTreeMap<PathBuf, (usize, Tree)> = BTreeMap::new();

        for (at, given) in named.iter().enumerate() {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled.into());
            }

            let resolved = resolve(cwd, given);

            if let Ok(tree) = Tree::locate(root, &resolved) {
                trees
                    .entry(tree.root().to_path_buf())
                    .or_insert_with(|| (at, tree));
                continue;
            }

            if !resolved.is_dir() {
                continue;
            }

            let scope = resolved
                .strip_prefix(root)
                .ok()
                .and_then(|relative| LayoutPath::from_relative(relative).ok());

            for (id, data) in layout.entries() {
                if data.kind() != EntryKind::Tree {
                    continue;
                }
                let Some(path) = layout.path_of(id) else {
                    continue;
                };
                if !under_scope(scope.as_ref(), &path) {
                    continue;
                }

                let disk = root.join(path.to_path_buf());
                if let Ok(tree) = Tree::open(root, &disk) {
                    trees
                        .entry(tree.root().to_path_buf())
                        .or_insert_with(|| (at, tree));
                }
            }

            for found in walk(&resolved).0 {
                if found.kind != EntryKind::Tree {
                    continue;
                }
                if let Ok(tree) = Tree::open(root, &found.disk) {
                    trees
                        .entry(tree.root().to_path_buf())
                        .or_insert_with(|| (at, tree));
                }
            }
        }

        Ok(trees
            .into_values()
            .map(|(named_at, tree)| TreeChange { named_at, tree })
            .collect())
    }

    /// Whether `path` is `scope` itself or under it; no scope is the whole of the work.
    fn under_scope(scope: Option<&LayoutPath>, path: &LayoutPath) -> bool {
        let Some(scope) = scope else {
            return true;
        };

        path == scope
            || path
                .as_str()
                .strip_prefix(scope.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    }

    /// The message subjects of a run, in the order the run named its paths.
    ///
    /// A tree is one subject, the way a file is: the run names it in the same list, and the editor asks
    /// about it in the same sections. Which name brought each file in is carried from `prepare`, so a
    /// tree named between two files is asked about between them.
    fn subjects_for(changing: &[Changed], trees: &[TreeChange], names: usize) -> Vec<Subject> {
        let mut subjects = Vec::with_capacity(changing.len() + trees.len());

        for at in 0..names {
            for file in changing {
                if file.named_at == at {
                    subjects.push(Subject {
                        tree: false,
                        key: file.id.to_string(),
                        path: file.path.as_str().to_owned(),
                        digest: file.digest.clone(),
                    });
                }
            }

            for tree in trees {
                if tree.named_at == at {
                    let path = tree.tree.layout_path();
                    subjects.push(Subject {
                        tree: true,
                        key: path.as_ref().map_or_else(
                            |_| "tree".to_owned(),
                            |path| format!("tree:{}", path.as_str()),
                        ),
                        path: path.map_or_else(|_| ".".to_owned(), |path| path.as_str().to_owned()),
                        digest: String::new(),
                    });
                }
            }
        }

        subjects
    }

    /// Whether the tree has anything a commit would record.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Tree`] when the tree's record, staging area or conflicts cannot be read.
    fn tree_needs_commit(
        tree: &Tree,
        layout: &Layout,
        index: &VCSIndex,
        store: &StorageSet,
        runtime: &tokio::runtime::Runtime,
    ) -> Result<bool, Stop> {
        let recorded = runtime
            .block_on(tree.recorded(layout, index, store))
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;

        // A tree the Layout records nothing for is one whose first version is its work as it stands.
        if recorded.is_none() {
            return Ok(true);
        }

        let staging = tree.staging();
        let staged = staging.structure().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
        let description = staging.description().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
        let conflicts = staging.conflicts().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

        Ok(staged.is_some() || description.is_some() || !conflicts.is_empty())
    }

    /// Records one tree's staged structure as a version, and makes the Layout name it.
    ///
    /// It answers the tree's path and whether the version is the first one.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Tree`] when the tree, its staging area, its store or its baseline cannot be
    /// read or written, [`Refusal::TreeConflict`] when it has a conflict that was never settled, and
    /// [`WriteError::Index`] or a Layout refusal when the version cannot be written.
    // The commit is one sequence of writes that has to stay in its order: reading what is staged, taking
    // the content into the work's store, making the objects, naming the version in the Layout, and only
    // then letting the staging area go.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "a tree's commit needs everything the tree is read and written through beside its words, in one order that cannot be cut"
    )]
    fn commit_tree(
        layout: &Layout,
        index: &VCSIndex,
        store: &StorageSet,
        runtime: &tokio::runtime::Runtime,
        creator_name: &str,
        tree: &Tree,
        text: &str,
        rules: &IgnoreRuleSet,
        include_ignored: bool,
    ) -> Result<(String, bool), Stop> {
        let path = tree.layout_path().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
        let shown = shown(tree)?;

        // The id the tree keeps locally is the Layout entry's, so a reading that finds the entry lost and
        // a directory holding a tree mark elsewhere can tell they are one tree moved. An entry the Layout
        // already names has one to write before anything else, including a run that finds nothing to
        // record.
        let entry_id = layout.id_of(&path);
        if let Some(id) = entry_id
            && let Err(error) = tree.write_local_id(id)
        {
            return Err(WriteError::Tree {
                reason: error.to_string(),
            }
            .into());
        }

        let recorded = runtime
            .block_on(tree.recorded(layout, index, store))
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;

        let staging = Staging::new(tree.root());
        let staged = staging.structure().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
        let description = staging.description().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;
        let conflicts = staging.conflicts().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

        if !conflicts.is_empty() {
            return Err(Refusal::TreeConflict { path: shown }.into());
        }

        let rules = tree
            .rules(rules.clone())
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;
        let scanned = tree
            .scan(&rules, include_ignored)
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;

        // What the version holds: what was staged, or the tree as it stands when this is its first
        // version. A tree with nothing staged and a version already keeps that version.
        let structure = match staged {
            Some(staged) => staged,
            None => match &recorded {
                Some(recorded) if description.is_none() => {
                    staging.clear().map_err(|error| WriteError::Tree {
                        reason: error.to_string(),
                    })?;

                    return Ok((shown, false));
                }
                Some(recorded) => recorded.structure.clone(),
                None => scanned.structure.clone(),
            },
        };

        let description_key = match description {
            Some(text) => Some(
                runtime
                    .block_on(store.write_object(text.as_bytes(), rorolala_storage::Codec::Raw))
                    .map_err(|error| WriteError::Tree {
                        reason: error.to_string(),
                    })?,
            ),
            None => None,
        };
        let structure = structure.with_description(description_key);

        if let Some(recorded) = &recorded
            && recorded.structure == structure
        {
            staging.clear().map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;

            return Ok((shown, false));
        }

        // The staging store's content is the work's now: it is carried into the Workspace's store before
        // anything names it, so a failure leaves content nothing refers to rather than a version whose
        // content is not here.
        if let Some(staged_content) = staging.store() {
            let keys = runtime
                .block_on(staged_content.list_exist_keys())
                .map_err(|error| WriteError::Tree {
                    reason: error.to_string(),
                })?;
            runtime
                .block_on(store.write_target().publish_from(&staged_content, &keys))
                .map_err(|error| WriteError::Tree {
                    reason: error.to_string(),
                })?;
        }

        let structure_key = runtime
            .block_on(store.write_object(&structure.encode(), rorolala_storage::Codec::Raw))
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;

        let creator =
            Creator::try_from(creator_name.to_owned()).map_err(|error| WriteError::Internal {
                reason: error.to_string(),
            })?;
        let message = Message::try_from(text.to_owned()).map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        let mut writes = index.batch();
        let creator_key = writes.add(&creator);
        let message_key = writes.add(&message);

        let variant = if let Some(recorded) = &recorded {
            let version = runtime
                .block_on(index.read(Key::new(recorded.version)))
                .map_err(|error| WriteError::Index {
                    reason: error.reason(),
                })?
                .expect_version()
                .map_err(|error| WriteError::Internal {
                    reason: error.to_string(),
                })?;

            version.new_variant(
                *structure_key.digest(),
                *creator_key.digest(),
                *message_key.digest(),
            )
        } else {
            let root = Version::root();
            writes.add(&root);
            root.new_variant(
                *structure_key.digest(),
                *creator_key.digest(),
                *message_key.digest(),
            )
        };
        let version = variant.new_version();
        writes.add(&variant);
        let version_key = writes.add(&version);

        runtime
            .block_on(writes.publish())
            .map_err(|error| WriteError::Internal {
                reason: error.to_string(),
            })?;

        let data = MutableData::new(
            Some(creator_name.to_owned()),
            *version_key.digest(),
            String::new(),
        )
        .with_kind(EntryKind::Tree);

        let mut batch = layout.batch();
        let entry_id = entry_id.unwrap_or_else(Uuid::new_v4);
        if let Some(id) = layout.id_of(&path) {
            batch
                .update_entry(id, data)
                .map_err(|error| Stop::Refused(layout_refusal(error)))?;
        } else {
            batch
                .create_file(entry_id, data, &path)
                .map_err(|error| Stop::Refused(layout_refusal(error)))?;
        }
        batch
            .commit()
            .map_err(|error| Stop::Refused(layout_refusal(error)))?;

        let baseline = tree.baseline();
        baseline
            .write(&structure, version_key.digest())
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;
        baseline
            .write_entries(&scanned.cache)
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;
        tree.write_local_id(entry_id)
            .map_err(|error| WriteError::Tree {
                reason: error.to_string(),
            })?;

        staging.clear().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

        Ok((shown, recorded.is_none()))
    }

    /// The name a tree is shown by: its Layout path, with the Workspace root shown as `.`.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Tree`] when the tree's path cannot be worked out.
    fn shown(tree: &Tree) -> Result<String, Stop> {
        let path = tree.layout_path().map_err(|error| WriteError::Tree {
            reason: error.to_string(),
        })?;

        Ok(if path.is_root() {
            ".".to_owned()
        } else {
            path.as_str().to_owned()
        })
    }

    /// The refusal a Layout that would not take a change is answered with.
    fn layout_refusal(error: rorolala_layout::LayoutError) -> Refusal {
        match error {
            rorolala_layout::LayoutError::AlreadyExists => Refusal::LayoutExists,
            rorolala_layout::LayoutError::NotFound => Refusal::LayoutMissing,
            other => Refusal::LayoutFailed {
                cause: other.to_string(),
            },
        }
    }

    /// Reads every name the run gave, settles what each file is to be written with, and says what
    /// changed, without writing anything.
    ///
    /// The plan is what the editor is opened on: every gate is read, every candidate is hashed, and
    /// the trees that have work to record are found before a message is asked for, so the subjects a
    /// caller sees are the ones an apply will write. A plan whose [`Plan::refused`] is set has read
    /// everything it could but will write nothing, which is how a name that cannot be recorded leaves
    /// the Workspace as it found it.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Cancelled`] when `cancellation` was set before or between the files, and
    /// [`WriteError::Storage`], [`WriteError::Index`] or [`WriteError::Layout`] when a reading fails.
    pub fn plan(
        carry: &Carry<'_>,
        cwd: &Path,
        names: &[String],
        options: Options,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Plan, WriteError> {
        plan_run(carry, cwd, names, options, progress, cancellation)
    }

    /// Writes what a plan named, once the caller has said what each subject did.
    ///
    /// The messages come in the order [`Plan::subjects`] names them, one per subject, so a file's own
    /// words are the subject's and a tree's are the tree's. The content of every changed file goes
    /// into the store first, then the index is made to hold the versions and the Layout is made to
    /// name them, and the trees are recorded last, since a tree is one unit of work of its own.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Cancelled`] when `cancellation` was set before or between the files, and
    /// [`WriteError::Storage`], [`WriteError::Index`], [`WriteError::Layout`] or [`WriteError::Tree`]
    /// when a write fails. A refusal that the plan could not see, such as a variant file that changed
    /// between the plan and the apply, is answered in [`Outcome::refused`] instead.
    pub fn apply(
        carry: &Carry<'_>,
        plan: Plan,
        messages: &[String],
        options: Options,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Outcome, WriteError> {
        apply_run(carry, plan, messages, options, progress, cancellation)
    }

    /// A batch recording asked for as one value, so a command line and a C caller fill the same
    /// request.
    ///
    /// [`Common`] comes flattened, so the workspace, the account, the Vault, the offline flag, the
    /// force flag, the concurrency and the labels are the fields every write request carries. A
    /// relative name in `files` is read from the Workspace root, never from the process directory:
    /// a C caller has no directory the run was made in, and the Workspace it named is the only
    /// place its paths can mean anything.
    #[derive(Debug, Deserialize)]
    pub struct TrackRequest {
        /// The fields every write request carries.
        #[serde(flatten)]
        common: Common,
        /// The names to record, as the run wrote them.
        files: Vec<String>,
        /// Whether each file's version is replaced rather than added after it.
        #[serde(default)]
        replace: bool,
        /// The cut every file this run writes is to be made with, by its registered name.
        #[serde(default)]
        policy: Option<String>,
        /// Whether a file the ignore rules govern may be named anyway.
        #[serde(default)]
        include_ignored: bool,
    }

    /// One subject a plan asks a message for, as the C ABI answers it.
    ///
    /// Its serde shape is `{"tree":false,"key":"...","path":"a.txt","digest":"..."}`, and `digest`
    /// is the empty string for a tree.
    #[derive(Debug, Clone, Serialize)]
    pub struct SubjectView {
        /// Whether the subject is a tree's structure rather than a file's content.
        pub tree: bool,
        /// The key the editing file names the subject's section by.
        pub key: String,
        /// The path as a person knows it.
        pub path: String,
        /// The digest of the content the subject was read from, as lowercase hex, or empty for a
        /// tree.
        pub digest: String,
    }

    impl From<&Subject> for SubjectView {
        fn from(subject: &Subject) -> Self {
            Self {
                tree: subject.tree,
                key: subject.key.clone(),
                path: subject.path.clone(),
                digest: subject.digest.clone(),
            }
        }
    }

    /// What happened to one subject, as the C ABI answers it.
    #[derive(Debug, Clone, Copy, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ActionView {
        /// It was not recorded before, and now is.
        Added,
        /// It was recorded, and its content changed.
        Updated,
        /// The version it was at was replaced by a new one.
        Replaced,
    }

    impl From<Action> for ActionView {
        fn from(action: Action) -> Self {
            match action {
                Action::Added => Self::Added,
                Action::Updated => Self::Updated,
                Action::Replaced => Self::Replaced,
            }
        }
    }

    /// One subject a run recorded, as the C ABI answers it.
    ///
    /// Its serde shape is `{"path":"a.txt","action":"added"}`.
    #[derive(Debug, Clone, Serialize)]
    pub struct ItemView {
        /// Where it sits in the Layout.
        pub path: String,
        /// What happened to it.
        pub action: ActionView,
    }

    impl From<&Item> for ItemView {
        fn from(item: &Item) -> Self {
            Self {
                path: item.path.clone(),
                action: ActionView::from(item.action),
            }
        }
    }

    /// The ways a name is not a file a run may record, as the C ABI answers it.
    #[derive(Debug, Clone, Copy, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum FileKindView {
        /// It is not a file.
        NotAFile,
        /// It is not inside the Workspace.
        OutsideWorkspace,
        /// It is part of the Workspace's own data rather than its work.
        InsideData,
    }

    impl From<FileKind> for FileKindView {
        fn from(kind: FileKind) -> Self {
            match kind {
                FileKind::NotAFile => Self::NotAFile,
                FileKind::OutsideWorkspace => Self::OutsideWorkspace,
                FileKind::InsideData => Self::InsideData,
            }
        }
    }

    /// Why a plan or an apply refused, as the C ABI answers it.
    ///
    /// The shape is the library's own: an externally tagged object whose tag is the variant in
    /// `snake_case`, so a file refusal is `{"file":{"path":"a.txt","kind":"not_a_file"}}` and an
    /// ignored one is `{"ignored":{"paths":["a.txt"]}}`.
    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum RefusalView {
        /// A name is not a file or a directory inside the Workspace.
        File {
            /// The name as it was written.
            path: String,
            /// What is wrong with it.
            kind: FileKindView,
        },
        /// The names are ones the ignore rules govern, so they cannot be selected.
        Ignored {
            /// What was named, as it was written.
            paths: Vec<String>,
        },
        /// A file's recorded chunking preference names a policy this build does not carry.
        Preference {
            /// The path whose preference it is.
            path: String,
            /// The name the preference holds.
            name: String,
        },
        /// The Layout names paths the tree does not hold.
        Lost {
            /// The paths the Layout names and the tree does not hold.
            paths: Vec<String>,
        },
        /// The changed files are ones the Vault's fetched copy says another account holds.
        Unowned {
            /// The paths that are not this account's to record.
            paths: Vec<String>,
        },
        /// A file `replace` names has no version the Layout records.
        Unrecorded {
            /// The path named.
            path: String,
        },
        /// The version `replace` names is a merge, and replacing it would drop what was joined in.
        Merge {
            /// The path named.
            path: String,
        },
        /// A variant is waiting to be joined into the file, and an amend would drop that join.
        Merging {
            /// The path named.
            path: String,
        },
        /// A variant file no longer holds the variant's own content.
        MergeChanged {
            /// The variant file that changed.
            path: String,
        },
        /// A tree cannot be recorded with `replace` or a chunking policy.
        TreeUnsupported,
        /// A tree has a conflict that was never settled.
        TreeConflict {
            /// The tree.
            path: String,
        },
        /// A Layout change names a path that is already there.
        LayoutExists,
        /// A Layout change names a path that is not there.
        LayoutMissing,
        /// The Layout failed for another reason.
        LayoutFailed {
            /// What the Layout said.
            cause: String,
        },
    }

    impl From<&Refusal> for RefusalView {
        fn from(refusal: &Refusal) -> Self {
            match refusal {
                Refusal::File { path, kind } => Self::File {
                    path: path.clone(),
                    kind: FileKindView::from(*kind),
                },
                Refusal::Ignored { paths } => Self::Ignored {
                    paths: paths.clone(),
                },
                Refusal::Preference { path, name } => Self::Preference {
                    path: path.clone(),
                    name: name.clone(),
                },
                Refusal::Lost { paths } => Self::Lost {
                    paths: paths.clone(),
                },
                Refusal::Unowned { paths } => Self::Unowned {
                    paths: paths.clone(),
                },
                Refusal::Unrecorded { path } => Self::Unrecorded { path: path.clone() },
                Refusal::Merge { path } => Self::Merge { path: path.clone() },
                Refusal::Merging { path } => Self::Merging { path: path.clone() },
                Refusal::MergeChanged { path } => Self::MergeChanged { path: path.clone() },
                Refusal::TreeUnsupported => Self::TreeUnsupported,
                Refusal::TreeConflict { path } => Self::TreeConflict { path: path.clone() },
                Refusal::LayoutExists => Self::LayoutExists,
                Refusal::LayoutMissing => Self::LayoutMissing,
                Refusal::LayoutFailed { cause } => Self::LayoutFailed {
                    cause: cause.clone(),
                },
            }
        }
    }

    /// What a plan would record, as the C ABI answers it.
    ///
    /// Its serde shape is
    /// `{"subjects":[{"tree":false,"key":"...","path":"a.txt","digest":"..."}],"unchanged":0,"prefill":[],"refused":null}`.
    /// `refused` is the shape [`RefusalView`] writes, and is null when the plan would go ahead.
    #[derive(Debug, Clone, Serialize)]
    pub struct TrackPlanReport {
        /// The subjects a message is asked for, in the order the run named them.
        pub subjects: Vec<SubjectView>,
        /// How many files hold what the Layout already names.
        pub unchanged: usize,
        /// What each subject is to be asked with, one per subject.
        pub prefill: Vec<String>,
        /// What stops the run before anything is written, when anything does.
        pub refused: Option<RefusalView>,
    }

    /// What carrying a plan out came to, as the C ABI answers it.
    ///
    /// Its serde shape is `{"items":[{"path":"a.txt","action":"added"}],"refused":[]}`.
    #[derive(Debug, Clone, Serialize)]
    pub struct TrackApplyReport {
        /// One item per subject recorded, in the order the run named them.
        pub items: Vec<ItemView>,
        /// What stopped the run after the plan was made, one refusal at most.
        pub refused: Vec<RefusalView>,
    }

    /// One subject a plan asked a message for, as an apply reads it back.
    #[derive(Debug, Deserialize)]
    struct GivenSubject {
        /// The path the plan named.
        path: String,
        /// The digest the plan read, which the content has to still have.
        digest: String,
    }

    /// A plan an apply was handed, read only for what it promised to record.
    #[derive(Debug, Deserialize)]
    struct GivenPlan {
        /// The subjects the plan said it would record.
        subjects: Vec<GivenSubject>,
    }

    /// Whether a plan made now names the subjects a plan was handed, in the same order and with
    /// the same content.
    ///
    /// The order is compared: the messages reach the apply in the plan's subject order, so a
    /// reordered plan is a different pairing of words and content even when the set is the same.
    fn same_subjects(subjects: &[Subject], given: &[GivenSubject]) -> bool {
        subjects.len() == given.len()
            && subjects
                .iter()
                .zip(given)
                .all(|(now, then)| now.path == then.path && now.digest == then.digest)
    }

    /// What a request resolved to, and the words the run draws itself with.
    struct Prepared {
        /// The Workspace being worked in.
        workspace: Workspace,
        /// The Layout being worked in.
        layout: Layout,
        /// The stores a file's content is written to.
        store: StorageSet,
        /// The index the versions are written to.
        index: VCSIndex,
        /// The ignore rules the tree is read with.
        rules: IgnoreRuleSet,
        /// The account the versions are made as.
        creator: String,
        /// The directory a relative name is read against, which is the Workspace root.
        cwd: PathBuf,
        /// What the recording may do beside recording what changed.
        options: Options,
        /// The words the run is drawn with.
        messages: Messages,
    }

    impl Prepared {
        /// The parts a plan or an apply works through.
        fn carry(&self) -> Carry<'_> {
            Carry {
                workspace: &self.workspace,
                layout: &self.layout,
                store: &self.store,
                index: &self.index,
                creator: &self.creator,
                rules: &self.rules,
                messages: &self.messages,
            }
        }
    }

    impl TrackRequest {
        /// Reads what the request would record, and writes nothing.
        ///
        /// The reading is the one a command makes before it asks for a message, so the subjects it
        /// names are the ones an apply records. A name is read against the Workspace root, and each
        /// subject comes back with the digest of the content it was read from.
        ///
        /// # Errors
        ///
        /// Returns [`WriteError::NoWorkspace`], [`WriteError::NoLayout`], [`WriteError::NoAccount`],
        /// [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the request does not resolve,
        /// [`WriteError::Layout`] when the Layout cannot be read, [`WriteError::Precondition`] when
        /// `policy` names no cut this build carries, [`WriteError::Cancelled`] when `cancellation`
        /// was set, and [`WriteError::Storage`], [`WriteError::Index`], [`WriteError::Tree`] or
        /// [`WriteError::Internal`] when the reading fails. A decision the run makes rather than a
        /// failure is the report's `refused`.
        pub fn request_plan(
            &self,
            progress: &Progress,
            cancellation: &Cancellation,
        ) -> Result<TrackPlanReport, WriteError> {
            let prepared = self.prepare()?;
            let planned = plan(
                &prepared.carry(),
                &prepared.cwd,
                &self.files,
                prepared.options,
                progress,
                cancellation,
            )?;

            Ok(TrackPlanReport {
                subjects: planned.subjects().iter().map(SubjectView::from).collect(),
                unchanged: planned.unchanged(),
                prefill: planned.prefill().to_vec(),
                refused: planned.refused().map(RefusalView::from),
            })
        }

        /// Records what a plan named, once the caller has said what each subject did.
        ///
        /// The plan is read again first, because the Workspace may have changed since it was made:
        /// a subject that was added, removed, reordered or given other content is refused rather
        /// than recorded. `messages_json` maps each subject's `path` to the words its version
        /// carries, and a subject with no message is refused.
        ///
        /// # Errors
        ///
        /// Returns what [`request_plan`](Self::request_plan) returns, [`WriteError::Json`] when
        /// `plan_json` or `messages_json` does not read, [`WriteError::Precondition`] when the plan
        /// no longer matches the work or a subject has no message, and [`WriteError::Storage`],
        /// [`WriteError::Index`], [`WriteError::Layout`], [`WriteError::Tree`] or
        /// [`WriteError::Internal`] when a write fails.
        pub fn request_apply(
            &self,
            plan_json: &str,
            messages_json: &str,
            progress: &Progress,
            cancellation: &Cancellation,
        ) -> Result<TrackApplyReport, WriteError> {
            let given: GivenPlan =
                serde_json::from_str(plan_json).map_err(|error| WriteError::Json {
                    reason: error.to_string(),
                })?;
            let messages: BTreeMap<String, String> =
                serde_json::from_str(messages_json).map_err(|error| WriteError::Json {
                    reason: error.to_string(),
                })?;

            let prepared = self.prepare()?;
            let carry = prepared.carry();
            let planned = plan(
                &carry,
                &prepared.cwd,
                &self.files,
                prepared.options,
                progress,
                cancellation,
            )?;

            if !same_subjects(planned.subjects(), &given.subjects) {
                return Err(WriteError::Precondition {
                    what: "the plan".to_owned(),
                    reason: "it no longer matches the work".to_owned(),
                });
            }

            let mut texts = Vec::with_capacity(given.subjects.len());
            for subject in &given.subjects {
                let Some(text) = messages.get(&subject.path) else {
                    return Err(WriteError::Precondition {
                        what: format!("the message for `{}`", subject.path),
                        reason: "the messages name no words for it".to_owned(),
                    });
                };
                texts.push(text.clone());
            }

            let outcome = apply(
                &carry,
                planned,
                &texts,
                prepared.options,
                progress,
                cancellation,
            )?;

            Ok(TrackApplyReport {
                items: outcome.items.iter().map(ItemView::from).collect(),
                refused: outcome.refused.iter().map(RefusalView::from).collect(),
            })
        }

        /// Resolves every part a recording works through, in the order a command reads them.
        ///
        /// `cwd` is the normalized Workspace root, so a relative name never resolves against the
        /// process directory: a request has no directory of its own the way a command has one, and
        /// the Workspace it named is the only place its paths can mean anything.
        ///
        /// # Errors
        ///
        /// Returns [`WriteError::NoWorkspace`], [`WriteError::NoLayout`], [`WriteError::NoAccount`],
        /// [`WriteError::NoStore`] or [`WriteError::NoIndex`] when a part is missing,
        /// [`WriteError::Layout`] when the Layout cannot be read, and [`WriteError::Precondition`]
        /// when `policy` names no cut this build carries.
        fn prepare(&self) -> Result<Prepared, WriteError> {
            let workspace = self.common.workspace()?;
            let layout = self.common.layout(&workspace)?;
            let account = self.common.account(&workspace)?;
            let store = self.common.store(&workspace)?;
            let index = self.common.index(&workspace)?;
            let cwd = normalize(workspace.get_root());
            let rules = request_rules(&workspace, self.include_ignored);
            let policy = self
                .policy
                .as_deref()
                .map(|name| {
                    rorolala_storage::policy(name)
                        .map(|found| found.name)
                        .ok_or_else(|| WriteError::Precondition {
                            what: format!("chunking policy `{name}`"),
                            reason: "this build carries no such policy".to_owned(),
                        })
                })
                .transpose()?;

            Ok(Prepared {
                workspace,
                layout,
                store,
                index,
                rules,
                creator: account.name(),
                cwd,
                options: Options {
                    force: self.common.force,
                    replace: self.replace,
                    include_ignored: self.include_ignored,
                    keep_words: self.replace,
                    policy,
                },
                messages: Messages::default(),
            })
        }
    }

    /// The rules a request selects with, the empty set when it includes what they govern.
    ///
    /// The command warns when a rules file cannot be read; the library has no words of its own, and
    /// a request that includes the ignored is saying the rules do not select at all, so both end as
    /// the empty set.
    fn request_rules(workspace: &Workspace, include_ignored: bool) -> IgnoreRuleSet {
        if include_ignored {
            return IgnoreRuleSet::empty();
        }

        IgnoreRuleSet::load(
            &workspace.get_root().join(WORKSPACE_IGNORE_RULES_DIR),
            workspace.get_root(),
        )
        .unwrap_or_else(|_| IgnoreRuleSet::empty())
    }
}

/// The parts a tracked file is written through.
struct Place<'a> {
    layout: &'a Layout,
    store: &'a StorageSet,
    index: &'a VCSIndex,
}

/// Records one file's content as a version and binds it in a Layout.
fn record(
    place: &Place<'_>,
    resolved: &Path,
    relative: &str,
    path: &str,
    creator: &str,
    message: &str,
    options: TrackOptions,
) -> Result<TrackedFile, WriteError> {
    let layout = place.layout;
    let layout_path = LayoutPath::from_relative(Path::new(relative)).map_err(|error| {
        WriteError::InvalidPath {
            path: path.to_owned(),
            reason: error.to_string(),
        }
    })?;
    let existing = layout.id_of(&layout_path);
    let existing_owner = existing
        .and_then(|id| layout.entry(id))
        .and_then(|entry| entry.owner().map(str::to_owned));
    if let Some(id) = existing
        && layout
            .entry(id)
            .is_some_and(|entry| entry.kind() != EntryKind::File)
    {
        return Err(WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path is not a file".to_owned(),
        });
    }

    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let (base, base_num) = match existing.and_then(|id| layout.entry(id)) {
        Some(entry) if entry.version() != [0; 32] => {
            let version = place
                .index
                .read_version(&Key::new(entry.version()).to_string())
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?;
            let number = runtime
                .block_on(place.index.version_num(&version))
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?;

            (entry.version(), number)
        }
        _ => ([0; 32], rorolala_vcs::ROOT_VERSION),
    };
    let stored = runtime.block_on(put_file(
        place.store,
        resolved,
        (!options.policy.is_empty()).then_some(options.policy.as_str()),
    ))?;

    let written = write_file_version(
        place.index,
        &stored.key,
        &base,
        base_num,
        None,
        creator,
        message,
    )?;
    let id = existing.unwrap_or_else(Uuid::new_v4);
    write_entry(
        layout,
        id,
        *written.version.digest(),
        options.owner.or(existing_owner),
        options.description,
        existing.is_some(),
    )?;
    if existing.is_none() {
        create_path(layout, &layout_path, id)?;
    }

    Ok(TrackedFile {
        id,
        storage: stored.key,
        variant: written.variant,
        version: written.version,
    })
}

/// Records the content of `path` as a version and binds it in the Layout.
///
/// `path` is relative to `cwd`, or absolute; it must be inside the Workspace and outside a tree.
/// `creator` and `message` become the version's creator and message. `options` names the chunking
/// policy, the owner and the entry's description; a `None` owner keeps the owner the entry already
/// has.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Workspace or names a directory,
/// [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the parts are missing, and
/// [`WriteError::Storage`], [`WriteError::Index`] or [`WriteError::Layout`] when a write fails.
pub fn track_file(
    workspace: &Workspace,
    cwd: &Path,
    path: &str,
    creator: &str,
    message: &str,
    options: TrackOptions,
) -> Result<TrackedFile, WriteError> {
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = workspace.storage_set()?;
    let index = VCSIndex::locate(cwd).ok_or(WriteError::NoIndex)?;
    let root = normalize(workspace.get_root());
    let (resolved, relative) = locate_file(&root, cwd, path)?;
    let place = Place {
        layout: &layout,
        store: &store,
        index: &index,
    };
    let tracked = record(
        &place, &resolved, &relative, path, creator, message, options,
    )?;
    rebuild_tree_cache(workspace)?;

    Ok(tracked)
}

/// Records the content of `path` in a Vault and binds it in its Layout.
///
/// `path` is relative to `cwd`, or absolute; it must be inside the Vault and outside a tree.
/// `creator` and `message` become the version's creator and message. `options` names the chunking
/// policy, the owner and the entry's description; a `None` owner keeps the owner the entry already
/// has.
///
/// # Errors
///
/// Returns [`WriteError::InvalidPath`] when the path is outside the Vault or names a directory,
/// [`WriteError::NoStore`] or [`WriteError::NoIndex`] when the parts are missing, and
/// [`WriteError::Storage`], [`WriteError::Index`] or [`WriteError::Layout`] when a write fails.
pub fn track_vault_file(
    vault: &Vault,
    cwd: &Path,
    path: &str,
    creator: &str,
    message: &str,
    options: TrackOptions,
) -> Result<TrackedFile, WriteError> {
    let layout = vault.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;
    let store = vault.storage_set()?;
    let index = VCSIndex::locate(vault.get_root()).ok_or(WriteError::NoIndex)?;
    let root = normalize(vault.get_root());
    let (resolved, relative) = locate_file(&root, cwd, path)?;
    let place = Place {
        layout: &layout,
        store: &store,
        index: &index,
    };

    record(
        &place, &resolved, &relative, path, creator, message, options,
    )
}

/// The file `path` names, and its path inside the scope.
fn locate_file(root: &Path, cwd: &Path, path: &str) -> Result<(PathBuf, String), WriteError> {
    let resolved = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        cwd.join(path)
    };
    let resolved = normalize(&resolved);
    let relative = resolved
        .strip_prefix(root)
        .map_err(|_| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path is outside the Workspace".to_owned(),
        })?;
    if !resolved.is_file() {
        return Err(WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path is not a file".to_owned(),
        });
    }
    let mut above = resolved.parent();
    while let Some(directory) = above {
        if !directory.starts_with(root) {
            break;
        }
        if directory.join(rorolala_tree::consts::MARK).is_dir() {
            return Err(WriteError::InvalidPath {
                path: path.to_owned(),
                reason: "the path is inside a tree".to_owned(),
            });
        }
        above = directory.parent();
    }

    let relative = relative
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join("/"))
        .ok_or_else(|| WriteError::InvalidPath {
            path: path.to_owned(),
            reason: "the path cannot be written".to_owned(),
        })?;

    Ok((resolved, relative))
}

/// Records one file's content as a version in the Workspace a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","storage":"...","variant":"...","version":"..."}`, or a `RolaWriteError *` the
/// caller releases with `free_rola_write_error`. `policy` is empty for the store's own choice,
/// `owner` is empty when the entry has no owner, and `description` is the entry's description.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`track_file`] names.
#[lazyffi(export = rola_write_track_file)]
pub fn track_file_at(
    directory: &Path,
    path: &str,
    creator: &str,
    message: &str,
    policy: &str,
    owner: &str,
    description: &str,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let options = TrackOptions {
        policy: policy.to_owned(),
        owner: (!owner.is_empty()).then(|| owner.to_owned()),
        description: description.to_owned(),
    };
    let tracked = track_file(&workspace, directory, path, creator, message, options)?;

    serde_json::to_string(&tracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Records one file's content as a version in the Vault a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","storage":"...","variant":"...","version":"..."}`, or a `RolaWriteError *` the
/// caller releases with `free_rola_write_error`. `policy` is empty for the store's own choice,
/// `owner` is empty when the entry has no owner, and `description` is the entry's description.
///
/// # Errors
///
/// Returns [`WriteError::NoVault`] when no Vault holds `directory`, and the failures
/// [`track_vault_file`] names.
#[lazyffi(export = rola_write_track_vault_file)]
pub fn track_vault_file_at(
    directory: &Path,
    path: &str,
    creator: &str,
    message: &str,
    policy: &str,
    owner: &str,
    description: &str,
) -> Result<String, WriteError> {
    let vault = Vault::locate(directory).ok_or(WriteError::NoVault)?;
    let options = TrackOptions {
        policy: policy.to_owned(),
        owner: (!owner.is_empty()).then(|| owner.to_owned()),
        description: description.to_owned(),
    };
    let tracked = track_vault_file(&vault, directory, path, creator, message, options)?;

    serde_json::to_string(&tracked).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Reads what a batch recording would record, without writing anything.
///
/// # FFI
///
/// `request` is a JSON object of the shape [`batch::TrackRequest`] reads: the fields every write
/// request carries (`workspace`, `account`, `target`, `offline`, `force`, `concurrency` and
/// `labels`) at the top level, `files` as an array of strings, and `replace`, `policy` and
/// `include_ignored` when they are needed. A relative name in `files` is read from the Workspace
/// root, not from the process directory. The answer is an owned JSON string the caller releases
/// with `free_string`,
/// `{"subjects":[{"tree":false,"key":"...","path":"a.txt","digest":"..."}],"unchanged":0,"prefill":[],"refused":null}`.
/// `digest` is the content's digest in lowercase hex, and empty for a tree. `key` is the subject's
/// section key, and `path` is the name the messages object maps. `refused` is null or an externally
/// tagged refusal object. `progress` is the handle the caller polls with
/// `rola_progress_next_json`, and `cancellation` is the flag it sets with
/// `rola_cancellation_cancel`.
///
/// # Errors
///
/// Returns a `RolaWriteError *` the caller releases with `free_rola_write_error`, with the tag
/// [`WriteError`]'s: `Json` when `request` does not read, and every failure
/// [`batch::TrackRequest::request_plan`] names when the reading fails.
#[lazyffi(export = rola_write_track_plan)]
pub fn track_plan_at(
    request: &str,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<String, WriteError> {
    let request: batch::TrackRequest =
        serde_json::from_str(request).map_err(|error| WriteError::Json {
            reason: error.to_string(),
        })?;
    let watched = crate::write::progress::watching(progress);
    let report = request.request_plan(&watched, cancellation)?;

    serde_json::to_string(&report).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Records what a plan named, once the caller has said what each subject did.
///
/// # FFI
///
/// `request` is the JSON object [`track_plan_at`] reads. `plan` is the JSON that call answered
/// with. `messages` is a JSON object mapping each subject's `path` to the words its version
/// carries, as `{"a.txt":"first","art":"tree structure"}`. The answer is an owned JSON string the
/// caller releases with `free_string`,
/// `{"items":[{"path":"a.txt","action":"added"}],"refused":[]}`. `action` is `added`, `updated` or
/// `replaced`. `refused` is the same externally tagged shape the plan answers with, and `items` is
/// empty when it is not.
///
/// # Errors
///
/// Returns a `RolaWriteError *` the caller releases with `free_rola_write_error`, with the tag
/// [`WriteError`]'s: `Json` when `request`, `plan` or `messages` does not read, `Precondition` when
/// the plan no longer matches the work or a subject has no message, and every failure
/// [`batch::TrackRequest::request_apply`] names when a write fails.
#[lazyffi(export = rola_write_track_apply)]
pub fn track_apply_at(
    request: &str,
    plan: &str,
    messages: &str,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<String, WriteError> {
    let request: batch::TrackRequest =
        serde_json::from_str(request).map_err(|error| WriteError::Json {
            reason: error.to_string(),
        })?;
    let watched = crate::write::progress::watching(progress);
    let report = request.request_apply(plan, messages, &watched, cancellation)?;

    serde_json::to_string(&report).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_utils_progress::Progress;
    use rorolala_vault::Vault;
    use rorolala_vcs::VCSIndex;
    use rorolala_workspace::Workspace;

    use super::batch::{Carry, Messages, Options, apply, plan};
    use super::{TrackOptions, track_file, track_vault_file};

    #[test]
    fn a_file_is_recorded_and_named() {
        let dir = std::env::temp_dir().join(format!("rola-write-track-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let tracked = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions {
                owner: Some("alice".to_owned()),
                description: "note".to_owned(),
                ..TrackOptions::default()
            },
        )
        .unwrap();

        let layout = workspace.layout().unwrap();
        let path = rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        assert_eq!(layout.id_of(&path), Some(tracked.id));
        assert_ne!(tracked.version.to_string(), "");

        fs::write(dir.join("a.txt"), b"two").unwrap();
        let again = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();
        assert_eq!(
            layout
                .entry(tracked.id)
                .and_then(|entry| entry.owner().map(str::to_owned)),
            Some("alice".to_owned())
        );
        assert_eq!(
            layout
                .entry(tracked.id)
                .map(|entry| entry.description().to_owned()),
            Some("note".to_owned())
        );

        let index = VCSIndex::locate(&dir).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let numbered = |version: rorolala_storage::Key| {
            let version = index
                .read_version(&version.to_string())
                .expect("the version is in the index");
            runtime
                .block_on(index.version_num(&version))
                .expect("the chain reads")
        };
        assert_eq!(numbered(tracked.version), 0);
        assert_eq!(numbered(again.version), 1);

        fs::create_dir_all(dir.join("tree/.rola-tree")).unwrap();
        fs::write(dir.join("tree/inside.txt"), b"inside").unwrap();
        assert!(
            track_file(
                &workspace,
                &dir,
                "tree/inside.txt",
                "alice",
                "tree",
                TrackOptions::default()
            )
            .is_err()
        );
        assert!(
            track_file(
                &workspace,
                &dir,
                "../outside.txt",
                "alice",
                "outside",
                TrackOptions::default()
            )
            .is_err()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_file_is_recorded_and_named() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-track-vault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Vault::create(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let tracked = track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        let layout = vault.layout().unwrap();
        let path = rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        assert_eq!(layout.id_of(&path), Some(tracked.id));
        assert_ne!(tracked.version.to_string(), "");

        let _ = fs::remove_dir_all(&dir);
    }

    /// A plan reads what changed and writes nothing; an apply records exactly the plan it was given.
    #[test]
    fn a_plan_is_read_only_and_an_apply_records_what_it_named() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-track-plan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let layout = workspace.layout().unwrap();
        let store = workspace.storage_set().unwrap();
        let index = VCSIndex::locate(&dir).unwrap();
        let rules = rorolala_ignore::IgnoreRuleSet::empty();
        let messages = Messages::default();
        let carry = Carry {
            workspace: &workspace,
            layout: &layout,
            store: &store,
            index: &index,
            creator: "alice",
            rules: &rules,
            messages: &messages,
        };
        let cancellation = crate::write::Cancellation::new();
        let path = rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let cache = rorolala_workspace_analyze::cache_path(&dir, &layout);

        let plan = plan(
            &carry,
            &dir,
            &["a.txt".to_owned()],
            Options::default(),
            &Progress::silent(),
            &cancellation,
        )
        .unwrap();

        assert!(plan.refused().is_none());
        assert_eq!(plan.changed(), 1);
        assert_eq!(plan.unchanged(), 0);
        assert_eq!(plan.subjects().len(), 1);
        assert_eq!(plan.subjects()[0].path, "a.txt");
        assert_eq!(
            plan.subjects()[0].digest,
            rorolala_storage::key_of_file(dir.join("a.txt"))
                .expect("the file's content hashes")
                .hex(),
            "a plan carries the digest of the content it read"
        );
        assert_eq!(
            layout.id_of(&path),
            None,
            "a plan writes nothing into the Layout"
        );
        assert!(
            !cache.exists(),
            "a plan writes nothing into the cache the reading keeps"
        );

        let texts = vec!["first".to_owned()];
        let outcome = apply(
            &carry,
            plan,
            &texts,
            Options::default(),
            &Progress::silent(),
            &cancellation,
        )
        .unwrap();

        assert!(outcome.refused.is_empty());
        assert_eq!(outcome.items.len(), 1);
        assert_eq!(outcome.items[0].path, "a.txt");
        assert!(layout.id_of(&path).is_some());
        assert!(cache.exists(), "an apply remembers what it recorded");

        let _ = fs::remove_dir_all(&dir);
    }

    /// A second recording leaves the reading clean: what the cache agrees with is the digest just
    /// recorded, not the one the cache held before the file changed.
    #[test]
    fn a_second_recording_leaves_the_reading_clean() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-track-again-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let rules = rorolala_ignore::IgnoreRuleSet::empty();
        let messages = Messages::default();
        let cancellation = crate::write::Cancellation::new();

        for (content, message) in [("one", "first"), ("two", "second")] {
            fs::write(dir.join("a.txt"), content).unwrap();

            let layout = workspace.layout().unwrap();
            let store = workspace.storage_set().unwrap();
            let index = VCSIndex::locate(&dir).unwrap();
            let carry = Carry {
                workspace: &workspace,
                layout: &layout,
                store: &store,
                index: &index,
                creator: "alice",
                rules: &rules,
                messages: &messages,
            };

            let plan = plan(
                &carry,
                &dir,
                &["a.txt".to_owned()],
                Options::default(),
                &Progress::silent(),
                &cancellation,
            )
            .unwrap();
            assert_eq!(plan.changed(), 1, "`{content}` is a change");

            let outcome = apply(
                &carry,
                plan,
                &[message.to_owned()],
                Options::default(),
                &Progress::silent(),
                &cancellation,
            )
            .unwrap();
            assert!(outcome.refused.is_empty());
        }

        let layout = workspace.layout().unwrap();
        let diff = rorolala_workspace_analyze::tree_diff(&layout, &workspace, 0.6, &rules).unwrap();
        assert!(
            diff.modified.is_empty(),
            "the recorded change is not left dirty: {:?}",
            diff.modified
        );
        assert!(
            diff.untagged.is_empty(),
            "the second recording adds nothing"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A plan that was cancelled before it began reads nothing and writes nothing.
    #[test]
    fn a_cancelled_plan_writes_nothing() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-track-cancel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let layout = workspace.layout().unwrap();
        let store = workspace.storage_set().unwrap();
        let index = VCSIndex::locate(&dir).unwrap();
        let rules = rorolala_ignore::IgnoreRuleSet::empty();
        let messages = Messages::default();
        let carry = Carry {
            workspace: &workspace,
            layout: &layout,
            store: &store,
            index: &index,
            creator: "alice",
            rules: &rules,
            messages: &messages,
        };
        let cancellation = crate::write::Cancellation::new();
        cancellation.cancel();
        let path = rorolala_layout::LayoutPath::from_relative("a.txt".as_ref()).unwrap();

        let error = plan(
            &carry,
            &dir,
            &["a.txt".to_owned()],
            Options::default(),
            &Progress::silent(),
            &cancellation,
        )
        .err()
        // UNWRAP: a plan that was cancelled before it began cannot have made a plan.
        .expect("a cancelled plan is refused");

        assert!(matches!(error, crate::write::WriteError::Cancelled));
        assert_eq!(layout.id_of(&path), None);

        let _ = fs::remove_dir_all(&dir);
    }

    /// A plan and an apply answer the shapes a C caller reads, with a refusal externally tagged the
    /// way the library names it.
    #[test]
    fn a_track_report_answers_the_c_abi_shape() {
        use super::batch::{
            ActionView, FileKindView, ItemView, RefusalView, SubjectView, TrackApplyReport,
            TrackPlanReport,
        };

        let plan = TrackPlanReport {
            subjects: vec![SubjectView {
                tree: false,
                key: "key".to_owned(),
                path: "a.txt".to_owned(),
                digest: "ab".to_owned(),
            }],
            unchanged: 2,
            prefill: vec![String::new()],
            refused: Some(RefusalView::File {
                path: "a.txt".to_owned(),
                kind: FileKindView::NotAFile,
            }),
        };
        assert_eq!(
            serde_json::to_string(&plan).expect("a report serializes"),
            r#"{"subjects":[{"tree":false,"key":"key","path":"a.txt","digest":"ab"}],"unchanged":2,"prefill":[""],"refused":{"file":{"path":"a.txt","kind":"not_a_file"}}}"#
        );

        let apply = TrackApplyReport {
            items: vec![ItemView {
                path: "a.txt".to_owned(),
                action: ActionView::Added,
            }],
            refused: vec![RefusalView::Ignored {
                paths: vec!["b.txt".to_owned()],
            }],
        };
        assert_eq!(
            serde_json::to_string(&apply).expect("a report serializes"),
            r#"{"items":[{"path":"a.txt","action":"added"}],"refused":[{"ignored":{"paths":["b.txt"]}}]}"#
        );
    }
    /// A plan and an apply through the ABI record what the plan saw.
    ///
    /// An account is located by the name of its key file alone, so a key file this test never signs
    /// with still lets the recording act as `alice`: what a recording names is the creator, and no
    /// signature is made on this side.
    #[test]
    fn a_plan_and_an_apply_record_a_file_through_the_abi() {
        let dir = std::env::temp_dir().join(format!("rola-write-track-abi-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        fs::create_dir_all(dir.join(".rola/auth")).unwrap();
        fs::write(
            dir.join(".rola/auth/alice.pem"),
            b"a key this test never signs with",
        )
        .unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();

        let request = serde_json::json!({
            "workspace": dir,
            "account": "alice",
            "files": ["a.txt"],
        })
        .to_string();
        let progress = super::WriteProgress::new();
        let cancellation = super::Cancellation::new();

        let planned =
            super::track_plan_at(&request, &progress, &cancellation).expect("the plan reads");
        let plan: serde_json::Value = serde_json::from_str(&planned).expect("the plan is JSON");
        assert_eq!(plan["subjects"][0]["path"], "a.txt", "{planned}");
        assert!(
            plan["subjects"][0]["digest"]
                .as_str()
                .is_some_and(|digest| digest.len() == 64),
            "{planned}"
        );

        let applied = super::track_apply_at(
            &request,
            &planned,
            r#"{"a.txt":"first"}"#,
            &progress,
            &cancellation,
        )
        .expect("the apply records");
        let report: serde_json::Value = serde_json::from_str(&applied).expect("the report is JSON");
        assert_eq!(report["items"][0]["path"], "a.txt", "{applied}");
        assert_eq!(report["items"][0]["action"], "added", "{applied}");

        let again =
            super::track_plan_at(&request, &progress, &cancellation).expect("the plan reads");
        let again: serde_json::Value = serde_json::from_str(&again).expect("the plan is JSON");
        assert_eq!(
            again["subjects"].as_array().map(Vec::len),
            Some(0),
            "{again}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

//! Puts a path on one version, and settles the tree against the Layout it works from.
//!
//! Two things meet here. The plain one is [`align_entry`]: a path, a version, and what the record
//! says about it, which is what a checkout or the C ABI asks for. The other is the whole of
//! `rola align`: a reading of the tree against the Layout, and what a run then does about one path
//! the reading found, whether that is confirming a loss, confirming or taking back a move, asserting
//! one, or putting content back.
//!
//! The order of the second is here rather than in the command because it is a sequence of writes to
//! one Workspace: the reading is taken once, what it found is acted on once, and a run that stops
//! partway leaves exactly what it had already written. The command chooses a mode and says the
//! results in its own words; a failure that has no category of its own is said in the words the
//! command hands in through [`Messages`], and the watching a fetch reports to comes in through
//! [`Carry::progress`].

use std::fs;
use std::path::{Path, PathBuf};

use rorolala_auth::Account;
use rorolala_errors::Failure as _;
use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::{EntryKind, Layout, LayoutError, LayoutPath};
use rorolala_protocol::VaultAddress;
use rorolala_storage::{Blake3Hash, Key, StorageSet};
use rorolala_tree::{EntryKind as TreeEntryKind, Staging, Tree, TreeStructure, read_structure};
use rorolala_utils_constants::WORKSPACE_IGNORE_RULES_DIR;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_utils_progress::Progress;
use rorolala_vault::Vault;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::{Merging, Pending, Workspace};
use rorolala_workspace_analyze::{
    Cache, PathRename, TreeDiff, cache_path, entry_of, tree_diff_remembering,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::fetch::Sources;
use crate::status::{VariantMove, variant_moves};
use crate::write::request::Common;
use crate::write::storage::extract_file;
use crate::write::{
    Cancellation, WriteError, WriteProgress,
    checkout::{Restored, restore, restore_vault},
    layout::set_all,
};

/// What aligning one path wrote.
#[derive(Debug, Clone, Serialize)]
pub struct Aligned {
    /// The Layout entry the path names.
    pub id: uuid::Uuid,
    /// The version the Layout now records.
    pub version: rorolala_storage::Key,
}

/// Puts the content of `version` at `path` and makes the entry say what `owner`, `description` and
/// `preference` say.
///
/// # Errors
///
/// Returns the failures [`restore`] names, and [`WriteError::Layout`] when the entry cannot be
/// written.
pub fn align_entry(
    workspace: &Workspace,
    cwd: &Path,
    path: &str,
    version: &str,
    owner: Option<&str>,
    description: &str,
    preference: Option<&str>,
) -> Result<Aligned, WriteError> {
    let restored = restore(workspace, cwd, path, version)?;
    let layout = workspace.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;

    align_in(&layout, &restored, owner, description, preference)
}

/// Puts the content of `version` at `path` in a Vault and makes the entry say what `owner`,
/// `description` and `preference` say.
///
/// # Errors
///
/// Returns the failures [`restore_vault`] names, and [`WriteError::Layout`] when the entry cannot
/// be written.
pub fn align_vault_entry(
    vault: &Vault,
    cwd: &Path,
    path: &str,
    version: &str,
    owner: Option<&str>,
    description: &str,
    preference: Option<&str>,
) -> Result<Aligned, WriteError> {
    let restored = restore_vault(vault, cwd, path, version)?;
    let layout = vault.layout().map_err(|error| WriteError::Layout {
        reason: error.to_string(),
    })?;

    align_in(&layout, &restored, owner, description, preference)
}

/// Writes the entry an aligned path now names, in one call.
fn align_in(
    layout: &Layout,
    restored: &Restored,
    owner: Option<&str>,
    description: &str,
    preference: Option<&str>,
) -> Result<Aligned, WriteError> {
    set_all(
        layout,
        restored.id,
        *restored.version.digest(),
        owner,
        description,
        preference,
    )?;

    Ok(Aligned {
        id: restored.id,
        version: restored.version,
    })
}

/// Puts a path under a Workspace on one version and writes what the record says about it.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `owner` and `preference` are empty when the entry has none.
///
/// It is the single-entry form: it aligns one path with one version. The `rola_write_align` export
/// is the whole `rola align` workflow, which takes a request document instead.
///
/// # Errors
///
/// Returns [`WriteError::NoWorkspace`] when no Workspace holds `directory`, and the failures
/// [`align_entry`] names.
#[lazyffi(export = rola_write_align_entry)]
pub fn align_entry_at(
    directory: &Path,
    path: &str,
    version: &str,
    owner: &str,
    description: &str,
    preference: &str,
) -> Result<String, WriteError> {
    let workspace = Workspace::locate(directory).ok_or(WriteError::NoWorkspace)?;
    let aligned = align_entry(
        &workspace,
        directory,
        path,
        version,
        (!owner.is_empty()).then_some(owner),
        description,
        (!preference.is_empty()).then_some(preference),
    )?;

    serde_json::to_string(&aligned).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// Puts a path under a Vault on one version and writes what the record says about it.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"id":"...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `owner` and `preference` are empty when the entry has none.
///
/// # Errors
///
/// Returns [`WriteError::NoVault`] when no Vault holds `directory`, and the failures
/// [`align_vault_entry`] names.
#[lazyffi(export = rola_write_align_vault)]
pub fn align_vault_at(
    directory: &Path,
    path: &str,
    version: &str,
    owner: &str,
    description: &str,
    preference: &str,
) -> Result<String, WriteError> {
    let vault = Vault::locate(directory).ok_or(WriteError::NoVault)?;
    let aligned = align_vault_entry(
        &vault,
        directory,
        path,
        version,
        (!owner.is_empty()).then_some(owner),
        description,
        (!preference.is_empty()).then_some(preference),
    )?;

    serde_json::to_string(&aligned).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// How alike two text files have to be to count as the same file moved, when nothing is said.
///
/// It is the same default `rola layout tree-diff` reads the tree with, since a move this acts on is
/// one that reading found.
const DEFAULT_ALIKE: f32 = 0.6;

/// What an alignment is to do with the one path a run named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Confirm that a path the Layout names but the tree does not hold is gone.
    Delete,
    /// Confirm the move the path is a side of.
    Rename,
    /// Take back the move the path is a side of, leaving a loss and a gain.
    Break,
    /// Assert that the path moved to the path named here.
    Move(String),
    /// Put a moved file back where the Layout names it.
    RestoreMove,
    /// Put a changed file back to the version the Layout names.
    RestoreModify,
    /// Put a file inside a tree back to what the tree has staged for it.
    RestoreStaged,
    /// Put a deleted file back, taking the version the Layout names out of the store.
    RestoreDelete,
    /// Do whichever of the three a path put back needs.
    Restore,
}

/// What an alignment is to do with the one path a run named, as a request document names it.
///
/// It is [`Mode`] with a wire spelling, so a C caller can name the same choice a command line does.
/// The document carries the choice in `kind`, and `move_to` also carries where the path went.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlignMode {
    /// Confirm that a path the Layout names but the tree does not hold is gone.
    Delete,
    /// Confirm the move the path is a side of.
    Rename,
    /// Take back the move the path is a side of, leaving a loss and a gain.
    Break,
    /// Assert that the path moved to the path named here.
    MoveTo {
        /// Where the path moved to, read against the Workspace root.
        path: String,
    },
    /// Put a moved file back where the Layout names it.
    RestoreMove,
    /// Put a changed file back to the version the Layout names.
    RestoreModify,
    /// Put a file inside a tree back to what the tree has staged for it.
    RestoreStaged,
    /// Put a deleted file back, taking the version the Layout names out of the store.
    RestoreDelete,
    /// Do whichever of the three a path put back needs.
    Restore,
}

impl From<&AlignMode> for Mode {
    fn from(mode: &AlignMode) -> Self {
        match mode {
            AlignMode::Delete => Self::Delete,
            AlignMode::Rename => Self::Rename,
            AlignMode::Break => Self::Break,
            AlignMode::MoveTo { path } => Self::Move(path.clone()),
            AlignMode::RestoreMove => Self::RestoreMove,
            AlignMode::RestoreModify => Self::RestoreModify,
            AlignMode::RestoreStaged => Self::RestoreStaged,
            AlignMode::RestoreDelete => Self::RestoreDelete,
            AlignMode::Restore => Self::Restore,
        }
    }
}

/// What a run may do beside what its mode says.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Put a move back in content as well as in place, for the modes it means something for.
    pub completely: bool,
    /// Select an entry the ignore rules govern anyway.
    pub include_ignored: bool,
}

/// What an alignment did to one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Did {
    /// A lost path was confirmed gone.
    Deleted,
    /// A move was confirmed.
    Moved,
    /// A move was taken back.
    Broke,
    /// A move was asserted for the next reading.
    Asserted,
    /// A moved file was put back where the Layout names it.
    RestoredMove,
    /// A moved file was put back where and as the Layout names it.
    RestoredCompletely,
    /// A changed file was put back to the version the Layout names.
    RestoredModify,
    /// A deleted file was put back, taken out of the store.
    RestoredDelete,
    /// A variant file was put back, ending the wait to be joined.
    RestoredMerge,
    /// A merge waiting on a variant file was ended, and its record taken away.
    Unmerged,
}

/// One path an alignment acted on.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    /// What was done with it.
    pub did: Did,
    /// What it was done to.
    pub what: String,
}

/// Why a run will not do what it was asked.
///
/// A refusal is a decision rather than a failure: the command turns one into its own words and exit
/// code, while [`WriteError`] is what the library could not do at all.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Refusal {
    /// The path names nothing the run's mode could act on.
    Argument,
    /// The path is one the ignore rules govern, so it cannot be selected.
    Ignored {
        /// What was named.
        path: String,
    },
    /// The path is not one the Layout names and the tree does not hold.
    NotLost {
        /// What was named.
        path: String,
    },
    /// The path is not a change against a version the Layout names.
    NotModified {
        /// What was named.
        path: String,
    },
    /// A tree has nothing staged for the path named.
    NotStaged {
        /// What was named.
        path: String,
    },
    /// The path is neither a move, a change, nor a deletion, so there is nothing to put back.
    NotRestorable {
        /// What was named.
        path: String,
    },
    /// Putting a file back would land on one already there.
    RestoreExists {
        /// Where it would land.
        path: String,
    },
    /// The path is not a side of a move the reading found.
    NoRename {
        /// What was named.
        path: String,
    },
    /// A run was asked to take back a move of two files that hold the same bytes.
    RealMove {
        /// Where the Layout named it.
        from: String,
        /// Where the tree holds it.
        to: String,
    },
    /// What a move names cannot be moved onto.
    Move {
        /// What was named.
        path: String,
        /// What is wrong with it.
        ///
        /// The wire spelling is `move_kind`, since `kind` is what names the refusal itself.
        #[serde(rename = "move_kind")]
        kind: MoveKind,
    },
    /// A per-file mode was asked of a tree.
    TreeUnsupported,
    /// A Layout change names a path that is already there.
    LayoutExists,
    /// A Layout change names a path that is not there.
    LayoutMissing,
    /// The Layout failed for another reason.
    LayoutFailed {
        /// What the Layout said.
        cause: String,
    },
    /// The run stopped for a reason the caller has the words for.
    Failed {
        /// What the caller says about it.
        cause: String,
    },
}

/// The ways what a move names cannot be moved onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveKind {
    /// It is not a file inside the Workspace.
    Missing,
    /// It is not a tree inside the Workspace.
    NotATree,
    /// The Layout already names it.
    Tracked,
}

/// What an alignment came to.
#[derive(Debug)]
pub enum Outcome {
    /// What was done to the paths named, in the order it was done.
    Paths(Vec<Item>),
    /// One tree's work was put back, named as the Layout names it.
    Tree {
        /// The tree's path in the Layout.
        path: String,
    },
    /// One file inside a tree was put back.
    TreeFile {
        /// The file as a person knows it.
        path: String,
        /// Whether it came from what the tree has staged rather than the version it records.
        staged: bool,
    },
    /// The run refused to act, and names why.
    Refused(Refusal),
}

/// What one alignment came to, and what it had to bring to come to it.
///
/// The two are one value because a run that stops still says what it brought: a restore that asked
/// for content and then failed has fetched that content all the same, and a caller that dropped the
/// count would leave the reader thinking nothing crossed.
#[derive(Debug)]
pub struct Settled {
    /// What the run did, or the write that stopped it.
    pub outcome: Result<Outcome, WriteError>,
    /// What each Vault handed over while the run worked, in the order they were asked.
    pub took: Vec<(String, usize)>,
}

impl Settled {
    /// A run that came to `outcome` without bringing anything.
    const fn of(outcome: Result<Outcome, WriteError>) -> Self {
        Self {
            outcome,
            took: Vec::new(),
        }
    }
}

/// The words an alignment says a failure in.
///
/// A word the library could print itself would be the one place a command could not translate, so
/// the words that have no category of their own come from the caller.
#[derive(Debug, Clone)]
pub struct Messages {
    /// The Layout records no version for what is being put back.
    pub not_recorded: String,
    /// The tree holds a path this build cannot write back.
    pub unsupported: String,
}

impl Default for Messages {
    fn default() -> Self {
        Self {
            not_recorded: "the tree records no version yet".to_owned(),
            unsupported: "the tree holds a path this build cannot write back".to_owned(),
        }
    }
}

/// What one alignment works over: the Workspace, the parts a restore reads, and the Vaults it may ask.
pub struct Carry<'a> {
    /// The Workspace being worked in.
    pub workspace: &'a Workspace,
    /// The Layout being worked in.
    pub layout: &'a Layout,
    /// The directory the run was made in, which a name is read from.
    pub cwd: &'a Path,
    /// The rules the run selects with, the empty set when the run includes the ignored.
    pub rules: &'a IgnoreRuleSet,
    /// The store content is read out of, when the run has one.
    pub storage: Option<&'a StorageSet>,
    /// The index that says which version names which object, when the run has one.
    pub index: Option<&'a VCSIndex>,
    /// The account a variant file is brought in as, when the run knows one.
    pub account: Option<&'a Account>,
    /// The Vaults content may be brought from, the one the run named first.
    pub candidates: &'a [(String, VaultAddress)],
    /// Whether the run may reach a Vault at all.
    pub offline: bool,
    /// How many reads a transfer keeps in flight, which is never less than one.
    pub concurrency: usize,
    /// What a fetch says while it runs. Nothing else in a run has a bar of its own.
    pub progress: &'a Progress,
    /// The words a failure with no category of its own is said in.
    pub messages: &'a Messages,
}

/// One `rola align` run, as a request document names it.
///
/// The fields are the run's: the [`Common`] every write shares, the path it acts on, the one
/// [`AlignMode`] it acts with, and the two flags the command line spells `--completely` and
/// `--include-ignored`. The document is one object, and `common`'s fields sit beside `path`
/// because they are flattened:
///
/// ```json
/// {"workspace":"D:/work","path":"a.txt","mode":{"kind":"delete"}}
/// ```
///
/// A relative `path`, and a relative `path` in a `move_to` mode, is read against the Workspace root:
/// a C or C# caller has no process directory the run could mean, so the root the request names is
/// the whole of where a name is read from.
///
/// [`align_at`] reads one from JSON; a Rust caller makes one and calls [`run`](Self::run).
#[derive(Debug, Deserialize)]
pub struct AlignRequest {
    /// The Workspace, the account, the target and the rest of what every write shares.
    #[serde(flatten)]
    common: Common,
    /// The path the run acts on, read against the Workspace root.
    path: String,
    /// What the run does with the path.
    mode: AlignMode,
    /// Whether a move is put back in content as well as in place.
    #[serde(default)]
    completely: bool,
    /// Whether an entry the ignore rules govern may be selected anyway.
    #[serde(default)]
    include_ignored: bool,
}

/// Settles the tree against the Layout for the one path a run named.
///
/// The mode is the command's choice; what it means for the path is read here. A path inside a tree
/// is settled as the tree's own entry, or as one file of its structure, before anything is fetched;
/// every other path is read against a fresh reading of the tree, and the reading's verdict is what
/// the mode acts on. Content a restore writes is brought from the Vaults [`Carry`] names only when
/// the store does not hold it.
///
/// `cancellation` is checked before the run touches anything and between the items of every loop
/// that acts on many paths, so a run asked to stop stops between two paths and leaves the ones
/// already written.
///
/// # Errors
///
/// The outcome is [`WriteError::Cancelled`] when `cancellation` was set before the run or between
/// the steps of a long one, [`WriteError::NoStore`] or [`WriteError::NoIndex`] when a mode needs
/// content that cannot be reached, and whatever the write itself failed with. A decision the run
/// makes rather than a failure is [`Outcome::Refused`], which is an outcome and not an error.
pub fn settle(
    carry: &Carry<'_>,
    given: &str,
    mode: &Mode,
    options: Options,
    cancellation: &Cancellation,
) -> Settled {
    if cancellation.is_cancelled() {
        return Settled::of(Err(WriteError::Cancelled));
    }

    let root = normalize(carry.workspace.get_root());
    let target = resolve(carry.cwd, given);

    // A tree is one unit of work: `--restore` and `--restore-delete` put the whole of it back to
    // what the Layout records, rather than a file's content at a path. A move is settled as one
    // entry like a file's, so the modes that act on an entry go on to the reading below.
    //
    // Which paths are trees is read from the Layout rather than from the mark on disk: a tree whose
    // directory was deleted has no mark to find, and what a restore writes then is the whole tree
    // again, mark and baseline included, rather than the structure's bytes in a file at its path.
    let tree_path = target
        .strip_prefix(&root)
        .ok()
        .and_then(|relative| find_tracked_tree(carry.layout, relative));

    if let Some(tree_path) = &tree_path {
        if matches!(mode, Mode::Restore | Mode::RestoreDelete) {
            return Settled::of(restore_tracked_tree(
                carry,
                &root,
                tree_path,
                options.completely,
                cancellation,
            ));
        }

        if matches!(mode, Mode::RestoreModify | Mode::RestoreStaged) {
            return Settled::of(restore_tracked_file(
                carry,
                &root,
                tree_path,
                &target,
                mode,
                cancellation,
            ));
        }

        // A per-file content mode has nothing to settle for a tree, and a move the reading found is
        // a move of the entry rather than of a file inside it.
        if !matches!(
            mode,
            Mode::Rename | Mode::Delete | Mode::Move(_) | Mode::RestoreMove
        ) {
            return Settled::of(Ok(Outcome::Refused(Refusal::TreeUnsupported)));
        }
    }

    // Content a restore writes comes out of the store, and the store may not hold it: what is
    // missing is asked of the Vaults the run was handed. A run that cannot say which account it
    // acts as has none to ask, and then restores only what is here.
    let sources = match sources_of(carry) {
        Ok(sources) => sources,
        Err(error) => return Settled::of(Err(error)),
    };
    let outcome = after_tree(
        carry,
        &root,
        given,
        mode,
        options,
        sources.as_ref(),
        cancellation,
    );
    let took = sources.as_ref().map_or_else(Vec::new, Sources::took);

    Settled { outcome, took }
}

impl AlignRequest {
    /// Runs the alignment the request names.
    ///
    /// It reads what the run works over in the order a command does: the Workspace, the Layout it
    /// works in, the Workspace root a relative name is read against, and then the parts a restore may
    /// need. The account and the Vault are read tolerantly, since a plain restore reads only what
    /// the store already holds: a run that has neither still puts content back, while a Vault the
    /// request names is asked first when content is missing.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoWorkspace`] when no Workspace holds the request's directory,
    /// [`WriteError::NoLayout`] or [`WriteError::Layout`] when the Layout cannot be read, and
    /// whatever [`settle`] names. A decision the run makes rather than a failure is
    /// [`Outcome::Refused`], which is an outcome and not an error.
    pub fn run(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Outcome, WriteError> {
        let common = &self.common;
        let workspace = common.workspace()?;
        let layout = common.layout(&workspace)?;
        let cwd = normalize(workspace.get_root());
        let account = common.account(&workspace).ok();
        let storage = common.store(&workspace).ok();
        let index = common.index(&workspace).ok();
        let candidates = candidates_of(common, &workspace);
        let rules = request_rules(&workspace, self.include_ignored);
        let messages = Messages::default();

        let carry = Carry {
            workspace: &workspace,
            layout: &layout,
            cwd: &cwd,
            rules: &rules,
            storage: storage.as_ref(),
            index: index.as_ref(),
            account: account.as_ref(),
            candidates: &candidates,
            offline: common.offline,
            concurrency: common.concurrency.max(1),
            progress,
            messages: &messages,
        };

        settle(
            &carry,
            &self.path,
            &Mode::from(&self.mode),
            Options {
                completely: self.completely,
                include_ignored: self.include_ignored,
            },
            cancellation,
        )
        .outcome
    }
}

/// The Vaults a request may bring content from, the one it reaches for first.
///
/// [`Common::candidates`] lists the Vaults the Workspace bound, and puts the one a named target
/// reaches for first. An address target is one the Workspace never bound, so it is not in that list
/// at all and is put ahead of it here: it is still the Vault the request says to ask.
fn candidates_of(common: &Common, workspace: &Workspace) -> Vec<(String, VaultAddress)> {
    let mut candidates = common.candidates(workspace).unwrap_or_default();
    if let Ok(reached) = common.vault(workspace)
        && reached.name.is_empty()
    {
        candidates.insert(0, (reached.name, reached.address));
    }

    candidates
}

/// The outcome of an alignment as a request caller reads it.
///
/// The tag is `kind`, and its value says which of the four shapes it is. It is a view rather than a
/// `Serialize` on [`Outcome`] because the four carry no field in common, and a newtype variant
/// holding a list cannot carry a tag in serde.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OutcomeJson<'a> {
    /// What was done to the paths named, in the order it was done.
    Paths {
        /// What was done, in the order it was done.
        paths: &'a [Item],
    },
    /// One tree's work was put back, named as the Layout names it.
    Tree {
        /// The tree's path in the Layout.
        path: &'a str,
    },
    /// One file inside a tree was put back.
    TreeFile {
        /// The file as a person knows it.
        path: &'a str,
        /// Whether it came from what the tree has staged rather than the version it records.
        staged: bool,
    },
    /// The run refused to act, and names why.
    Refused {
        /// Why it refused.
        refusal: &'a Refusal,
    },
}

impl<'a> From<&'a Outcome> for OutcomeJson<'a> {
    fn from(outcome: &'a Outcome) -> Self {
        match outcome {
            Outcome::Paths(items) => Self::Paths { paths: items },
            Outcome::Tree { path } => Self::Tree { path },
            Outcome::TreeFile { path, staged } => Self::TreeFile {
                path,
                staged: *staged,
            },
            Outcome::Refused(refusal) => Self::Refused { refusal },
        }
    }
}

/// Runs the whole `rola align` workflow for a request document and answers what it came to.
///
/// # FFI
///
/// `request` is an owned JSON document the caller keeps; it names the Workspace, the path and the
/// mode, and carries the `Common` fields beside them:
/// `{"workspace":"D:/work","path":"a.txt","mode":{"kind":"delete"}}`. The mode is one of `delete`,
/// `rename`, `break`, `move_to`, `restore_move`, `restore_modify`, `restore_staged`,
/// `restore_delete` and `restore`, where `move_to` also carries `path`. Relative paths in `path`
/// and in a `move_to` mode resolve against the Workspace root. `completely` and `include_ignored`
/// are false when left out.
///
/// `progress` is the `RolaProgress *` the run says its events to, and `cancellation` is the
/// `RolaCancellation *` it checks. The outcome is an owned JSON string the caller releases with
/// `free_string`, tagged by `kind`: `{"kind":"paths","paths":[{"did":"...","what":"..."}]}`,
/// `{"kind":"tree","path":"..."}`, `{"kind":"tree_file","path":"...","staged":...}` or
/// `{"kind":"refused","refusal":{...}}`. A failure is a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::Json`] when `request` does not read as an [`AlignRequest`], and whatever
/// [`AlignRequest::run`] names.
#[lazyffi(export = rola_write_align)]
pub fn align_at(
    request: &str,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<String, WriteError> {
    let request: AlignRequest =
        serde_json::from_str(request).map_err(|error| WriteError::Json {
            reason: error.to_string(),
        })?;
    let watching = crate::write::progress::watching(progress);
    let outcome = request.run(&watching, cancellation)?;

    serde_json::to_string(&OutcomeJson::from(&outcome)).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// The Vaults a run may bring content from, or nothing when it has no account to speak as.
///
/// The progress the caller handed in is what the exchanges report to, and the concurrency is what a
/// transfer of many keys is spread over.
///
/// # Errors
///
/// Returns [`WriteError::Internal`] when the runtime the exchanges are waited on could not be made.
fn sources_of<'a>(carry: &Carry<'a>) -> Result<Option<Sources<'a>>, WriteError> {
    let Some(account) = carry.account else {
        return Ok(None);
    };
    let sources = Sources::new(
        carry.workspace,
        account,
        carry.candidates.to_vec(),
        carry.offline,
    )
    .map_err(internal)?
    .reporting(carry.progress.clone())
    .with_concurrency(carry.concurrency);

    Ok(Some(sources))
}

/// The steps of a run that has a Layout path to settle, once anything a tree needed is done.
///
/// The reading is taken before the selection is read from it, and the selection is what the ignore
/// rules are asked about: a path the rules govern is refused before any of it is written.
#[allow(
    clippy::too_many_lines,
    reason = "it is one reading of the tree followed by the one thing the mode says about it, and a split would pass the same seven values through a second signature"
)]
fn after_tree(
    carry: &Carry<'_>,
    root: &Path,
    given: &str,
    mode: &Mode,
    options: Options,
    sources: Option<&Sources<'_>>,
    cancellation: &Cancellation,
) -> Result<Outcome, WriteError> {
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled);
    }

    let layout = carry.layout;
    let content = Content {
        storage: carry.storage,
        index: carry.index,
        sources,
    };

    // The reading is taken unfiltered: a variant file the tree holds somewhere unexpected is found
    // among the untagged, and the ignore rules filtering them would hide exactly the files a merge
    // is waiting for. The rules' say over the run is made at the selection instead, below.
    let diff = tree_diff_remembering(
        layout,
        carry.workspace,
        DEFAULT_ALIKE,
        &IgnoreRuleSet::empty(),
    )
    .map_err(internal)?;

    let selected = match select(root, carry.cwd, given, &diff) {
        Ok(selected) => selected,
        Err(refusal) => return Ok(Outcome::Refused(refusal)),
    };

    // What the ignore rules govern cannot be selected for settling: an entry the rules cover is
    // refused, unless the Layout already names it, since the rules have no jurisdiction over what
    // is tracked, or the reading found it to be where a tracked file moved, which is the work's own
    // and no rule's. `--include-ignored` is the run saying it means it.
    if !options.include_ignored {
        let governed = match &selected {
            Selector::One(path) => {
                layout.id_of(path).is_none() && !selectable(carry.rules, path, &diff.renamed)
            }
            Selector::Under(prefix) => {
                !prefix.is_empty() && carry.rules.is_ignore(Path::new(prefix), true)
            }
        };

        if governed {
            let path = match &selected {
                Selector::One(path) => path.as_str().to_owned(),
                Selector::Under(prefix) => prefix.clone(),
            };

            return Ok(Outcome::Refused(Refusal::Ignored { path }));
        }
    }

    // A variant file is not a path the Layout names, so the ways a Layout path is settled do not
    // apply to it. It may be where the record says or somewhere a move put it, and what settles it
    // is ending the merge, putting its file back, or writing down where it moved to.
    let mut merging = Merging::read(root);
    let moves = moved_variants(carry, &merging, root, &content, &diff);

    if let Selector::One(path) = &selected {
        let at = merging.variant_of(layout, path);
        let found = moves
            .iter()
            .find(|pair| &pair.from == path || &pair.to == path);

        if let Some(target) = at
            .map(Pending::target)
            .or_else(|| found.map(|pair| pair.target))
        {
            let variant = at
                .map(|pending| *pending.variant())
                .or_else(|| found.map(|pair| pair.variant));

            return match mode {
                Mode::Delete => single(unmerge(&mut merging, root, target, path)),
                Mode::Rename => found.map_or(Ok(Outcome::Refused(Refusal::Argument)), |pair| {
                    single(relocate_variant(
                        carry,
                        &mut merging,
                        root,
                        target,
                        &pair.from,
                        &pair.to,
                    ))
                }),
                Mode::Move(to) => single(assert_variant_move(
                    carry,
                    &mut merging,
                    root,
                    target,
                    path,
                    to,
                )),
                Mode::Restore | Mode::RestoreModify | Mode::RestoreDelete => variant
                    .map_or(Ok(Outcome::Refused(Refusal::Argument)), |variant| {
                        single(restore_variant(path, root, &content, variant))
                    }),
                _ => Ok(Outcome::Refused(Refusal::Argument)),
            };
        }
    }

    match (mode, &selected) {
        (Mode::Delete, Selector::One(path)) => {
            single(delete(carry, &diff, &mut merging, root, path))
        }
        (Mode::Rename, Selector::One(path)) => single(rename(carry, &diff, &merging, path, root)),
        (Mode::Break, Selector::One(path)) => single(take_back(carry, &diff, path, root)),
        (Mode::Move(target), Selector::One(path)) => {
            single(assert_move(carry, &diff, path, root, target))
        }
        (Mode::RestoreMove, Selector::One(path)) => single(restore_move(
            carry,
            &diff,
            path,
            root,
            &content,
            options.completely,
            cancellation,
        )),
        (Mode::RestoreModify, Selector::One(path)) => {
            single(restore_modify(carry, &diff, path, root, &content))
        }
        (Mode::RestoreStaged, Selector::One(path)) => Ok(Outcome::Refused(Refusal::NotStaged {
            path: path.as_str().to_owned(),
        })),
        (Mode::RestoreDelete, Selector::One(path)) => {
            single(restore_delete(carry, &diff, path, root, &content))
        }
        (Mode::Restore, Selector::One(path)) => single(restore_whichever(
            carry,
            &diff,
            path,
            root,
            &content,
            options.completely,
            cancellation,
        )),
        (Mode::Restore, Selector::Under(prefix)) => paths(restore_all(
            carry,
            &diff,
            prefix,
            root,
            &content,
            options.completely,
            cancellation,
        )),
        // A directory or `.` names many files, and only putting them back acts on many at once.
        (_, Selector::Under(_)) => Ok(Outcome::Refused(Refusal::Argument)),
    }
}

/// What a mode acts on: the one path named, or every path under a directory the run was made in.
enum Selector {
    /// One path, named by what was written.
    One(LayoutPath),
    /// Every path under the directory whose prefix this is, `""` naming the whole tree.
    Under(String),
}

/// Where content a restore writes comes from, and where an object the store lacks may be asked of.
///
/// Two lifetimes because the store and the index are borrowed from the run while the sources own
/// their own borrows of the Workspace and the account: the sources outlive the content that reads
/// them and cannot be made to share its lifetime.
struct Content<'s, 'b> {
    /// The store the content is read out of, when the run has one.
    storage: Option<&'s StorageSet>,
    /// The index that says which version names which object, when the run has one.
    index: Option<&'s VCSIndex>,
    /// Where an object the store does not hold may be brought from.
    sources: Option<&'s Sources<'b>>,
}

/// Why one step of an alignment stopped: a refusal the command words, or a write that failed.
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

/// One path's item as the outcome of a run.
fn single(item: Result<Item, Stop>) -> Result<Outcome, WriteError> {
    paths(item.map(|item| vec![item]))
}

/// Every path's items as the outcome of a run, a refusal staying a refusal.
fn paths(items: Result<Vec<Item>, Stop>) -> Result<Outcome, WriteError> {
    match items {
        Ok(items) => Ok(Outcome::Paths(items)),
        Err(Stop::Refused(refusal)) => Ok(Outcome::Refused(refusal)),
        Err(Stop::Failed(error)) => Err(error),
    }
}

/// A step that answered only done or stopped, as the outcome of a run.
fn step(step: Result<(), Stop>, outcome: Outcome) -> Result<Outcome, WriteError> {
    match step {
        Ok(()) => Ok(outcome),
        Err(Stop::Refused(refusal)) => Ok(Outcome::Refused(refusal)),
        Err(Stop::Failed(error)) => Err(error),
    }
}

/// The failure a step the library has no category for is answered with, in its own words.
#[allow(
    clippy::needless_pass_by_value,
    reason = "a call site hands over the error it already holds and drops, and a borrowed one would make every step write an `&` for a value nothing else needs"
)]
fn internal(error: impl ToString) -> WriteError {
    WriteError::Internal {
        reason: error.to_string(),
    }
}

/// The refusal a change to the Layout did not apply as, told apart the way a Layout command tells it.
fn layout_refusal(error: &LayoutError) -> Refusal {
    match error {
        LayoutError::AlreadyExists => Refusal::LayoutExists,
        LayoutError::NotFound => Refusal::LayoutMissing,
        other => Refusal::LayoutFailed {
            cause: other.to_string(),
        },
    }
}

/// Reads what was written into what the run is to act on.
///
/// What is written is a path where the run was made, as a shell would read it: `.` is the directory
/// the run is in and everything under it, and `..` climbs from there. A name that lands on a
/// directory selects everything under it, except a directory the reading found to be a side of a
/// move: a tree is a directory, and a move of one is settled as the one entry it is, not as every
/// path under it.
///
/// # Errors
///
/// Answers with [`Refusal::Argument`] when the path reaches out of the Workspace, or names nothing
/// a Layout could.
fn select(root: &Path, cwd: &Path, given: &str, diff: &TreeDiff) -> Result<Selector, Refusal> {
    let absolute = normalize(&resolve(cwd, given));

    let Ok(relative) = absolute.strip_prefix(root) else {
        return Err(Refusal::Argument);
    };

    if absolute.is_dir() {
        if let Ok(path) = LayoutPath::from_relative(relative)
            && diff
                .renamed
                .iter()
                .any(|pair| pair.from == path || pair.to == path)
        {
            return Ok(Selector::One(path));
        }

        return Ok(Selector::Under(prefix(relative)));
    }

    LayoutPath::from_relative(relative)
        .map(Selector::One)
        .map_err(|_| Refusal::Argument)
}

/// Whether `path` may be selected by a run reading with `rules`: not ignored, or a move's
/// destination, which a rule is not written against.
///
/// The destinations are the tree reading's own: a file that turned out to be where a tracked path
/// went is already the work's, however the rules read its new place.
fn selectable(rules: &IgnoreRuleSet, path: &LayoutPath, renames: &[PathRename]) -> bool {
    if renames.iter().any(|rename| rename.to == *path) {
        return true;
    }

    !rules.is_ignore(Path::new(path.as_str()), false)
}

/// The rules a tree's own work is read by, loaded without a word.
///
/// The command warns when a rules file cannot be read; the library has no words of its own, and a
/// tree put back is the work being asked for, so a rules file that does not read is read as none.
/// The rules are the Workspace's whatever the run said about the ignored: what a tree never
/// recorded is what `--completely` takes out, and that is not the ignore rules' question.
fn tree_rules(workspace: &Workspace) -> IgnoreRuleSet {
    IgnoreRuleSet::load(
        &workspace.get_root().join(WORKSPACE_IGNORE_RULES_DIR),
        workspace.get_root(),
    )
    .unwrap_or_else(|_| IgnoreRuleSet::empty())
}

/// The rules a request selects with, the empty set when it includes what they govern.
///
/// The command warns when a rules file cannot be read; the library has no words of its own, and a
/// request that includes the ignored is saying the rules do not select at all, so both end as the
/// empty set.
fn request_rules(workspace: &Workspace, include_ignored: bool) -> IgnoreRuleSet {
    if include_ignored {
        return IgnoreRuleSet::empty();
    }

    tree_rules(workspace)
}

/// The tree a Layout path sits in, when the Layout records one above it.
///
/// The path itself is looked at first, then its parents: a run that names a file inside a tree is
/// naming the tree, and a run that names a tree whose directory is gone is still naming a tree. The
/// Layout is the reading because the mark on disk is what a deleted tree no longer has.
fn find_tracked_tree(layout: &Layout, relative: &Path) -> Option<LayoutPath> {
    let mut components: Vec<&str> = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()?;
    if components.is_empty() {
        return None;
    }

    while !components.is_empty() {
        let path = LayoutPath::from_relative(Path::new(&components.join("/"))).ok()?;
        if let Some(id) = layout.id_of(&path)
            && layout
                .entry(id)
                .is_some_and(|data| data.kind() == EntryKind::Tree)
        {
            return Some(path);
        }

        components.pop();
    }

    None
}

/// The prefix the paths under `relative` share, or `""` for the Workspace root itself, which is the
/// one directory with no name of its own.
fn prefix(relative: &Path) -> String {
    LayoutPath::from_relative(relative)
        .map_or_else(|_| String::new(), |path| path.as_str().to_owned())
}

/// The path `given` names, made absolute against the directory the run was made in.
///
/// It mirrors `rola track`'s reading of a name, since a name is written the same way wherever it is
/// written, and the two commands act on the same paths.
fn resolve(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// Whether `path` is `prefix`'s directory or under it, with `""` naming the whole tree.
fn under(prefix: &str, path: &LayoutPath) -> bool {
    if prefix.is_empty() {
        return true;
    }

    path.as_str()
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// The move `path` is a side of, when the reading found one.
fn rename_of<'a>(diff: &'a TreeDiff, path: &LayoutPath) -> Option<&'a PathRename> {
    diff.renamed
        .iter()
        .find(|pair| &pair.from == path || &pair.to == path)
}

/// Puts the tree a run named back to what the Layout records, making it where nothing is.
///
/// # Errors
///
/// Returns [`WriteError::NoStore`] when there is no store to read the structure from,
/// [`WriteError::NoIndex`] when there is no index to resolve it through, and the failures
/// [`restore_tree`] names.
fn restore_tracked_tree(
    carry: &Carry<'_>,
    root: &Path,
    tree_path: &LayoutPath,
    completely: bool,
    cancellation: &Cancellation,
) -> Result<Outcome, WriteError> {
    let at = root.join(tree_path.to_path_buf());
    let tree = match Tree::locate(root, &at) {
        Ok(tree) if at.is_dir() => tree,
        _ => prepare_tree(root, tree_path)?,
    };
    let content = Content {
        storage: carry.storage,
        index: carry.index,
        sources: None,
    };

    step(
        restore_tree(carry, &tree, &content, completely, cancellation),
        Outcome::Tree {
            path: tree_path.as_str().to_owned(),
        },
    )
}

/// Puts one file inside the tree a run named back to what the tree records or what it has staged.
///
/// # Errors
///
/// Returns [`Refusal::TreeUnsupported`] when the path is the tree itself, the mark is not there, or
/// a per-file mode was asked of one, and the failures [`restore_tree_file`] names.
fn restore_tracked_file(
    carry: &Carry<'_>,
    root: &Path,
    tree_path: &LayoutPath,
    target: &Path,
    mode: &Mode,
    cancellation: &Cancellation,
) -> Result<Outcome, WriteError> {
    let tree_root = root.join(tree_path.to_path_buf());

    if target == tree_root.as_path() {
        return Ok(Outcome::Refused(Refusal::TreeUnsupported));
    }

    let tree = match Tree::locate(root, &tree_root) {
        Ok(tree) if tree_root.is_dir() => tree,
        _ => return Ok(Outcome::Refused(Refusal::TreeUnsupported)),
    };
    let content = Content {
        storage: carry.storage,
        index: carry.index,
        sources: None,
    };

    match restore_tree_file(
        carry,
        &tree,
        &tree_root,
        target,
        mode,
        &content,
        cancellation,
    ) {
        Ok(()) => Ok(Outcome::TreeFile {
            path: target.display().to_string(),
            staged: matches!(mode, Mode::RestoreStaged),
        }),
        Err(Stop::Refused(refusal)) => Ok(Outcome::Refused(refusal)),
        Err(Stop::Failed(error)) => Err(error),
    }
}

/// Makes a tree's directory and its skeleton where nothing is, so a deleted tree can be put back.
///
/// A path where a file sits and a tree now belongs is taken out of the way: the restore is written
/// over the work, and what the Layout records is a directory there.
fn prepare_tree(root: &Path, path: &LayoutPath) -> Result<Tree, WriteError> {
    let tree_root = root.join(path.to_path_buf());

    if tree_root.exists() && !tree_root.is_dir() {
        fs::remove_file(&tree_root).map_err(internal)?;
    }
    fs::create_dir_all(&tree_root).map_err(internal)?;

    Staging::new(&tree_root).ensure_store().map_err(internal)?;

    Tree::open(root, &tree_root).map_err(internal)
}

/// Puts one file inside a tree back to what the tree records or what it has staged.
///
/// A tree is one Layout entry, but a file inside it has two structures of its own to come from: the
/// version the Layout names, and the staging area that version is being changed into. `--restore`
/// names the whole tree, so this is the per-file answer, and a path the chosen structure does not
/// name is taken out of the work rather than left as a change against it.
fn restore_tree_file(
    carry: &Carry<'_>,
    tree: &Tree,
    tree_root: &Path,
    target: &Path,
    mode: &Mode,
    content: &Content<'_, '_>,
    cancellation: &Cancellation,
) -> Result<(), Stop> {
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled.into());
    }

    let Some(store) = content.storage else {
        return Err(WriteError::NoStore.into());
    };
    let Some(index) = content.index else {
        return Err(WriteError::NoIndex.into());
    };
    let runtime = tokio::runtime::Runtime::new().map_err(internal)?;

    match mode {
        Mode::RestoreModify => {
            let Some(recorded) = runtime
                .block_on(tree.recorded(carry.layout, index, store))
                .map_err(internal)?
            else {
                return Err(Refusal::Failed {
                    cause: carry.messages.not_recorded.clone(),
                }
                .into());
            };

            write_tree_entry(
                tree_root,
                target,
                &recorded.structure,
                store,
                &runtime,
                carry.messages,
            )?;
        }
        Mode::RestoreStaged => {
            let staging = tree.staging();
            let Some(structure) = staging.structure().map_err(internal)? else {
                return Err(Refusal::NotStaged {
                    path: target.display().to_string(),
                }
                .into());
            };
            let Some(staged) = staging.store() else {
                return Err(Refusal::NotStaged {
                    path: target.display().to_string(),
                }
                .into());
            };

            write_tree_entry(
                tree_root,
                target,
                &structure,
                &StorageSet::sole(staged),
                &runtime,
                carry.messages,
            )?;
        }
        _ => return Err(Refusal::TreeUnsupported.into()),
    }

    Ok(())
}

/// Writes the entry `structure` holds for `target`, or takes `target` away when it holds none.
fn write_tree_entry(
    tree_root: &Path,
    target: &Path,
    structure: &TreeStructure,
    store: &StorageSet,
    runtime: &tokio::runtime::Runtime,
    messages: &Messages,
) -> Result<(), Stop> {
    let unsupported = || Refusal::Failed {
        cause: messages.unsupported.clone(),
    };
    let relative = target
        .strip_prefix(tree_root)
        .ok()
        .and_then(Path::to_str)
        .map(|path| path.replace('\\', "/"))
        .ok_or_else(unsupported)?;

    let Some(entry) = structure.entry(&relative) else {
        if target.is_file() || target.is_symlink() {
            fs::remove_file(target).map_err(internal)?;
        }

        return Ok(());
    };

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(internal)?;
    }

    match entry.kind() {
        TreeEntryKind::File => {
            let Some(key) = entry.storage() else {
                return Err(unsupported().into());
            };
            if target.is_dir() {
                return Err(unsupported().into());
            }
            runtime.block_on(extract_file(store, key, target))?;
        }
        TreeEntryKind::Symlink => {
            let Some(link) = entry.target() else {
                return Err(unsupported().into());
            };
            if target.exists() {
                let _ = fs::remove_file(target);
            }

            symlink(link, target).map_err(internal)?;
        }
        TreeEntryKind::Directory => {
            return Err(Refusal::TreeUnsupported.into());
        }
    }

    Ok(())
}

/// Puts the work of a tree back to the structure the Layout records.
///
/// What the version holds is written over the paths it names; `--completely` also takes out what the
/// tree never recorded.
fn restore_tree(
    carry: &Carry<'_>,
    tree: &Tree,
    content: &Content<'_, '_>,
    completely: bool,
    cancellation: &Cancellation,
) -> Result<(), Stop> {
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled.into());
    }

    let Some(store) = content.storage else {
        return Err(WriteError::NoStore.into());
    };
    let Some(index) = content.index else {
        return Err(WriteError::NoIndex.into());
    };
    let layout = carry.layout;
    let not_recorded = || Refusal::Failed {
        cause: carry.messages.not_recorded.clone(),
    };

    let runtime = tokio::runtime::Runtime::new().map_err(internal)?;

    let Ok(path) = tree.layout_path() else {
        return Err(not_recorded().into());
    };
    let Some(id) = layout.id_of(&path) else {
        return Err(not_recorded().into());
    };
    let Some(data) = layout.entry(id) else {
        return Err(not_recorded().into());
    };

    let version = data.version();
    if version == [0; 32] {
        return Err(not_recorded().into());
    }

    let target = runtime
        .block_on(read_structure(index, store, &version))
        .map_err(internal)?;

    let root = tree.root();
    fs::create_dir_all(root).map_err(internal)?;

    for entry in target.entries() {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled.into());
        }

        let disk = root.join(entry.path());

        match entry.kind() {
            TreeEntryKind::Directory => {
                fs::create_dir_all(&disk).map_err(internal)?;
            }
            TreeEntryKind::File => {
                let Some(storage_key) = entry.storage() else {
                    continue;
                };
                if let Some(parent) = disk.parent() {
                    fs::create_dir_all(parent).map_err(internal)?;
                }
                runtime.block_on(extract_file(store, storage_key, &disk))?;
            }
            TreeEntryKind::Symlink => {
                let Some(link) = entry.target() else {
                    continue;
                };
                if disk.exists() {
                    let _ = fs::remove_file(&disk);
                }
                if let Some(parent) = disk.parent() {
                    fs::create_dir_all(parent).map_err(internal)?;
                }

                symlink(link, &disk).map_err(internal)?;
            }
        }
    }

    let rules = tree.rules(tree_rules(carry.workspace)).map_err(internal)?;
    let scanned = tree.scan(&rules, false).map_err(internal)?;

    if completely {
        for entry in scanned.structure.entries() {
            if target.entry(entry.path()).is_some() {
                continue;
            }

            let disk = root.join(entry.path());
            match entry.kind() {
                TreeEntryKind::Directory => {
                    let _ = fs::remove_dir(&disk);
                }
                TreeEntryKind::File | TreeEntryKind::Symlink => {
                    let _ = fs::remove_file(&disk);
                }
            }
        }
    }

    // The work is the version again, so the baseline says the version is what the tree records. A
    // tree put back from a deleted directory has no baseline left otherwise, and the next reading
    // would have to rebuild one before it could tell that the work agrees.
    let baseline = tree.baseline();
    baseline.write(&target, &version).map_err(internal)?;
    baseline.write_entries(&scanned.cache).map_err(internal)?;
    // A tree put back is one the Layout still names, so the id it keeps locally is the entry's.
    tree.write_local_id(id).map_err(internal)?;

    Ok(())
}

/// Confirms that a path the Layout names and the tree does not hold is gone.
fn delete(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    merging: &mut Merging,
    root: &Path,
    path: &LayoutPath,
) -> Result<Item, Stop> {
    let named = || Refusal::NotLost {
        path: path.as_str().to_owned(),
    };

    if !diff.lost.contains(path) {
        return Err(named().into());
    }

    let Some(id) = carry.layout.id_of(path) else {
        return Err(named().into());
    };

    carry
        .layout
        .remove_entry(id)
        .map_err(|error| layout_refusal(&error))?;

    // A file the Layout no longer names has no merge waiting for it: its variant file, if there is
    // one, is left where it lies as a file of its own.
    if merging.remove(id)
        && let Err(error) = merging.write(root)
    {
        return Err(internal(error).into());
    }

    Ok(Item {
        did: Did::Deleted,
        what: path.as_str().to_owned(),
    })
}

/// Ends the merge a variant file was brought in for, taking its record away.
///
/// The variant file itself is not touched: the run that ends a merge has already taken it away, and
/// one that is still there is a file the reading will report in its own words.
fn unmerge(
    merging: &mut Merging,
    root: &Path,
    target: Uuid,
    path: &LayoutPath,
) -> Result<Item, Stop> {
    // The file being gone is what ends the merge, as a path being gone is what confirms a loss: one
    // still there is a merge still waiting, and a run that wants it gone takes it away first.
    if root.join(path.to_path_buf()).is_file() {
        return Err(Refusal::NotLost {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    merging.remove(target);

    if let Err(error) = merging.write(root) {
        return Err(internal(error).into());
    }

    Ok(Item {
        did: Did::Unmerged,
        what: path.as_str().to_owned(),
    })
}

/// The variant files the tree holds somewhere other than where the merge records put them.
///
/// It is the finding `rola status` makes, so what a reading reports and what an alignment settles
/// are one move seen twice: an untagged file whose content is the variant's own is the file that
/// moved, and nothing else can be.
fn moved_variants(
    carry: &Carry<'_>,
    merging: &Merging,
    root: &Path,
    content: &Content<'_, '_>,
    diff: &TreeDiff,
) -> Vec<VariantMove> {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return Vec::new();
    };

    variant_moves(
        merging,
        carry.layout,
        root,
        content.index,
        &runtime,
        &diff.untagged,
    )
}

/// Writes down that the variant file waiting for `target` now lies at `to`.
fn relocate_variant(
    carry: &Carry<'_>,
    merging: &mut Merging,
    root: &Path,
    target: Uuid,
    from: &LayoutPath,
    to: &LayoutPath,
) -> Result<Item, Stop> {
    let not_lost = || Refusal::NotLost {
        path: from.as_str().to_owned(),
    };

    let Some(target_path) = carry.layout.path_of(target) else {
        return Err(not_lost().into());
    };

    let place = Merging::place_of(&target_path, to);

    if !merging.relocate(target, Some(place)) {
        return Err(not_lost().into());
    }

    if let Err(error) = merging.write(root) {
        return Err(internal(error).into());
    }

    Ok(Item {
        did: Did::Moved,
        what: format!("{} -> {}", from.as_str(), to.as_str()),
    })
}

/// Asserts that the variant file waiting for `target` moved to `to`, for a reading that found no
/// move of its own.
fn assert_variant_move(
    carry: &Carry<'_>,
    merging: &mut Merging,
    root: &Path,
    target: Uuid,
    from: &LayoutPath,
    to: &str,
) -> Result<Item, Stop> {
    let Ok(to) = LayoutPath::new(to) else {
        return Err(Refusal::Move {
            path: to.to_owned(),
            kind: MoveKind::Missing,
        }
        .into());
    };

    // A move asserted to somewhere nothing is, is a move to nowhere: the file has to be where the
    // run says, as a path asserted for a Layout entry does.
    if !root.join(to.to_path_buf()).is_file() {
        return Err(Refusal::Move {
            path: to.as_str().to_owned(),
            kind: MoveKind::Missing,
        }
        .into());
    }

    relocate_variant(carry, merging, root, target, from, &to)
}

/// Puts a variant file back, taking the content the variant was brought in with out of the store.
///
/// It is the other half of ending a merge: a variant file taken away can be brought back as long as
/// the record waits, and the content it is written from is the variant's own rather than a version's.
fn restore_variant(
    path: &LayoutPath,
    root: &Path,
    content: &Content<'_, '_>,
    variant: Blake3Hash,
) -> Result<Item, Stop> {
    let disk = root.join(path.to_path_buf());

    if disk.exists() {
        return Err(Refusal::RestoreExists {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    let Some(vcs) = content.index else {
        return Err(WriteError::NoIndex.into());
    };
    let runtime = tokio::runtime::Runtime::new().map_err(internal)?;
    let stored = variant_storage(vcs, &runtime, variant).map_err(internal)?;

    write_stored(content, &runtime, stored, &disk)?;

    Ok(Item {
        did: Did::RestoredMerge,
        what: path.as_str().to_owned(),
    })
}

/// Confirms a move the reading found.
///
/// The path the file is now at is one the Layout names, and what the Layout names there is the
/// version the file held before it moved. A move that was edited is therefore left changed, and
/// what the reading wrote down says the two agree, so the disagreement is written down here rather
/// than left for a reading that would never find it.
fn rename(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    merging: &Merging,
    path: &LayoutPath,
    root: &Path,
) -> Result<Item, Stop> {
    let layout = carry.layout;
    let Some(pair) = rename_of(diff, path) else {
        return Err(Refusal::NoRename {
            path: path.as_str().to_owned(),
        }
        .into());
    };

    layout
        .move_path(&pair.from, &pair.to)
        .map_err(|error| layout_refusal(&error))?;

    // A file a variant is waiting for takes that variant file with it.
    if let Some(target) = layout.id_of(&pair.to)
        && let Err(error) = merging.follow(root, target, &pair.from, &pair.to)
    {
        return Err(internal(error).into());
    }

    remember_renamed(carry, root, &pair.to, diff.modified.contains(&pair.to))?;

    Ok(Item {
        did: Did::Moved,
        what: format!("{} -> {}", pair.from.as_str(), pair.to.as_str()),
    })
}

/// Writes down where a confirmed move left the file: what the Layout now names, and whether the
/// tree still holds it.
fn remember_renamed(
    carry: &Carry<'_>,
    root: &Path,
    to: &LayoutPath,
    modified: bool,
) -> Result<(), Stop> {
    let disk = root.join(to.to_path_buf());
    if !disk.is_file() {
        return Ok(());
    }

    let path = cache_path(root, carry.layout);
    let mut cache = Cache::read(&path);

    let entry = match cache.get(to).cloned() {
        Some(entry) => entry,
        None => entry_of(&disk).map_err(internal)?,
    };

    cache.insert(to.clone(), if modified { entry.disagreed() } else { entry });
    cache.write(&path).map_err(internal)?;

    Ok(())
}

/// Takes back a move the reading guessed at, leaving the two paths a loss and a gain.
///
/// The reading pairs two text files by how alike they are, and keeps that likeness in what it
/// wrote down about the lost path; forgetting it leaves the digest, which is what the path is
/// remembered by, and stops the two being paired. A move of two files that hold the same bytes is
/// one the reading found by digest, not by likeness, and is a fact rather than a guess.
fn take_back(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
) -> Result<Item, Stop> {
    let Some(pair) = rename_of(diff, path) else {
        return Err(Refusal::NoRename {
            path: path.as_str().to_owned(),
        }
        .into());
    };

    // A strong move is one the two being the same bytes makes: taking it back would be denying what
    // is there, so it is refused. Only a move the likeness only suggests is a guess to be undone.
    if pair.strong {
        return Err(Refusal::RealMove {
            from: pair.from.as_str().to_owned(),
            to: pair.to.as_str().to_owned(),
        }
        .into());
    }

    let cache_file = cache_path(root, carry.layout);
    let mut cache = Cache::read(&cache_file);

    let Some(was) = cache.get(&pair.from).cloned() else {
        return Err(Refusal::NoRename {
            path: path.as_str().to_owned(),
        }
        .into());
    };

    cache.insert(pair.from.clone(), was.without_spans());
    cache.write(&cache_file).map_err(internal)?;

    Ok(Item {
        did: Did::Broke,
        what: format!("{} / {}", pair.from.as_str(), pair.to.as_str()),
    })
}

/// Asserts that a lost path moved to `target`, so the next reading pairs them.
///
/// The reading knows a path moved by what it wrote down about the path that is gone; what is
/// written down here is what `target` holds now, so the two are the same to the next reading. What
/// changes is the memory, not the Layout: `--rename` is still what moves the path. A tree has no
/// content to write down, so what is written is the id of the lost entry taken into the tree's
/// mark, which is the whole of what a tree moving takes with it.
fn assert_move(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    target: &str,
) -> Result<Item, Stop> {
    let layout = carry.layout;
    let not_lost = || Refusal::NotLost {
        path: path.as_str().to_owned(),
    };

    if !diff.lost.contains(path) {
        return Err(not_lost().into());
    }

    let Some(id) = layout.id_of(path) else {
        return Err(not_lost().into());
    };

    let Ok(target) = LayoutPath::new(target) else {
        return Err(Refusal::Move {
            path: target.to_owned(),
            kind: MoveKind::Missing,
        }
        .into());
    };

    if layout.id_of(&target).is_some() {
        return Err(Refusal::Move {
            path: target.as_str().to_owned(),
            kind: MoveKind::Tracked,
        }
        .into());
    }

    let disk = root.join(target.to_path_buf());

    // A tree is a directory, and what it moved with is the id in its mark: writing the lost entry's
    // id there is what makes the next reading pair the two. A file is instead what it holds, which
    // is what the reading remembers until it can pair the paths itself.
    if layout
        .entry(id)
        .is_some_and(|data| data.kind() == EntryKind::Tree)
    {
        let Ok(tree) = Tree::open(root, &disk) else {
            return Err(Refusal::Move {
                path: target.as_str().to_owned(),
                kind: MoveKind::NotATree,
            }
            .into());
        };

        tree.write_local_id(id).map_err(internal)?;

        return Ok(Item {
            did: Did::Asserted,
            what: format!("{} -> {}", path.as_str(), target.as_str()),
        });
    }

    if !disk.is_file() {
        return Err(Refusal::Move {
            path: target.as_str().to_owned(),
            kind: MoveKind::Missing,
        }
        .into());
    }

    let entry = entry_of(&disk).map_err(internal)?;

    let cache_file = cache_path(root, layout);
    let mut cache = Cache::read(&cache_file);
    cache.insert(path.clone(), entry);
    cache.write(&cache_file).map_err(internal)?;

    Ok(Item {
        did: Did::Asserted,
        what: format!("{} -> {}", path.as_str(), target.as_str()),
    })
}

/// Puts a moved file back where the Layout names it.
///
/// The Layout still names the path the file was at, since the move was never confirmed, so putting
/// the file back there undoes the move on disk. What the file holds is not touched, one that was
/// edited as well as moved is left changed at its old path, which is then `--restore-modify`'s to
/// settle, unless `completely`, which puts the version the Layout names back over it too, so the
/// move is undone in content as well as in place.
fn restore_move(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: &Content<'_, '_>,
    completely: bool,
    cancellation: &Cancellation,
) -> Result<Item, Stop> {
    let Some(pair) = rename_of(diff, path) else {
        return Err(Refusal::NoRename {
            path: path.as_str().to_owned(),
        }
        .into());
    };

    let from_disk = root.join(pair.from.to_path_buf());
    let to_disk = root.join(pair.to.to_path_buf());

    // What the Layout names is where the file goes; a file already there is one the restore would
    // have to take away, and a restore does not take anything away.
    if from_disk.exists() {
        return Err(Refusal::RestoreExists {
            path: pair.from.as_str().to_owned(),
        }
        .into());
    }

    if let Some(parent) = from_disk.parent() {
        fs::create_dir_all(parent).map_err(internal)?;
    }

    if let Err(error) = fs::rename(&to_disk, &from_disk) {
        return Err(internal(error).into());
    }

    let what = format!("{} <- {}", pair.from.as_str(), pair.to.as_str());

    if completely {
        // A tree moved whole, so what is at the old path is the tree again. Putting the version
        // back over it is the tree's own restore, rather than a file's bytes at the path.
        let tree = carry
            .layout
            .id_of(&pair.from)
            .and_then(|id| carry.layout.entry(id))
            .is_some_and(|data| data.kind() == EntryKind::Tree);
        if tree {
            let tree = Tree::open(root, &from_disk).map_err(internal)?;
            restore_tree(carry, &tree, content, true, cancellation)?;
        } else {
            put_version(carry, &pair.from, root, content)?;
        }

        return Ok(Item {
            did: Did::RestoredCompletely,
            what,
        });
    }

    Ok(Item {
        did: Did::RestoredMove,
        what,
    })
}

/// Puts a changed file back to the version the Layout names, writing over what it holds.
fn restore_modify(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: &Content<'_, '_>,
) -> Result<Item, Stop> {
    // A path the Layout does not name has no version to go back to: a move's destination is one of
    // these, which is why a move is put back first and only then has its content settled.
    if !diff.modified.contains(path) || carry.layout.id_of(path).is_none() {
        return Err(Refusal::NotModified {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    put_version(carry, path, root, content)?;

    Ok(Item {
        did: Did::RestoredModify,
        what: path.as_str().to_owned(),
    })
}

/// Puts a deleted file back, taking the version the Layout names out of the store.
fn restore_delete(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: &Content<'_, '_>,
) -> Result<Item, Stop> {
    if !diff.lost.contains(path) || carry.layout.id_of(path).is_none() {
        return Err(Refusal::NotLost {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    // A lost path is one the tree does not hold, so putting it back has nothing to land on: a file
    // already there is something to be told about rather than written over.
    if root.join(path.to_path_buf()).exists() {
        return Err(Refusal::RestoreExists {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    put_version(carry, path, root, content)?;

    Ok(Item {
        did: Did::RestoredDelete,
        what: path.as_str().to_owned(),
    })
}

/// Puts a lost tree back whole, whether its directory is still there or was deleted.
///
/// It is the same work naming the tree does: the directory and its mark are made where nothing is,
/// and the structure the Layout records is written over the paths it names.
fn restore_lost_tree(
    carry: &Carry<'_>,
    path: &LayoutPath,
    root: &Path,
    content: &Content<'_, '_>,
    completely: bool,
    cancellation: &Cancellation,
) -> Result<Item, Stop> {
    let at = root.join(path.to_path_buf());
    let tree = match Tree::locate(root, &at) {
        Ok(tree) if at.is_dir() => tree,
        _ => prepare_tree(root, path)?,
    };

    restore_tree(carry, &tree, content, completely, cancellation)?;

    Ok(Item {
        did: Did::RestoredDelete,
        what: path.as_str().to_owned(),
    })
}

/// Writes the version stored under `version` out to `disk`, making the way to it.
///
/// It is what putting a file back is made of, whether the file is one that changed or one that is
/// gone: the Layout names a version, the store holds the content it was made of, and the two are put
/// back together at the path. Content the store does not hold is brought from a Vault first, when
/// the run has one to ask.
fn write_recorded(content: &Content<'_, '_>, version: [u8; 32], disk: &Path) -> Result<(), Stop> {
    let Some(vcs) = content.index else {
        return Err(WriteError::NoIndex.into());
    };

    let runtime = tokio::runtime::Runtime::new().map_err(internal)?;
    let recorded = recorded_hash(vcs, &runtime, version).map_err(internal)?;

    write_stored(content, &runtime, recorded, disk)
}

/// Writes the content stored under `stored` out to `disk`, making the way to it.
///
/// A version names its content through the variant it points at, while a variant file is written
/// from what the variant holds directly; both end here, where the content is brought when the store
/// does not have it and then put at the path.
fn write_stored(
    content: &Content<'_, '_>,
    runtime: &tokio::runtime::Runtime,
    stored: [u8; 32],
    disk: &Path,
) -> Result<(), Stop> {
    let Some(store) = content.storage else {
        return Err(WriteError::NoStore.into());
    };

    if let Some(parent) = disk.parent() {
        fs::create_dir_all(parent).map_err(internal)?;
    }

    if let Some(sources) = content.sources {
        sources.bring(store, &[Key::new(stored)]);
    }

    runtime.block_on(extract_file(store, &Key::new(stored), disk))?;

    Ok(())
}

/// The content the variant `hash` was brought in with.
fn variant_storage(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: Blake3Hash,
) -> Result<Blake3Hash, String> {
    let variant = runtime
        .block_on(vcs.read(Key::new(hash)))
        .map_err(|error| error.reason())?
        .expect_variant()
        .map_err(|error| error.to_string())?;

    Ok(*variant.storage_hash())
}

/// Writes down that a path put back holds what the Layout names again.
///
/// Putting a file back is what makes the two agree, so what the reading remembers of the path has to
/// say so: a path left marked as disagreed, by a move that was edited and confirmed, would go on
/// being reported as changed after its content was put back.
fn remember_restored(carry: &Carry<'_>, root: &Path, path: &LayoutPath) -> Result<(), Stop> {
    let disk = root.join(path.to_path_buf());
    if !disk.is_file() {
        return Ok(());
    }

    let cache_file = cache_path(root, carry.layout);
    let mut cache = Cache::read(&cache_file);
    let entry = entry_of(&disk).map_err(internal)?;

    cache.insert(path.clone(), entry);
    cache.write(&cache_file).map_err(internal)?;

    Ok(())
}

/// Writes the version the Layout names for `path` over what is there, and remembers that it does.
///
/// A file's content is put back by naming what the Layout holds for it and letting the store hand
/// that version out at the path, whether the file is one that changed or one that is gone, since
/// both want the same thing written, and remembering it is what stops the path being reported as
/// changed once it agrees again.
fn put_version(
    carry: &Carry<'_>,
    path: &LayoutPath,
    root: &Path,
    content: &Content<'_, '_>,
) -> Result<(), Stop> {
    let not_restorable = || Refusal::NotRestorable {
        path: path.as_str().to_owned(),
    };

    let Some(id) = carry.layout.id_of(path) else {
        return Err(not_restorable().into());
    };
    let Some(data) = carry.layout.entry(id) else {
        return Err(not_restorable().into());
    };

    write_recorded(content, data.version(), &root.join(path.to_path_buf()))?;

    remember_restored(carry, root, path)
}

/// Puts the path back to what the Layout names, whichever of the three it needs.
fn restore_whichever(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: &Content<'_, '_>,
    completely: bool,
    cancellation: &Cancellation,
) -> Result<Item, Stop> {
    if rename_of(diff, path).is_some() {
        return restore_move(carry, diff, path, root, content, completely, cancellation);
    }

    if diff.lost.contains(path) && carry.layout.id_of(path).is_some() {
        return restore_delete(carry, diff, path, root, content);
    }

    if diff.modified.contains(path) && carry.layout.id_of(path).is_some() {
        return restore_modify(carry, diff, path, root, content);
    }

    Err(Refusal::NotRestorable {
        path: path.as_str().to_owned(),
    }
    .into())
}

/// Puts every path under a directory back to what the Layout names, each by what it needs.
///
/// A move is settled first: its destination is not a path the Layout names, so content can only be
/// put back once the file is where the Layout names it. What is left, a path the tree no longer
/// holds, and one that changed where it is, is put back after. A path the tree holds and no Layout
/// names (`untagged`) is left where it lies, since there is nothing it is to be put back to.
fn restore_all(
    carry: &Carry<'_>,
    diff: &TreeDiff,
    prefix: &str,
    root: &Path,
    content: &Content<'_, '_>,
    completely: bool,
    cancellation: &Cancellation,
) -> Result<Vec<Item>, Stop> {
    let mut items = Vec::new();

    for pair in &diff.renamed {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled.into());
        }
        if !(under(prefix, &pair.from) || under(prefix, &pair.to)) {
            continue;
        }

        items.push(restore_move(
            carry,
            diff,
            &pair.from,
            root,
            content,
            completely,
            cancellation,
        )?);
    }

    for path in &diff.lost {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled.into());
        }
        let Some(id) = carry.layout.id_of(path) else {
            continue;
        };
        if !under(prefix, path) {
            continue;
        }

        // A lost tree is put back whole, as it is when the tree itself is named: what the version
        // holds is a structure rather than a file's bytes, and writing it at the path would leave a
        // structure object where a directory belongs.
        if carry
            .layout
            .entry(id)
            .is_some_and(|data| data.kind() == EntryKind::Tree)
        {
            items.push(restore_lost_tree(
                carry,
                path,
                root,
                content,
                completely,
                cancellation,
            )?);

            continue;
        }

        items.push(restore_delete(carry, diff, path, root, content)?);
    }

    for path in &diff.modified {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled.into());
        }
        if !under(prefix, path) || carry.layout.id_of(path).is_none() {
            continue;
        }

        items.push(restore_modify(carry, diff, path, root, content)?);
    }

    Ok(items)
}

/// The stored hash the version `hash` names, as the Layout records it.
fn recorded_hash(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: [u8; 32],
) -> Result<[u8; 32], String> {
    let version = runtime
        .block_on(vcs.read(Key::new(hash)))
        .map_err(|error| error.reason())?
        .expect_version()
        .map_err(|error| error.to_string())?;

    let variant = runtime
        .block_on(vcs.read(Key::new(*version.variant())))
        .map_err(|error| error.reason())?
        .expect_variant()
        .map_err(|error| error.to_string())?;

    Ok(*variant.storage_hash())
}

/// Points `link` at `target`, on either platform a tree may be on.
#[cfg(unix)]
fn symlink(target: &str, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Points `link` at `target`, on either platform a tree may be on.
#[cfg(windows)]
fn symlink(target: &str, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_ignore::IgnoreRuleSet;
    use rorolala_layout::LayoutPath;
    use rorolala_utils_location::Locate as _;
    use rorolala_utils_progress::Progress;
    use rorolala_vault::Vault;
    use rorolala_vcs::VCSIndex;
    use rorolala_workspace::Workspace;

    use super::{
        AlignMode, AlignRequest, Carry, Did, Item, Messages, Mode, MoveKind, Options, Outcome,
        OutcomeJson, Refusal, Settled, align_entry, align_vault_entry, settle,
    };
    use crate::write::Cancellation;
    use crate::write::request::Common;
    use crate::write::track::{TrackOptions, track_file, track_vault_file};

    #[test]
    fn a_path_is_aligned_to_a_version_and_a_description() {
        let dir = std::env::temp_dir().join(format!("rola-write-align-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();

        let aligned = align_entry(
            &workspace,
            &dir,
            "a.txt",
            &first.version.to_string(),
            Some("alice"),
            "aligned",
            Some("share"),
        )
        .unwrap();

        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");
        let layout = workspace.layout().unwrap();
        let entry = layout.entry(aligned.id).unwrap();
        assert_eq!(entry.version(), *first.version.digest());
        assert_eq!(entry.description(), "aligned");
        assert_eq!(entry.owner(), Some("alice"));
        assert_eq!(entry.preference(), Some("share"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_path_is_aligned_to_a_version() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-align-vault-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Vault::create(&dir).unwrap();
        let vault = Vault::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        let first = track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();

        fs::write(dir.join("a.txt"), b"two").unwrap();
        track_vault_file(
            &vault,
            &dir,
            "a.txt",
            "alice",
            "second",
            TrackOptions::default(),
        )
        .unwrap();

        let aligned = align_vault_entry(
            &vault,
            &dir,
            "a.txt",
            &first.version.to_string(),
            Some("alice"),
            "aligned",
            Some("share"),
        )
        .unwrap();

        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");
        let layout = vault.layout().unwrap();
        let entry = layout.entry(aligned.id).unwrap();
        assert_eq!(entry.version(), *first.version.digest());
        assert_eq!(entry.description(), "aligned");
        assert_eq!(entry.owner(), Some("alice"));
        assert_eq!(entry.preference(), Some("share"));

        let _ = fs::remove_dir_all(&dir);
    }

    /// Settles one path over `dir`, with nothing to fetch from.
    fn settled(dir: &std::path::Path, path: &str, mode: &Mode, options: Options) -> Settled {
        let workspace = Workspace::locate(dir).unwrap();
        let layout = workspace.layout().unwrap();
        let rules = IgnoreRuleSet::empty();
        let messages = Messages::default();
        let progress = Progress::silent();
        let index = VCSIndex::locate(dir);
        let storage = workspace.storage_set().ok();
        let carry = Carry {
            workspace: &workspace,
            layout: &layout,
            cwd: dir,
            rules: &rules,
            storage: storage.as_ref(),
            index: index.as_ref(),
            account: None,
            candidates: &[],
            offline: true,
            concurrency: 1,
            progress: &progress,
            messages: &messages,
        };

        settle(&carry, path, mode, options, &Cancellation::new())
    }

    /// A run that confirms a loss takes the entry out of the Layout and says so.
    #[test]
    fn a_lost_path_is_confirmed_gone() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-align-gone-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();
        fs::remove_file(dir.join("a.txt")).unwrap();

        let settled = settled(&dir, "a.txt", &Mode::Delete, Options::default());

        match settled.outcome.expect("the run settles") {
            Outcome::Paths(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].did, Did::Deleted);
                assert_eq!(items[0].what, "a.txt");
            }
            other => panic!("{other:?}"),
        }
        assert!(
            workspace
                .layout()
                .unwrap()
                .id_of(&LayoutPath::new("a.txt").unwrap())
                .is_none()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A run that puts a deleted file back writes the version the Layout names over it.
    #[test]
    fn a_deleted_file_is_put_back_out_of_the_store() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-align-restore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();
        fs::remove_file(dir.join("a.txt")).unwrap();

        let settled = settled(&dir, "a.txt", &Mode::RestoreDelete, Options::default());

        match settled.outcome.expect("the run settles") {
            Outcome::Paths(items) => assert_eq!(items[0].did, Did::RestoredDelete),
            other => panic!("{other:?}"),
        }
        assert_eq!(fs::read(dir.join("a.txt")).unwrap(), b"one");

        let _ = fs::remove_dir_all(&dir);
    }

    /// A request document names its mode, and leaves the two flags out when they are not said.
    #[test]
    fn an_alignment_request_names_its_mode() {
        let request: AlignRequest = serde_json::from_str(
            r#"{"workspace":"/work","path":"a.txt","mode":{"kind":"move_to","path":"b.txt"}}"#,
        )
        .expect("the request reads");

        assert_eq!(
            request.mode,
            AlignMode::MoveTo {
                path: "b.txt".to_owned()
            }
        );
        assert!(!request.completely);
        assert!(!request.include_ignored);
    }

    /// A request runs the alignment it names out of the Workspace it names.
    #[test]
    fn an_alignment_request_runs_out_of_its_workspace() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-align-request-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"one").unwrap();
        track_file(
            &workspace,
            &dir,
            "a.txt",
            "alice",
            "first",
            TrackOptions::default(),
        )
        .unwrap();
        fs::remove_file(dir.join("a.txt")).unwrap();

        let request = AlignRequest {
            common: Common {
                workspace: dir.clone(),
                ..Common::default()
            },
            // Relative, to prove it is read against the Workspace root rather than the process
            // directory the test binary happens to run in.
            path: "a.txt".to_owned(),
            mode: AlignMode::Delete,
            completely: false,
            include_ignored: false,
        };

        match request
            .run(&Progress::silent(), &Cancellation::new())
            .expect("the run settles")
        {
            Outcome::Paths(items) => assert_eq!(items[0].did, Did::Deleted),
            other => panic!("{other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    /// An outcome says which of the four it is, and carries what it came to.
    #[test]
    fn an_alignment_outcome_says_which_one_it_is() {
        let paths = Outcome::Paths(vec![Item {
            did: Did::Deleted,
            what: "a.txt".to_owned(),
        }]);
        let json = serde_json::to_value(OutcomeJson::from(&paths)).expect("the outcome writes");
        assert_eq!(json["kind"], "paths");
        assert_eq!(json["paths"][0]["did"], "deleted");
        assert_eq!(json["paths"][0]["what"], "a.txt");

        let refused = Outcome::Refused(Refusal::Move {
            path: "a.txt".to_owned(),
            kind: MoveKind::Tracked,
        });
        let json = serde_json::to_value(OutcomeJson::from(&refused)).expect("the outcome writes");
        assert_eq!(json["kind"], "refused");
        assert_eq!(json["refusal"]["kind"], "move");
        assert_eq!(json["refusal"]["move_kind"], "tracked");
    }
}

//! Bringing what a Vault holds and this Layout does not into the Layout being worked in.
//!
//! The command names references in the Vault's Layout and a place here; this module reads every
//! reference and works out every landing before anything is written, then puts each version's
//! content where it belongs. The command keeps the operands, the words and the progress.
//!
//! A reference is a path of the Vault's Layout, a glob over those paths, or a `Uuid`. Which of the
//! three it is, and where it lands, has to mean the same thing wherever a bring is asked for, the
//! desktop program included, so all of it is read here rather than by the command.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use globset::GlobBuilder;
use rorolala_errors::Failure as _;
use rorolala_layout::{EntryKind, Layout, LayoutPath, MutableData};
use rorolala_storage::{Key, Resolved, ShortHash, StorageSet};
use rorolala_tree::{
    EntryKind as TreeKind, Staging, Tree, TreeStructure, conflict_copy, merge, read_structure,
};
use rorolala_utils_lazyffi::{StrSpan, lazyffi};
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_utils_progress::Progress;
use rorolala_vcs::{VCSIndex, VCSIndexObject, Variant};
use rorolala_workspace::{Merging, Pending, Workspace};
use serde::Deserialize;
use uuid::Uuid;

use crate::daemon::{FetchLayoutRequest, action_fetch_layout, action_sync_index_all_async};
use crate::fetch::Sources;
use crate::write::checkout::Checkout;
use crate::write::layout::cached_copy;
use crate::write::progress::{WriteProgress, watching};
use crate::write::request::{Common, WriteCommon};
use crate::write::storage::extract_file;
use crate::write::sync::{chain_store_keys, store_of};
use crate::write::{Cancellation, WriteError};

/// The mark the Vault puts in front of a path that is not placed: `@/new/a.fbx` is an entry the
/// Vault holds no path for yet, and `@/removed/a.fbx` one that was taken out of the work.
///
/// It is the Vault's own arrangement, so nothing is written under it here: what comes in lands by
/// the name below the mark, and a run that kept the mark would make a directory of it that says
/// something about the Vault's bookkeeping rather than about the work.
const UNPLACED: &str = "@/";

/// Where the references are to land here, read from the operands the way `mv` reads its target.
#[derive(Debug, Clone)]
pub enum Destination {
    /// No destination was written: each reference lands at the path the Vault names it by.
    Named,
    /// One reference lands at exactly this path.
    Exact(LayoutPath),
    /// Every reference lands under this directory, joined with the name the Vault gives it; `""`
    /// names the Workspace root, the one directory with no name of its own.
    Under(String),
}

/// One reference read: the `Uuid`, what the Vault says about it, and where it lands here.
#[derive(Debug, Clone)]
pub struct Wanted {
    /// The `Uuid` it is known by.
    pub id: Uuid,
    /// What the Vault's Layout says about it.
    pub data: MutableData,
    /// The path this Layout is to name it by.
    pub path: LayoutPath,
}

/// What stopped a reference from being read.
#[derive(Debug)]
pub enum Refusal {
    /// The operands name nothing to bring in, or a destination several references cannot share.
    Argument,
    /// The Vault's Layout names nothing by the reference.
    Unknown(String),
    /// The Vault's Layout gives the `Uuid` no path, and a name is needed where it is to land.
    Unnamed(Uuid),
    /// Something is already at the path the reference was to land at.
    Taken(String),
    /// This Layout already holds the `Uuid` the reference names.
    Known(String),
}

/// The words a bring says a per-entry failure in.
///
/// The library does not know the caller's language, and a failure line whose words are English in a
/// Chinese program is a line that looks broken. The caller fills this from its own translations; the
/// library only chooses which word a failure is.
#[derive(Debug, Clone)]
pub struct Messages {
    /// A version whose content is not here and could not be brought.
    pub no_content: String,
    /// A tree whose mark could not be read back after it was written.
    pub tree_mark: String,
}

impl Default for Messages {
    fn default() -> Self {
        Self {
            no_content: "the version's content is not here".to_owned(),
            tree_mark: "the tree's mark could not be read".to_owned(),
        }
    }
}

/// One entry a bring put in.
#[derive(Debug, Clone)]
pub struct Item {
    /// The `Uuid` it is known by.
    pub uuid: String,
    /// The path the Vault's Layout names it by, when it names one.
    pub remote_path: Option<String>,
    /// The path this Layout now names it by.
    pub local_path: String,
}

/// What a bring came to.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The entries that came in.
    pub entries: Vec<Item>,
    /// What could not be brought in, one line each.
    pub failed: Vec<String>,
}

/// A bring asked for as one value, so a command line and a C caller fill the same request.
///
/// [`Common`] comes flattened, so the workspace, the account, the Vault, the offline flag, the
/// concurrency and the labels are the fields every write request carries. The references are read
/// the way `mv` reads its operands: `refs` is what is to come in, and `to` is where it lands, with
/// nothing there to land each reference at the path the Vault's Layout names it by.
///
/// A request that names `join` is the variant merge instead, read as a [`BringVariantRequest`]
/// beside it: the two are the same document, and which answer it gets is what `join` says.
#[derive(Debug, Deserialize)]
pub struct BringRequest {
    /// The fields every write request carries.
    #[serde(flatten)]
    common: Common,
    /// The references to bring in: paths of the Vault's Layout, globs over those paths, or `Uuid`s.
    ///
    /// A variant merge names none, since what it brings in is a variant of the index rather than an
    /// entry of the Vault's Layout, so this is empty or absent for it.
    #[serde(default)]
    refs: Vec<String>,
    /// Where the references land, or nothing to land each at the path the Vault names it by.
    #[serde(default)]
    to: Option<String>,
    /// Whether each reference keeps only its own name under a destination.
    #[serde(default)]
    by_name: bool,
    /// Whether this document asks for a variant merge rather than a bring of entries.
    ///
    /// Only its presence is read. The file the variant is joined into is the variant document's
    /// `file`, so a caller may put anything here; what the field says is which answer it wants.
    #[serde(default)]
    join: Option<String>,
}

impl BringRequest {
    /// Brings what the Vault holds and this Layout does not into it.
    ///
    /// The order is the one every bring takes, whichever caller asked for it: the Workspace, the
    /// Layout, the account and the Vault are resolved, the Vault's Layout is fetched unless the run
    /// is offline, its cached copy is opened, the index is brought up to date, and the content is
    /// asked of whichever Vault holds it. The references are read only after the copy is current,
    /// since what they name is what the Vault's Layout says now, and every landing is worked out
    /// before anything is written.
    ///
    /// `progress` is what the fetch says itself to; `cancellation` is checked before the run
    /// reaches a Vault and between two entries.
    ///
    /// A destination that is relative is read against the Workspace's root, never against the
    /// process's own directory: a caller outside a shell has no directory the run was made in, and
    /// the Workspace it named is the only place its paths can mean anything.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoWorkspace`] or [`WriteError::NoLayout`] when the request names none,
    /// [`WriteError::NoAccount`] when no account by the name given is here, the failures
    /// [`Common::vault`] names when the Vault cannot be resolved, [`WriteError::Action`] when the
    /// Vault's Layout or the index cannot be exchanged, [`WriteError::NotCached`] when the Vault's
    /// Layout was never fetched here, [`WriteError::NoIndex`] when no index is here,
    /// [`WriteError::Internal`] when the runtime cannot be made, and [`WriteError::Cancelled`] when
    /// `cancellation` was set before the run or between two of its entries. What the write itself
    /// fails with is whatever [`bring_in`] names, and a reference that is refused is a
    /// [`WriteError::NotFound`] or a [`WriteError::Precondition`].
    pub fn run(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Outcome, WriteError> {
        // A run asked to stop before it began reaches nothing: the fetch and the sync below are
        // exchanges with a Vault, and doing them for a run nobody waits for is work for nothing.
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        let workspace = self.common.workspace()?;
        let layout = self.common.layout(&workspace)?;
        let account = self.common.account(&workspace)?;
        let reached = self.common.vault(&workspace)?;

        // What the Vault holds now is what the references are read against, so its copy is brought
        // up to date first. Offline, the copy that is here is the one read, and a run with none is
        // told so below rather than by a fetch it may not make.
        if !self.common.offline {
            action_fetch_layout(
                &workspace,
                &account,
                reached.address.to_string(),
                &FetchLayoutRequest {
                    name: reached.name.clone(),
                },
            )
            .map_err(|error| WriteError::Action {
                reason: error.to_string(),
            })?;
        }

        let remote = cached_copy(&workspace, &reached.name)
            .map_err(|error| WriteError::Layout {
                reason: error.to_string(),
            })?
            .ok_or(WriteError::NotCached)?;

        let index = self.common.index(&workspace)?;

        // Only the index: it is light, and it is what tells a version from its content. The content
        // itself is asked for one object at a time, by what `bring_in` finds it needs.
        let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;

        if !self.common.offline {
            runtime
                .block_on(action_sync_index_all_async(
                    &workspace,
                    &account,
                    reached.address.to_string(),
                    progress.clone(),
                ))
                .map_err(|error| WriteError::Action {
                    reason: error.to_string(),
                })?;
        }

        let store = self.common.store(&workspace).ok();
        let sources = Sources::new(
            &workspace,
            &account,
            self.common.candidates(&workspace)?,
            self.common.offline,
        )
        .map_err(|reason| WriteError::Internal { reason })?
        .with_concurrency(self.common.concurrency)
        .reporting(progress.clone());

        let to = self
            .to
            .as_deref()
            .map(|to| rooted(workspace.get_root(), to));

        let wanted = resolve(
            &remote,
            &layout,
            workspace.get_root(),
            &self.refs,
            to.as_deref(),
            self.by_name,
        )
        .map_err(refused)?;

        bring_in(
            &workspace,
            &layout,
            &remote,
            &index,
            store.as_ref(),
            &runtime,
            wanted,
            &sources,
            &Messages::default(),
            cancellation,
        )
    }
}

/// A variant merge asked for as one value, so a command line and a C caller fill the same request.
///
/// A variant is content that already happened elsewhere, joined into a file this Layout names
/// without either side giving up. What comes in is not a path of the Vault's Layout but a variant
/// the index holds, so nothing here reads the Vault's Layout: the content goes beside the work as a
/// variant file, and what waits for what is written into the Workspace's merge records rather than
/// into the Layout.
///
/// The variant, its content and the index are asked of the Vault the Layout tracks only when they
/// are not here already. A merge of content that is here is local work, so a run that has all of it
/// needs no Vault, no account and no reachable one.
#[derive(Debug, Deserialize)]
pub struct BringVariantRequest {
    /// The fields every write request carries.
    #[serde(flatten)]
    common: Common,
    /// The variant to bring in, as a whole hash or as the head of one.
    variant: String,
    /// The file it is to be joined into, as the run names it.
    file: String,
    /// Where the variant file is to lie, or nothing for the place the merge convention gives it.
    #[serde(default)]
    to: Option<String>,
    /// Whether a tree variant is merged into its tree rather than put beside the work.
    #[serde(default)]
    tree_merge: bool,
}

/// A bring a C caller asks for.
///
/// Both brings are one entry point, and `join` is what tells them apart: an empty `join` asks for a
/// bring of entries, and a non-empty one asks for the variant merge, whose `variant`, `file` and
/// `tree_merge` are then read. A bring of entries reads `refs`, `to` and `by_name` instead.
///
/// # FFI
///
/// A `RolaWriteBringRequest` the caller fills. Every `const char *` is borrowed for the call and
/// released by the caller, so nothing here is freed when the call returns. `refs` is a
/// `RolaStrSpan` of the references to bring in, and an empty `to` lands each at the path the Vault's
/// Layout names it by.
#[lazyffi(export = RolaWriteBringRequest, input)]
#[derive(Debug, Clone, Default)]
pub struct BringInput {
    /// The fields every write request carries.
    pub common: WriteCommon,
    /// The references to bring in.
    pub refs: StrSpan,
    /// Where the references land, empty to land each at the path the Vault names it by.
    pub to: String,
    /// Whether each reference keeps only its own name under a destination.
    pub by_name: bool,
    /// The marker that asks for a variant merge, empty for a bring of entries.
    pub join: String,
    /// The variant to bring in, for a variant merge.
    pub variant: String,
    /// The file it is to be joined into, for a variant merge.
    pub file: String,
    /// Whether a tree variant is merged into its tree, for a variant merge.
    pub tree_merge: bool,
}

impl BringVariantRequest {
    /// The request a command builds from the resources it holds.
    ///
    /// A C caller fills [`BringInput`] instead, so this exists for the caller that already holds
    /// the Workspace, the account and the flags as values.
    #[must_use]
    pub const fn new(
        common: Common,
        variant: String,
        file: String,
        to: Option<String>,
        tree_merge: bool,
    ) -> Self {
        Self {
            common,
            variant,
            file,
            to,
            tree_merge,
        }
    }

    /// Brings a variant in to be joined into a file this Layout names.
    ///
    /// The order is the one a merge takes whichever caller asked for it: the Workspace, the Layout
    /// and the index are resolved, the variant word is read as a key, the file it is to be joined
    /// into is read from the Layout, and the merge records are read for what is already waiting.
    /// The variant is then read from the index, and what is not there is asked of the Vault the
    /// Layout tracks unless the run is offline. The content is taken out of the store, brought from
    /// whichever Vault holds it when there is one to ask, and written beside the work as a variant
    /// file, or merged into a tree. What is waiting for what goes into the merge records last, so a
    /// run that stops before the write leaves no merge waiting for a file nothing wrote.
    ///
    /// A tree variant merged into its tree is decided by structure: the version the variant names
    /// as its base is the shared ancestor, this tree's recorded structure is this side, and the
    /// variant's structure is the other. What both sides changed differently becomes a copy beside
    /// the work and a path the tree's staging names, which `tree add` settles.
    ///
    /// `progress` is what the exchanges say themselves to, and `cancellation` is checked before the
    /// run begins, after the Vault is asked, and before the work is written.
    ///
    /// A cancelled run is answered with [`WriteError::Cancelled`], which this module answers every
    /// cancelled write with. It is carried in [`VariantRefusal::Failed`] because a merge has one
    /// answer type: keeping the one error rather than adding a second way to say cancelled is what
    /// leaves a caller with one meaning to branch on. Nothing is rolled back, so what was written
    /// before the stop stays written and the caller reads what happened.
    ///
    /// The file a variant is to be joined into is read from where the run was made and then as a
    /// path of the Layout, the same reading `rola track` gives a name: a person writes one the way
    /// their shell does, and a caller outside a shell names it from the root.
    ///
    /// # Errors
    ///
    /// Returns [`VariantRefusal::NotAVariantHash`] when the variant word names no one variant,
    /// [`VariantRefusal::NotATarget`] when the file is no entry the Layout names,
    /// [`VariantRefusal::AlreadyMerging`] or [`VariantRefusal::AlreadyBroughtIn`] when the merge
    /// records say the run would undo or repeat one, [`VariantRefusal::UnknownVariant`] when the
    /// index holds no such variant and no Vault could be asked for it, [`VariantRefusal::OffChain`]
    /// when the two histories never meet and the request does not force the merge,
    /// [`VariantRefusal::NoStore`] when the Workspace keeps no store, [`VariantRefusal::NoContent`]
    /// when the content could not be brought here, [`VariantRefusal::PlaceOutside`] or
    /// [`VariantRefusal::PlaceTaken`] when the variant file cannot lie where it was to,
    /// [`VariantRefusal::Chain`] when the file's version chain cannot be read,
    /// [`VariantRefusal::NoTrack`], [`VariantRefusal::NoAccount`] or [`VariantRefusal::NoVault`]
    /// when the variant is not here and no Vault could be asked for it, and
    /// [`VariantRefusal::Failed`] for a Workspace, a Layout, an index or an exchange that failed in
    /// a way that is no refusal of the merge's own. A run that was cancelled is
    /// [`VariantRefusal::Failed`] around [`WriteError::Cancelled`].
    #[allow(
        clippy::too_many_lines,
        reason = "the merge reads a variant, its history, its content and its place in one order, and every refusal in that order is one the caller words apart"
    )]
    pub fn variant(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<VariantReport, VariantRefusal> {
        // A run asked to stop before it began reaches no Vault and writes nothing: the steps below
        // are the ones that ask and write, and doing them for a run nobody waits for is work for
        // nothing.
        if cancellation.is_cancelled() {
            return Err(VariantRefusal::Failed(WriteError::Cancelled));
        }

        // The Workspace, the Layout and the index are the only things a merge cannot do without:
        // the Vault, the account and the store are asked for below, where each is wanted, so a
        // merge of content that is here goes on in a Workspace that tracks no Vault.
        let workspace = self.common.workspace().map_err(VariantRefusal::Failed)?;
        let layout = self
            .common
            .layout(&workspace)
            .map_err(VariantRefusal::Failed)?;
        let index = self
            .common
            .index(&workspace)
            .map_err(VariantRefusal::Failed)?;
        let runtime = tokio::runtime::Runtime::new().map_err(|error| {
            VariantRefusal::Failed(WriteError::Internal {
                reason: error.to_string(),
            })
        })?;

        let variant_key = resolve_variant(&index, &runtime, &self.variant).map_err(|miss| {
            VariantRefusal::NotAVariantHash {
                hash: self.variant.clone(),
                miss,
            }
        })?;

        let root = workspace.get_root();
        let Some(id) = target_id(&layout, root, &self.file) else {
            return Err(VariantRefusal::NotATarget);
        };
        let Some(data) = layout.entry(id) else {
            return Err(VariantRefusal::NotATarget);
        };

        // One variant joins one file once: a second waiting on the same file would leave the first
        // with nowhere to go, and the same variant brought in twice would be two names for one
        // thing.
        let mut merging = Merging::read(root);
        if merging.at(id).is_some() {
            return Err(VariantRefusal::AlreadyMerging);
        }
        if merging.holds_variant(variant_key.digest()) {
            return Err(VariantRefusal::AlreadyBroughtIn);
        }

        let mut found = read_variant(&index, &runtime, &variant_key);

        // What is not here is asked of the Vault, unless the run was made offline: an offline run
        // reads what is here and says so where the read wants what is not.
        if found.is_none() && !self.common.offline {
            pull_index(&workspace, &self.common, &runtime, progress)?;
            found = read_variant(&index, &runtime, &variant_key);

            if cancellation.is_cancelled() {
                return Err(VariantRefusal::Failed(WriteError::Cancelled));
            }
        }

        let Some(variant_object) = found else {
            return Err(VariantRefusal::UnknownVariant);
        };

        // A variant is joined into a file whose history it shares: the two chains have to meet at
        // some version below the root, or they are histories that never met and the drawing has no
        // place to put it. `force` is how a run says it knows that and means to join it anyway.
        if !self.common.force
            && !shares_history(
                &index,
                &runtime,
                data.version(),
                variant_object.base_version(),
            )?
        {
            return Err(VariantRefusal::OffChain);
        }

        let Some(store) = self.common.store(&workspace).ok() else {
            return Err(VariantRefusal::NoStore);
        };
        let content = Key::new(*variant_object.storage_hash());

        // The content itself is brought the way any other bring carries it: asked of whichever
        // Vault holds it, when there is an account to ask as and a run that may reach one at all.
        let account = self.common.account(&workspace).ok();
        let sources = account.as_ref().and_then(|account| {
            Sources::new(
                &workspace,
                account,
                self.common.candidates(&workspace).ok()?,
                self.common.offline,
            )
            .ok()
            .map(|sources| {
                sources
                    .with_concurrency(self.common.concurrency)
                    .reporting(progress.clone())
            })
        });

        // A version is only a meaning once what it stored is here. A run that could have asked for
        // it and did not get it stops now, rather than handing over a name whose content is nowhere
        // and leaving the next run to discover it. A run with no Vault to ask never had a way to
        // bring it, which is a bring of what is here rather than a fetch that failed.
        if let Some(sources) = &sources {
            let missing = sources.bring(&store, &[content]);

            if sources.can_ask() && !missing.is_empty() {
                return Err(VariantRefusal::NoContent);
            }
        }

        if cancellation.is_cancelled() {
            return Err(VariantRefusal::Failed(WriteError::Cancelled));
        }

        // A tree variant's content is a structure rather than a file's bytes: what is put beside
        // the work is the whole directory the structure names, so the two sides can be compared as
        // trees.
        let tree_structure = runtime
            .block_on(store.read_object(&content))
            .ok()
            .and_then(|bytes| TreeStructure::decode(&bytes).ok());

        if self.tree_merge
            && data.kind() == EntryKind::Tree
            && let Some(structure) = &tree_structure
        {
            return Ok(VariantReport {
                target: self.file.clone(),
                variant: self.variant.clone(),
                path: merge_tree(
                    &workspace,
                    &layout,
                    &index,
                    &store,
                    &runtime,
                    id,
                    &data,
                    &variant_object,
                    structure,
                    sources.as_ref(),
                    cancellation,
                )?,
            });
        }

        // A place the run names is checked as it is given: one that climbs out of the Workspace is
        // refused rather than held at the root, since there is no target that has moved yet to
        // excuse it. A place that comes to climb out later, by the target moving shallower, is held
        // instead.
        if let Some(at) = self.to.as_deref()
            && let Some(target_path) = layout.path_of(id)
            && !Merging::place_holds(&target_path, Path::new(at))
        {
            return Err(VariantRefusal::PlaceOutside);
        }

        let pending = Pending::new(
            id,
            *variant_key.digest(),
            self.to.as_ref().map(PathBuf::from),
        );
        let Some(path) = merging.variant_path(&layout, &pending) else {
            return Err(VariantRefusal::PlaceOutside);
        };
        let disk = root.join(path.to_path_buf());

        // Nothing is written over: a file already at the place is something to be told about rather
        // than replaced, which is how `bring` reads a landing everywhere else. A tree may take an
        // empty directory, the same way a bring lands one.
        let taken = if tree_structure.is_some() {
            disk.exists()
                && !(disk.is_dir()
                    && std::fs::read_dir(&disk)
                        .map_or(true, |mut entries| entries.next().is_none()))
        } else {
            disk.exists()
        };
        if taken {
            return Err(VariantRefusal::PlaceTaken {
                path: path.as_str().to_owned(),
            });
        }

        if let Some(parent) = disk.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            return Err(VariantRefusal::Failed(WriteError::Io {
                path: parent.display().to_string(),
                reason: error.to_string(),
            }));
        }

        if let Some(structure) = &tree_structure {
            // What the structure names is brought in a second ask, since the structure is what says
            // what the inside is made of.
            if let Some(sources) = &sources {
                sources.bring(&store, &structure.content_keys());
            }

            if cancellation.is_cancelled() {
                return Err(VariantRefusal::Failed(WriteError::Cancelled));
            }

            if let Err(error) = write_worktree(&disk, structure, &store, &runtime) {
                return Err(VariantRefusal::Failed(WriteError::Tree { reason: error }));
            }
        } else if let Err(error) = runtime.block_on(extract_file(&store, &content, &disk)) {
            return Err(VariantRefusal::Failed(error));
        }

        merging.push(pending);
        if let Err(error) = merging.write(root) {
            return Err(VariantRefusal::Failed(WriteError::Io {
                path: Merging::path(root).display().to_string(),
                reason: error.to_string(),
            }));
        }

        Ok(VariantReport {
            target: self.file.clone(),
            variant: self.variant.clone(),
            path: path.as_str().to_owned(),
        })
    }
}

/// Why a word named no one variant the index can name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashMiss {
    /// It reads as neither a hash nor the head of one.
    Malformed,
    /// It reads as a head, and nothing the index holds starts with it.
    Unknown,
    /// It reads as a head, and more than one variant starts with it.
    Ambiguous(Vec<String>),
}

/// What a variant merge came to.
///
/// It is the one file the run wrote, named the way the caller named the three things it is about,
/// so a caller that words them its own way needs nothing else from the merge.
#[derive(Debug, Clone)]
pub struct VariantReport {
    /// The file the variant is waiting to be joined into, as the run named it.
    pub target: String,
    /// The variant that was brought in, as the run named it.
    pub variant: String,
    /// The path the variant's file was written to.
    pub path: String,
}

/// Why a variant was not brought in to be joined into a file.
///
/// The variants are the ones a caller words apart: each is a different thing to tell a person, and
/// each ends the run at a different step of the merge. The library does not know the caller's
/// language, so it says which thing went wrong and leaves the words to the caller, the same way
/// [`Refusal`] does for a bring of entries.
#[derive(Debug)]
pub enum VariantRefusal {
    /// What was named is no variant the index can name.
    NotAVariantHash {
        /// The word, as it was written.
        hash: String,
        /// Why it named no variant.
        miss: HashMiss,
    },
    /// The target is no file the Layout being worked in names.
    NotATarget,
    /// A variant is already waiting to be joined into the target.
    AlreadyMerging,
    /// That variant was already brought in.
    AlreadyBroughtIn,
    /// The index holds no such variant, and no Vault could be asked for it.
    UnknownVariant,
    /// The variant shares no version with the file's own chain.
    OffChain,
    /// This run has no store to write the content out of.
    NoStore,
    /// The content the variant's version needs could not be brought here.
    NoContent,
    /// The place the variant file was to lie at is nowhere the Workspace holds.
    PlaceOutside,
    /// Something is already at the place the variant file was to lie at.
    PlaceTaken {
        /// The place, as the Layout would name it.
        path: String,
    },
    /// The file's version chain could not be read.
    Chain,
    /// The Layout being worked in tracks no Vault to ask.
    NoTrack,
    /// The run acts as no account to ask a Vault as.
    NoAccount,
    /// The Vault the Layout tracks cannot be named.
    NoVault,
    /// The merge stopped: the run was cancelled, or a Workspace, a Layout, an index or an exchange
    /// failed in a way that is no refusal of the merge's own.
    Failed(WriteError),
}

impl From<VariantRefusal> for WriteError {
    fn from(refusal: VariantRefusal) -> Self {
        match refusal {
            VariantRefusal::Failed(error) => error,
            VariantRefusal::NotAVariantHash { hash, miss } => match miss {
                HashMiss::Malformed => Self::NotFound {
                    what: format!("`{hash}` as a variant hash"),
                },
                HashMiss::Unknown => Self::NotFound {
                    what: format!("the variant `{hash}`"),
                },
                HashMiss::Ambiguous(candidates) => Self::Precondition {
                    what: format!("the variant `{hash}`"),
                    reason: format!("it names {}", candidates.join(", ")),
                },
            },
            VariantRefusal::NotATarget => Self::Precondition {
                what: "the file the variant is joined into".to_owned(),
                reason: "the Layout being worked in names no entry at it".to_owned(),
            },
            VariantRefusal::AlreadyMerging => Self::Precondition {
                what: "the file the variant is joined into".to_owned(),
                reason: "a variant is already waiting to be joined into it".to_owned(),
            },
            VariantRefusal::AlreadyBroughtIn => Self::Precondition {
                what: "the variant".to_owned(),
                reason: "it was already brought in".to_owned(),
            },
            VariantRefusal::UnknownVariant => Self::NotFound {
                what: "the variant in the index".to_owned(),
            },
            VariantRefusal::OffChain => Self::Precondition {
                what: "the merge".to_owned(),
                reason: "the variant shares no version with the file's own chain".to_owned(),
            },
            VariantRefusal::NoStore => Self::NoStore,
            VariantRefusal::NoContent => Self::NotFound {
                what: "the content the variant's version names".to_owned(),
            },
            VariantRefusal::PlaceOutside => Self::Precondition {
                what: "the variant file's place".to_owned(),
                reason: "it is nowhere the Workspace holds".to_owned(),
            },
            VariantRefusal::PlaceTaken { path } => Self::Precondition {
                what: format!("the place `{path}`"),
                reason: "something is already there".to_owned(),
            },
            VariantRefusal::Chain => Self::Index {
                reason: "the file's version chain could not be read".to_owned(),
            },
            VariantRefusal::NoTrack => Self::NoTrack,
            VariantRefusal::NoAccount => Self::NoAccount,
            VariantRefusal::NoVault => Self::NoVault,
        }
    }
}

/// The failure a refused reference is reported as.
fn refused(refusal: Refusal) -> WriteError {
    match refusal {
        Refusal::Argument => WriteError::Precondition {
            what: "the references".to_owned(),
            reason: "they name no destination a run can use".to_owned(),
        },
        Refusal::Unknown(reference) => WriteError::NotFound {
            what: format!("`{reference}` in the Vault's Layout"),
        },
        Refusal::Unnamed(uuid) => WriteError::Precondition {
            what: format!("the landing of `{uuid}`"),
            reason: "the Vault's Layout gives it no path, and the request names no destination"
                .to_owned(),
        },
        Refusal::Taken(path) => WriteError::Precondition {
            what: format!("the landing `{path}`"),
            reason: "something is already there".to_owned(),
        },
        Refusal::Known(uuid) => WriteError::Precondition {
            what: format!("the entry `{uuid}`"),
            reason: "this Layout already holds it".to_owned(),
        },
    }
}

/// The key the variant word names, read as a whole hash or as the head of one.
///
/// A whole hash is taken as it is, whether or not the index holds it: what the index holds is what
/// the read below answers, and a word the index cannot name is refused there rather than here. Only
/// a head is resolved, and then among the variants the index holds, since a head names an object
/// only beside the ones that are there.
///
/// # Errors
///
/// Returns [`HashMiss::Malformed`] when the word reads as neither a hash nor the head of one,
/// [`HashMiss::Unknown`] when it reads as a head nothing the index holds starts with, and
/// [`HashMiss::Ambiguous`] when several do.
fn resolve_variant(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    word: &str,
) -> Result<Key, HashMiss> {
    if let Ok(key) = Key::from_str(word) {
        return Ok(key);
    }

    let Ok(head) = ShortHash::new(word) else {
        return Err(HashMiss::Malformed);
    };

    match head.resolve(variant_keys(index, runtime).iter()) {
        Resolved::ExactlyOne(key) => Ok(key),
        Resolved::NoMatch => Err(HashMiss::Unknown),
        Resolved::Ambiguous(keys) => Err(HashMiss::Ambiguous(keys.iter().map(Key::hex).collect())),
    }
}

/// The keys the index holds of the variant kind, which is what a head is resolved against.
///
/// An index that cannot be read whole answers with nothing: the run is refused where it reads the
/// variant it wanted, which is a failure about that variant rather than about this listing.
fn variant_keys(index: &VCSIndex, runtime: &tokio::runtime::Runtime) -> Vec<Key> {
    let Ok(objects) = runtime.block_on(index.read_objects()) else {
        return Vec::new();
    };

    objects
        .into_iter()
        .filter(|(_, object)| matches!(object, VCSIndexObject::Variant(_)))
        .map(|(key, _)| key)
        .collect()
}

/// The `Uuid` of the file `target` names, read from where the run was made and then as it stands.
///
/// It is the same reading `rola track` and `rola align` give a name, since a person writes one the
/// same way wherever it is written. A caller outside a shell has no directory of its own to write
/// from, and the Layout's own paths are the second reading, so the same word still names a file
/// there.
fn target_id(layout: &Layout, root: &Path, target: &str) -> Option<Uuid> {
    if let Ok(cwd) = std::env::current_dir() {
        let rooted = normalize(root);
        let named = if Path::new(target).is_absolute() {
            PathBuf::from(target)
        } else {
            cwd.join(target)
        };
        let absolute = normalize(&named);

        if let Ok(relative) = absolute.strip_prefix(&rooted)
            && let Ok(path) = LayoutPath::from_relative(relative)
            && let Some(id) = layout.id_of(&path)
        {
            return Some(id);
        }
    }

    LayoutPath::new(target)
        .ok()
        .and_then(|path| layout.id_of(&path))
}

/// The variant the index holds under `key`, when it holds one.
///
/// A read that fails and one that holds another kind of object are the same answer here: what the
/// run wanted is not here, and whether the index refused or held something else is the index's
/// business rather than the merge's.
fn read_variant(index: &VCSIndex, runtime: &tokio::runtime::Runtime, key: &Key) -> Option<Variant> {
    match runtime.block_on(index.read(*key)) {
        Ok(VCSIndexObject::Variant(variant)) => Some(variant),
        _ => None,
    }
}

/// Brings the index over from the Vault the Layout tracks, so what the run names can be read here.
///
/// Only the index: it is light, and it is what tells a variant from its content. The Vault is asked
/// for it only when the variant itself was not here, so a merge of content already synced never
/// reaches one.
///
/// What cannot be done is a refusal rather than a cause: each names the one thing the run lacked,
/// and a reader who is told the Layout tracks no Vault has a different thing to do about it than
/// one who is told it tracks one that cannot be named.
///
/// # Errors
///
/// Returns [`VariantRefusal::NoTrack`] when the Layout tracks no Vault,
/// [`VariantRefusal::NoAccount`] when the run acts as no account, [`VariantRefusal::NoVault`] when
/// the Vault it tracks cannot be resolved, and [`VariantRefusal::Failed`] when the exchange cannot
/// be made.
fn pull_index(
    workspace: &Workspace,
    common: &Common,
    runtime: &tokio::runtime::Runtime,
    progress: &Progress,
) -> Result<(), VariantRefusal> {
    let reached = common.vault(workspace).map_err(|error| match error {
        WriteError::NoTrack => VariantRefusal::NoTrack,
        _ => VariantRefusal::NoVault,
    })?;
    let account = common
        .account(workspace)
        .map_err(|_| VariantRefusal::NoAccount)?;

    runtime
        .block_on(action_sync_index_all_async(
            workspace,
            &account,
            reached.address.to_string(),
            progress.clone(),
        ))
        .map_err(|error| {
            VariantRefusal::Failed(WriteError::Action {
                reason: error.to_string(),
            })
        })?;

    Ok(())
}

/// Whether two histories meet at a version below the root.
///
/// The file's own chain is read from the version it records back to the root; the variant's chain
/// is read from the version it is based on in the same way. Meeting anywhere on the two lines is a
/// shared history: two chains that forked from one version and advanced separately are still one
/// history, and the merge belongs in it. A variant based on the root is a new history's first
/// variant, which joins as it always has; the root alone is not a shared base between two deeper
/// chains, since every file's first variant is based on it.
///
/// A chain that cannot be read to a version the index holds proves nothing, and is answered as no
/// shared history rather than as a failure of the index: what a run can act on is whether the merge
/// has a place, not why one could not be found.
///
/// # Errors
///
/// Returns [`VariantRefusal::Chain`] when the file's own chain holds an object that is not a version
/// or a variant, and [`VariantRefusal::Failed`] when the index cannot be read at all.
fn shares_history(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    head: [u8; 32],
    base: &[u8; 32],
) -> Result<bool, VariantRefusal> {
    let mut chain = BTreeSet::new();
    let mut current = head;
    let mut seen = BTreeSet::new();

    while seen.insert(current) {
        let version = match runtime.block_on(index.read(Key::new(current))) {
            Ok(VCSIndexObject::Version(version)) => version,
            Ok(_) => return Err(VariantRefusal::Chain),
            Err(error) => {
                return Err(VariantRefusal::Failed(WriteError::Index {
                    reason: error.reason(),
                }));
            }
        };

        if version.is_root() {
            break;
        }

        chain.insert(current);

        let variant = match runtime.block_on(index.read(Key::new(*version.variant()))) {
            Ok(VCSIndexObject::Variant(variant)) => variant,
            Ok(_) => return Err(VariantRefusal::Chain),
            Err(error) => {
                return Err(VariantRefusal::Failed(WriteError::Index {
                    reason: error.reason(),
                }));
            }
        };

        current = *variant.base_version();
    }

    match runtime.block_on(index.read(Key::new(*base))) {
        Ok(VCSIndexObject::Version(version)) if version.is_root() => return Ok(true),
        Ok(VCSIndexObject::Version(_)) => {}
        Ok(_) | Err(_) => return Ok(false),
    }

    let mut current = *base;
    let mut seen = BTreeSet::new();

    while seen.insert(current) {
        if chain.contains(&current) {
            return Ok(true);
        }

        let Ok(VCSIndexObject::Version(version)) = runtime.block_on(index.read(Key::new(current)))
        else {
            return Ok(false);
        };

        if version.is_root() {
            return Ok(false);
        }

        let Ok(VCSIndexObject::Variant(variant)) =
            runtime.block_on(index.read(Key::new(*version.variant())))
        else {
            return Ok(false);
        };

        current = *variant.base_version();
    }

    Ok(false)
}

/// Merges a tree variant into the tree it is a variant of, and answers the tree's path.
///
/// The version the variant names as its base is the shared ancestor, this tree's recorded structure
/// is this side, and the variant's structure is the other. What both sides changed differently
/// becomes this side's entry in the merged structure and a copy beside the work, named the way a
/// conflict is named, so a person can compare the two; the tree's staging records each such path,
/// which is what `tree add` settles.
///
/// # Errors
///
/// Returns [`VariantRefusal::NotATarget`] when the Layout names no path for the tree,
/// [`VariantRefusal::Failed`] when a structure cannot be read or written, and
/// [`VariantRefusal::Failed`] around [`WriteError::Cancelled`] when the run was cancelled before
/// the work is written.
#[allow(
    clippy::too_many_arguments,
    reason = "the merge reads and writes through the Workspace, both Layouts, the index, the store and the fetching at once, and its steps share every one of them"
)]
fn merge_tree(
    workspace: &Workspace,
    layout: &Layout,
    index: &VCSIndex,
    store: &StorageSet,
    runtime: &tokio::runtime::Runtime,
    id: Uuid,
    data: &MutableData,
    variant: &Variant,
    structure: &TreeStructure,
    sources: Option<&Sources<'_>>,
    cancellation: &Cancellation,
) -> Result<String, VariantRefusal> {
    let Some(tree_path) = layout.path_of(id) else {
        return Err(VariantRefusal::NotATarget);
    };
    let tree_dir = workspace.get_root().join(tree_path.to_path_buf());

    let ours = runtime
        .block_on(read_structure(index, store, &data.version()))
        .map_err(|error| {
            VariantRefusal::Failed(WriteError::Tree {
                reason: error.to_string(),
            })
        })?;
    let base = runtime
        .block_on(read_structure(index, store, variant.base_version()))
        .map_err(|error| {
            VariantRefusal::Failed(WriteError::Tree {
                reason: error.to_string(),
            })
        })?;

    let merged = merge(&base, &ours, structure);

    // What the variant's structure names is brought in a second ask, since the structure is what
    // says what the inside is made of.
    if let Some(sources) = sources {
        sources.bring(store, &structure.content_keys());
    }

    if cancellation.is_cancelled() {
        return Err(VariantRefusal::Failed(WriteError::Cancelled));
    }

    if let Err(error) = write_worktree(&tree_dir, &merged.structure, store, runtime) {
        return Err(VariantRefusal::Failed(WriteError::Tree { reason: error }));
    }

    for path in &merged.conflicts {
        let Some(entry) = structure.entry(path) else {
            continue;
        };
        let theirs = tree_dir.join(conflict_copy(path));

        if entry.kind() == TreeKind::Directory {
            let inside = structure.subtree(path);
            let _ = write_worktree(&theirs, &inside, store, runtime);
        } else if let Some(storage) = entry.storage() {
            let _ = runtime.block_on(extract_file(store, storage, &theirs));
        }
    }

    let staging = Staging::new(&tree_dir);
    if let Err(error) = staging.write_conflicts(&merged.conflicts) {
        return Err(VariantRefusal::Failed(WriteError::Tree {
            reason: error.to_string(),
        }));
    }

    Ok(tree_path.as_str().to_owned())
}

/// Brings what a Vault holds and a Layout does not into it, or a variant the index holds in to be
/// joined into a file the Layout names, from a request a C caller fills.
///
/// # FFI
///
/// `request` is a `RolaWriteBringRequest` the caller fills and keeps; every `const char *` in it is
/// borrowed for the call and released by the caller. A `RolaWriteBring *` the caller releases with
/// `free_rola_write_bring` answers both forms: `_joined` says which one came back. Nothing else in
/// the answer means anything for the form that did not come back. A failure is a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `progress` is the handle the
/// caller polls with `rola_progress_next`, and `cancellation` is the flag it sets with
/// `rola_cancellation_cancel`.
///
/// An empty `join` brings entries in. `_entry_count` and `_entry_at` name each one, with
/// `remote_path` empty when the Vault's Layout names no path for the entry, and `_failed_count` and
/// `_failed_at` hold one line per entry that could not come in. A non-empty `join` brings a variant
/// in, and `_joined` is true with `_target`, `_variant` and `_path` naming the file it waits to be
/// joined into, the variant that came in and the path its file was written to.
///
/// # Errors
///
/// Returns the failures [`BringRequest::run`] or [`BringVariantRequest::variant`] name, as `join`
/// asks for one or the other.
#[lazyffi(export = rola_write_bring)]
pub fn bring_at(
    request: &BringInput,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<WriteBring, WriteError> {
    let watched = watching(progress);
    let asked = BringRequest {
        common: request.common.clone().into(),
        refs: request.refs.clone().into_vec(),
        to: (!request.to.is_empty()).then(|| request.to.clone()),
        by_name: request.by_name,
        join: (!request.join.is_empty()).then(|| request.join.clone()),
    };

    // The two brings answer different things, so the answer is read from the request rather than
    // guessed at: `join` is the one field a variant merge has and a bring of entries does not.
    if asked.join.is_some() {
        let variant = BringVariantRequest::new(
            asked.common,
            request.variant.clone(),
            request.file.clone(),
            (!request.to.is_empty()).then(|| request.to.clone()),
            request.tree_merge,
        );
        let report = variant.variant(&watched, cancellation)?;

        return Ok(WriteBring::of_variant(&report));
    }

    let outcome = asked.run(&watched, cancellation)?;

    Ok(WriteBring::of_entries(&outcome))
}

/// What a bring came to, as the C ABI hands it over.
///
/// One export answers two things, a bring of entries and a variant merge, and `joined` says which
/// of them came back: a variant merge answers the target, the variant and the path, and a bring of
/// entries answers the entries and the failures. The fields of the form that did not come back say
/// nothing, so a caller reads the ones `joined` names.
///
/// # FFI
///
/// A `RolaWriteBring *` the caller releases with `free_rola_write_bring`. `_joined` says which form
/// it is. `_entry_count` and `_entry_at` answer the entries, each a `RolaWriteBringEntry *`
/// released with `free_rola_write_bring_entry`; `_failed_count` and `_failed_at` answer the failure
/// lines; and `_target`, `_variant` and `_path` answer a variant merge. Every `char *` is released
/// with `free_string`.
#[lazyffi(export = RolaWriteBring)]
#[derive(Debug, Clone, Default)]
pub struct WriteBring {
    /// Whether the answer is a variant merge rather than a bring of entries.
    joined: bool,
    /// The entries that came in, for a bring of entries.
    entries: Vec<WriteBringEntry>,
    /// What could not be brought in, one line each, for a bring of entries.
    failed: Vec<String>,
    /// The file the variant waits to be joined into, for a variant merge.
    target: String,
    /// The variant that came in, for a variant merge.
    variant: String,
    /// The path the variant's file was written to, for a variant merge.
    path: String,
}

impl WriteBring {
    /// The answer a bring of entries is handed over as.
    fn of_entries(outcome: &Outcome) -> Self {
        Self {
            entries: outcome.entries.iter().map(WriteBringEntry::of).collect(),
            failed: outcome.failed.clone(),
            ..Self::default()
        }
    }

    /// The answer a variant merge is handed over as.
    fn of_variant(report: &VariantReport) -> Self {
        Self {
            joined: true,
            target: report.target.clone(),
            variant: report.variant.clone(),
            path: report.path.clone(),
            ..Self::default()
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteBring {
    /// Whether the answer is a variant merge rather than a bring of entries.
    #[must_use]
    pub const fn joined(&self) -> bool {
        self.joined
    }

    /// How many entries came in, which is zero for a variant merge.
    #[must_use]
    pub const fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// One entry that came in, or null past the end.
    #[must_use]
    pub fn entry_at(&self, index: usize) -> Option<WriteBringEntry> {
        self.entries.get(index).cloned()
    }

    /// How many entries could not come in, which is zero for a variant merge.
    #[must_use]
    pub const fn failed_count(&self) -> usize {
        self.failed.len()
    }

    /// One entry that could not come in, or an empty string past the end.
    #[must_use]
    pub fn failed_at(&self, index: usize) -> String {
        self.failed.get(index).cloned().unwrap_or_default()
    }

    /// The file the variant waits to be joined into, or an empty string for a bring of entries.
    #[must_use]
    pub fn target(&self) -> String {
        self.target.clone()
    }

    /// The variant that came in, or an empty string for a bring of entries.
    #[must_use]
    pub fn variant(&self) -> String {
        self.variant.clone()
    }

    /// The path the variant's file was written to, or an empty string for a bring of entries.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }
}

/// One entry a bring put in.
///
/// # FFI
///
/// A `RolaWriteBringEntry *` the caller releases with `free_rola_write_bring_entry`. `_uuid`,
/// `_remote_path` and `_local_path` each return a `char *` the caller releases with `free_string`;
/// `_remote_path` is empty when the Vault's Layout names no path for the entry.
#[lazyffi(export = RolaWriteBringEntry)]
#[derive(Debug, Clone, Default)]
pub struct WriteBringEntry {
    /// The `Uuid` it is known by.
    uuid: String,
    /// The path the Vault's Layout names it by, empty when it names none.
    remote_path: String,
    /// The path this Layout now names it by.
    local_path: String,
}

impl WriteBringEntry {
    /// The answer one brought entry is handed over as.
    fn of(item: &Item) -> Self {
        Self {
            uuid: item.uuid.clone(),
            remote_path: item.remote_path.clone().unwrap_or_default(),
            local_path: item.local_path.clone(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteBringEntry {
    /// The `Uuid` it is known by.
    #[must_use]
    pub fn uuid(&self) -> String {
        self.uuid.clone()
    }

    /// The path the Vault's Layout names it by, or an empty string when it names none.
    #[must_use]
    pub fn remote_path(&self) -> String {
        self.remote_path.clone()
    }

    /// The path this Layout now names it by.
    #[must_use]
    pub fn local_path(&self) -> String {
        self.local_path.clone()
    }
}

/// Reads every reference before anything is written, so one that names nothing leaves nothing half
/// done.
///
/// Every reference is read against `remote`, the Vault's Layout, and every landing is worked out
/// here, which is why a run that is refused has written nothing when it is refused.
///
/// # Errors
///
/// Returns [`Refusal::Argument`] when the operands cannot be read as a destination, when a
/// destination that is not a directory is given more than one reference, or when a reference reads
/// as a pattern but not as a usable one. Returns [`Refusal::Unknown`] when a reference names nothing
/// in `remote`, [`Refusal::Unnamed`] when the landing path cannot be worked out, [`Refusal::Taken`]
/// when the landing is already something here or another reference of the same run lands there, and
/// [`Refusal::Known`] when this Layout already holds the `Uuid`.
pub fn resolve(
    remote: &Layout,
    layout: &Layout,
    root: &Path,
    refs: &[String],
    to: Option<&str>,
    by_name: bool,
) -> Result<Vec<Wanted>, Refusal> {
    let destination = destination(root, to)?;

    // A destination that is not a directory names one landing. More than one operand is already
    // more than it can take, the way `mv` reads them; one operand naming several, a glob, is
    // found out once the references are read below.
    if matches!(destination, Destination::Exact(_)) && refs.len() > 1 {
        return Err(Refusal::Argument);
    }

    let matched = expand(remote, refs, &destination, by_name)?;

    // A destination that is not a directory names one landing, so it can take only one entry.
    if matches!(destination, Destination::Exact(_)) && matched.len() > 1 {
        return Err(Refusal::Argument);
    }

    let mut wanted = Vec::with_capacity(matched.len());
    let mut landings = BTreeSet::new();

    for Wanted { id, data, path } in matched {
        // What is to come in is something this Layout does not hold.
        if layout.entry(id).is_some() {
            return Err(Refusal::Known(id.to_string()));
        }

        // Where it lands is somewhere nothing is: not a name this Layout gives, not a file already
        // there, and not where an earlier reference of this run is to land.
        if !landings.insert(path.as_str().to_owned())
            || layout.id_of(&path).is_some()
            || landing_taken(root, &path, &data)
        {
            return Err(Refusal::Taken(path.as_str().to_owned()));
        }

        wanted.push(Wanted { id, data, path });
    }

    Ok(wanted)
}

/// Everything the references name, each entry once and in the order the references name them, with
/// where each of them lands.
///
/// A reference that names a folder of the copy brings what stands under it as well: bringing a
/// folder in is bringing in the tree it is, and a path that names only the directory, no entry
/// stands there, names what is under it rather than nothing. An entry several references name, a
/// glob and a path that reaches into what it matched, is one request rather than a clash, so it is
/// brought in once.
///
/// # Errors
///
/// Returns whatever [`seeds`] and [`landing`] refuse, one reference at a time.
fn expand(
    remote: &Layout,
    refs: &[String],
    destination: &Destination,
    by_name: bool,
) -> Result<Vec<Wanted>, Refusal> {
    let mut paths: Vec<LayoutPath> = remote.paths().into_iter().map(|(path, _)| path).collect();
    paths.sort_unstable();

    let mut wanted = Vec::new();
    let mut seen = BTreeSet::new();

    for reference in refs {
        let (base, seeds) = seeds(remote, &paths, reference, by_name)?;

        for seed in seeds {
            let mut candidates = vec![seed.clone()];
            candidates.extend(
                paths
                    .iter()
                    .filter(|path| under(path.as_str(), seed.as_str()))
                    .cloned(),
            );

            for path in candidates {
                let Some(id) = remote.id_of(&path) else {
                    continue;
                };

                if !seen.insert(id) {
                    continue;
                }

                let Some(data) = remote.entry(id) else {
                    continue;
                };

                let Some(landing) = landing(destination, &path, &base) else {
                    return Err(Refusal::Unnamed(id));
                };

                wanted.push(Wanted {
                    id,
                    data,
                    path: landing,
                });
            }
        }
    }

    Ok(wanted)
}

/// What one reference names: the directory a glob selects from, and the paths it names.
///
/// It is what the entries are walked from, so a path that names only a folder, no entry stands
/// there, names what is under it rather than nothing, and a glob that matches a folder brings the
/// folder's tree in with it.
///
/// # Errors
///
/// Returns [`Refusal::Argument`] when a reference reads as a pattern but not as a usable one, and
/// [`Refusal::Unknown`] when it names nothing, a glob that matches nothing included, since a run
/// that quietly did part of what was asked is worse than one that stops.
fn seeds(
    remote: &Layout,
    paths: &[LayoutPath],
    reference: &str,
    by_name: bool,
) -> Result<(String, Vec<LayoutPath>), Refusal> {
    if let Ok(id) = Uuid::from_str(reference) {
        return remote.path_of(id).map_or_else(
            || Err(Refusal::Unknown(reference.to_owned())),
            |path| {
                let base = if by_name {
                    parent(path.as_str())
                } else {
                    String::new()
                };

                Ok((base, vec![path]))
            },
        );
    }

    if is_pattern(reference) {
        let matched = matching(reference, paths)?;

        if matched.is_empty() {
            return Err(Refusal::Unknown(reference.to_owned()));
        }

        return Ok((pattern_base(reference), matched));
    }

    let Ok(path) = LayoutPath::new(reference) else {
        return Err(Refusal::Unknown(reference.to_owned()));
    };

    if remote.id_of(&path).is_none()
        && !paths
            .iter()
            .any(|candidate| under(candidate.as_str(), path.as_str()))
    {
        return Err(Refusal::Unknown(reference.to_owned()));
    }

    let base = if by_name {
        parent(path.as_str())
    } else {
        String::new()
    };

    Ok((base, vec![path]))
}

/// The directory a path stands in, which is what a name is read against.
///
/// It is the whole path before its last separator, and nothing when it has none: an entry at the
/// top of the Workspace stands in the root, which is the directory with no name of its own.
fn parent(path: &str) -> String {
    path.rsplit_once('/')
        .map_or_else(String::new, |(directory, _)| directory.to_owned())
}

/// The directory a glob selects from, which what it matches is rebased on.
///
/// It is what stands before the first pattern character, cut back to the directory that holds it:
/// `Assets/*` and `Assets/**` both select from `Assets`, and a pattern with no separator at all
/// selects from the root.
fn pattern_base(reference: &str) -> String {
    let cut = reference
        .find(['*', '?', '[', '{'])
        .unwrap_or(reference.len());
    let head = &reference[..cut];

    head.rfind('/')
        .map_or_else(String::new, |at| head[..at].to_owned())
}

/// Whether `path` names something below `of`, and not `of` itself.
fn under(path: &str, of: &str) -> bool {
    path.len() > of.len() && path.starts_with(of) && path.as_bytes()[of.len()] == b'/'
}

/// Whether a reference is to be read as a glob over the Vault's Layout paths.
///
/// A reference with none of the pattern characters names one path exactly, so a path holding a `[`
/// or `{` is still nameable; one with them is a pattern, and a literal occurrence has to be written
/// as a class the way [`globset`] spells one.
fn is_pattern(reference: &str) -> bool {
    reference
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | '{'))
}

/// The paths among `paths` that the glob `reference` names, keeping the order they came in.
///
/// Separators are literal, the way a shell reads a pattern: `*` names what is directly under a
/// directory and `**` what is anywhere below it, rather than `*` swallowing a whole tree.
///
/// # Errors
///
/// Returns [`Refusal::Argument`] when the reference does not read as a pattern.
fn matching(reference: &str, paths: &[LayoutPath]) -> Result<Vec<LayoutPath>, Refusal> {
    let glob = GlobBuilder::new(reference)
        .literal_separator(true)
        .build()
        .map_err(|_| Refusal::Argument)?;
    let matcher = glob.compile_matcher();

    Ok(paths
        .iter()
        .filter(|path| matcher.is_match(path.as_str()))
        .cloned()
        .collect())
}

/// Where the operands say the references are to land.
///
/// A destination that is absolute is read where it says. A relative one is read where the run was
/// made, the way `rola track` and `rola align` read a name: `.` is the directory the run is in and
/// `..` climbs from there. Either way it has to be somewhere the Workspace holds, since a Layout
/// names nothing outside its own root.
///
/// # Errors
///
/// Returns [`Refusal::Argument`] when a relative destination is given and the directory the run was
/// made in cannot be read, when the destination is outside the Workspace, or when it is not a path a
/// Layout may name.
fn destination(root: &Path, to: Option<&str>) -> Result<Destination, Refusal> {
    let Some(to) = to else {
        return Ok(Destination::Named);
    };

    // A destination that already says where it is does not need the directory the run was made in:
    // reading it anyway would make the landing depend on where the process happens to be.
    let absolute = if Path::new(to).is_absolute() {
        normalize(Path::new(to))
    } else {
        let Ok(cwd) = std::env::current_dir() else {
            return Err(Refusal::Argument);
        };

        normalize(&resolve_against(&cwd, to))
    };

    let Ok(relative) = absolute.strip_prefix(root) else {
        return Err(Refusal::Argument);
    };

    if absolute.is_dir() {
        return Ok(Destination::Under(prefix(relative)));
    }

    LayoutPath::from_relative(relative)
        .map(Destination::Exact)
        .map_err(|_| Refusal::Argument)
}

/// Whether where an entry lands is already taken.
///
/// A tree may land on a directory that is there and empty: a folder made for it, with nothing in it,
/// is the same as nothing being there for a thing that is a directory. A file is only ever a landing
/// where nothing is.
fn landing_taken(root: &Path, path: &LayoutPath, data: &MutableData) -> bool {
    let disk = root.join(path.to_path_buf());
    if !disk.exists() {
        return false;
    }
    if data.kind() != EntryKind::Tree || !disk.is_dir() {
        return true;
    }

    std::fs::read_dir(&disk).map_or(true, |mut entries| entries.next().is_some())
}

/// The path one reference lands at: the path the Vault gives it, joined into the destination when
/// that is a directory, or the destination itself when that is the whole name.
///
/// A path keeps all of itself under a directory, and a glob match keeps what it has below the
/// directory the glob selects from: that is what makes bringing a folder in bring the folder, the
/// tree it is, remade where it is put, while `Assets/*` brings what is in `Assets` rather than
/// `Assets` again.
///
/// What a path names under the Vault's unplaced mark, `@/new/a.fbx`, lands without that mark.
/// The `@` is the Vault's way of saying the entry has no path yet, not a directory anything here
/// has: a Layout here that named `@/new/a.fbx` would make an `@` directory that names a decision
/// the Vault has not made, and every reader of it would take it for a folder of the work.
fn landing(destination: &Destination, named: &LayoutPath, base: &str) -> Option<LayoutPath> {
    match destination {
        Destination::Named => {
            placeable(named.as_str()).and_then(|placed| LayoutPath::new(placed).ok())
        }
        Destination::Exact(path) => Some(path.clone()),
        Destination::Under(directory) => {
            let rest = if base.is_empty() {
                named.as_str()
            } else {
                named
                    .as_str()
                    .strip_prefix(base)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .unwrap_or(named.as_str())
            };
            let rest = placeable(rest)?;
            let joined = if directory.is_empty() {
                rest.to_owned()
            } else {
                format!("{directory}/{rest}")
            };

            // Both halves are names a `LayoutPath` already accepted, so joining them cannot name
            // something one would not.
            LayoutPath::new(&joined).ok()
        }
    }
}

/// A path as it is to be written here, with the Vault's unplaced mark taken off the front.
///
/// `None` is a path that is nothing but the mark: there is no name in it to place.
fn placeable(path: &str) -> Option<&str> {
    match path.strip_prefix(UNPLACED) {
        Some("") => None,
        Some(rest) => Some(rest),
        None => Some(path),
    }
}

/// Where a destination written in a request is read from: the Workspace's root.
///
/// A C caller runs inside a program whose working directory says nothing about the Workspace, so a
/// relative destination names a path under the root the request named rather than under the process
/// directory. A destination that is already absolute is left as it is.
fn rooted(root: &Path, to: &str) -> String {
    if Path::new(to).is_absolute() {
        to.to_owned()
    } else {
        root.join(to).display().to_string()
    }
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve_against(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// The prefix the paths under `relative` share, or `""` for the Workspace root itself.
fn prefix(relative: &Path) -> String {
    LayoutPath::from_relative(relative)
        .map_or_else(|_| String::new(), |path| path.as_str().to_owned())
}

/// Puts each version's content at its path, and makes the Layout name it.
///
/// What could not be done is named rather than stopping the run: a bring is not one write but
/// several, and what went in is worth reporting beside what did not.
///
/// `messages` is how a per-entry failure is said, since the caller's language is not this crate's.
///
/// # Errors
///
/// Returns [`WriteError::Cancelled`] when `cancellation` was set before the run began or between
/// two entries. What was written before the stop stays written; the caller is told where the run
/// stopped rather than shown a state it never had.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the run needs the workspace, both Layouts, the index, the store, the fetching, the words and the watching, and its steps share every one of them"
)]
pub fn bring_in(
    workspace: &Workspace,
    layout: &Layout,
    remote: &Layout,
    index: &VCSIndex,
    store: Option<&StorageSet>,
    runtime: &tokio::runtime::Runtime,
    wanted: Vec<Wanted>,
    sources: &Sources<'_>,
    messages: &Messages,
    cancellation: &Cancellation,
) -> Result<Outcome, WriteError> {
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled);
    }

    let mut entries = Vec::new();
    let mut failed = Vec::new();

    // The readings are gathered in one cache and the Layout changes in one batch, so a bring of
    // many files reads what others wrote once, writes the Layout once per shard, and writes the
    // cache once, rather than doing each of those for every file.
    let mut checkout = Checkout::open(layout, workspace.get_root());
    let mut batch = layout.batch();

    // Every version's content is named before anything is written, so what is missing is asked of
    // the Vaults in one go rather than one file at a time.
    let mut resolved = Vec::new();
    for wanted in wanted {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        match runtime.block_on(store_of(index, &wanted.data.version())) {
            Ok(stored) => resolved.push((wanted, stored)),
            Err(cause) => failed.push(format!("{}: {cause}", wanted.id)),
        }
    }

    if let Some(store) = store {
        let mut keys: Vec<Key> = Vec::new();

        for (wanted, stored) in &resolved {
            if cancellation.is_cancelled() {
                return Err(WriteError::Cancelled);
            }

            keys.extend(*stored);

            // A tree's version names a structure, and a structure is one version's inside. An older
            // version is a structure too, and a later retrack reads it before it asks for what it
            // names: bringing the chain here is what leaves the pointer able to move back.
            if wanted.data.kind() == EntryKind::Tree
                && let Ok(chain) = runtime.block_on(chain_store_keys(
                    index,
                    &wanted.data.version(),
                    Some(store),
                    false,
                ))
            {
                keys.extend(chain);
            }
        }

        keys.sort_unstable();
        keys.dedup();

        sources.bring(store, &keys);
    }

    for (wanted, stored) in resolved {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        let Wanted { id, data, path } = wanted;

        let (Some(stored), Some(store)) = (stored, store) else {
            failed.push(format!("{id}: {}", messages.no_content));

            continue;
        };

        let disk = workspace.get_root().join(path.to_path_buf());
        if let Some(parent) = disk.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            failed.push(format!("{id}: {error}"));

            continue;
        }

        // A tree lands as a directory rather than a file: its structure says what the work is, its
        // file content is brought in a second ask, and its baseline records the version the Vault
        // holds so the tree is clean as soon as it is here.
        if data.kind() == EntryKind::Tree {
            let bytes = match runtime.block_on(store.read_object(&stored)) {
                Ok(bytes) => bytes,
                Err(error) => {
                    failed.push(format!("{id}: {error}"));

                    continue;
                }
            };
            let structure = match TreeStructure::decode(&bytes) {
                Ok(structure) => structure,
                Err(error) => {
                    failed.push(format!("{id}: {error}"));

                    continue;
                }
            };

            sources.bring(store, &structure.content_keys());

            if let Err(error) = write_worktree(&disk, &structure, store, runtime) {
                failed.push(format!("{id}: {error}"));

                continue;
            }

            let staging = Staging::new(&disk);
            if let Err(error) = staging.ensure_store() {
                failed.push(format!("{id}: {error}"));

                continue;
            }
            let Ok(tree) = Tree::open(workspace.get_root(), &disk) else {
                failed.push(format!("{id}: {}", messages.tree_mark));

                continue;
            };
            if let Err(error) = tree.baseline().write(&structure, &data.version()) {
                failed.push(format!("{id}: {error}"));

                continue;
            }
            // The id is what a later reading knows the tree by, wherever the directory is moved to.
            if let Err(error) = tree.write_local_id(id) {
                failed.push(format!("{id}: {error}"));

                continue;
            }
            if let Err(error) = batch.create_file(id, data, &path) {
                failed.push(format!("{id}: {error}"));

                continue;
            }

            entries.push(item(remote, id, &path));

            continue;
        }

        if let Err(error) = runtime.block_on(extract_file(store, &stored, &disk)) {
            failed.push(format!("{id}: {error}"));

            continue;
        }
        if let Err(cause) = checkout.remember(path.as_str()) {
            failed.push(format!("{id}: {cause}"));

            continue;
        }

        // What the Layout says is the version the Vault holds, the holder it names and the cut it
        // prefers; the path is this side's own, which is what the run was asked for. Nothing is
        // rebuilt from its parts: rebuilding is how a preference would be dropped, and a dropped
        // preference is reported nowhere, the entry would go on being written, cut another way
        // than the one it asked for.
        if let Err(error) = batch.create_file(id, data, &path) {
            failed.push(format!("{id}: {error}"));

            continue;
        }

        entries.push(item(remote, id, &path));
    }

    if let Err(cause) = checkout.finish() {
        failed.push(cause.to_string());
    }

    if let Err(error) = batch.commit() {
        failed.push(error.to_string());
    }

    Ok(Outcome { entries, failed })
}

/// One entry a bring put in, said the way its caller reports it.
fn item(remote: &Layout, id: Uuid, path: &LayoutPath) -> Item {
    Item {
        uuid: id.to_string(),
        remote_path: remote.path_of(id).map(|path| path.as_str().to_owned()),
        local_path: path.as_str().to_owned(),
    }
}

/// Writes `structure` under `root`, making the way to each entry.
///
/// A structure is what a version holds and the work is what a directory shows, so a tree brought in
/// has to be put back the way a restore and a settle put one back: a directory for every directory,
/// the stored content for every file, and the target for every link. What the structure does not
/// name is untouched, which is the caller's decision rather than this reading's.
///
/// The failure is a message because every caller reports it beside the entry it is about, and a
/// deeper type would only be turned into the same words one step later.
///
/// # Errors
///
/// Returns what the directory, the store or a link could not do.
pub(super) fn write_worktree(
    root: &Path,
    structure: &TreeStructure,
    store: &StorageSet,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;

    for entry in structure.entries() {
        let at = root.join(entry.path());

        match entry.kind() {
            TreeKind::Directory => {
                std::fs::create_dir_all(&at).map_err(|error| error.to_string())?;
            }
            TreeKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };
                if let Some(parent) = at.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }

                runtime
                    .block_on(extract_file(store, storage, &at))
                    .map_err(|error| error.to_string())?;
            }
            TreeKind::Symlink => {
                let Some(target) = entry.target() else {
                    continue;
                };
                if let Some(parent) = at.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                if at.exists() {
                    let _ = std::fs::remove_file(&at);
                }

                #[cfg(unix)]
                std::os::unix::fs::symlink(target, &at).map_err(|error| error.to_string())?;
                #[cfg(windows)]
                std::os::windows::fs::symlink_file(target, &at)
                    .map_err(|error| error.to_string())?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rorolala_layout::LayoutPath;

    use super::{
        BringInput, BringRequest, BringVariantRequest, Destination, Item, Outcome, Refusal,
        VariantRefusal, VariantReport, WriteBring, bring_at, is_pattern, landing, matching, parent,
        pattern_base, placeable, rooted, under,
    };
    use crate::write::{Cancellation, WriteError, WriteProgress};

    /// A path a Layout would name.
    fn path(text: &str) -> LayoutPath {
        LayoutPath::new(text).expect("a path a Layout could name")
    }

    #[test]
    fn a_missing_destination_lands_at_the_name_the_vault_gives() {
        assert_eq!(
            landing(&Destination::Named, &path("a/b.psd"), ""),
            Some(path("a/b.psd"))
        );
    }

    #[test]
    fn a_whole_name_destination_is_the_path_itself() {
        assert_eq!(
            landing(&Destination::Exact(path("here.psd")), &path("a/b.psd"), ""),
            Some(path("here.psd"))
        );
    }

    #[test]
    fn a_directory_destination_keeps_the_path_and_rebases_what_a_glob_selected() {
        let named = path("a/b.psd");

        // A path keeps all of itself under the destination, so the folder it is in is remade there.
        assert_eq!(
            landing(&Destination::Under("dir".to_owned()), &named, ""),
            Some(path("dir/a/b.psd"))
        );

        // A glob match keeps only what it has below the directory the glob selected from.
        assert_eq!(
            landing(&Destination::Under("dir".to_owned()), &named, "a"),
            Some(path("dir/b.psd"))
        );

        // The root as a destination adds no segment.
        assert_eq!(
            landing(&Destination::Under(String::new()), &named, ""),
            Some(path("a/b.psd"))
        );
    }

    #[test]
    fn a_name_is_read_against_the_directory_it_stands_in() {
        assert_eq!(parent("Assets/Tex/wood.png"), "Assets/Tex");
        assert_eq!(parent("Assets/Tex"), "Assets");
        // A name at the top of the Workspace stands in the root, which has no name of its own.
        assert_eq!(parent("Tex"), "");
    }

    #[test]
    fn a_by_name_destination_lands_a_reference_by_its_own_name() {
        // What `--by-name` makes of a path reference: the base is the directory it stands in, so
        // the entry keeps its own name and, under a folder, the tree the folder is.
        let folder = "Assets/Tex";

        assert_eq!(parent(folder), "Assets");

        for (named, expected) in [
            ("Assets/Tex/wood.png", "dir/Tex/wood.png"),
            ("Assets/Tex", "dir/Tex"),
        ] {
            let base = parent(folder);

            assert_eq!(
                landing(&Destination::Under("dir".to_owned()), &path(named), &base),
                Some(path(expected))
            );
        }
    }

    #[test]
    fn an_unplaced_path_lands_by_the_name_below_the_mark() {
        assert_eq!(placeable("@/new/a.fbx"), Some("new/a.fbx"));
        assert_eq!(placeable("@/removed/a.fbx"), Some("removed/a.fbx"));
        assert_eq!(placeable("a/b.fbx"), Some("a/b.fbx"));
        assert_eq!(placeable("@/"), None);

        // Both ways of landing drop the mark: the name the Vault gives, and the same name under a
        // directory.
        assert_eq!(
            landing(&Destination::Named, &path("@/new/a.fbx"), ""),
            Some(path("new/a.fbx"))
        );
        assert_eq!(
            landing(
                &Destination::Under("art".to_owned()),
                &path("@/new/a.fbx"),
                ""
            ),
            Some(path("art/new/a.fbx"))
        );

        // A glob over the unplaced area selects from it, so its matches have no mark left to drop.
        assert_eq!(
            landing(
                &Destination::Under(String::new()),
                &path("@/new/a.fbx"),
                "@/new"
            ),
            Some(path("a.fbx"))
        );
    }

    #[test]
    fn a_glob_selects_from_the_directory_before_it() {
        assert_eq!(pattern_base("Installers/*"), "Installers");
        assert_eq!(pattern_base("Installers/**"), "Installers");
        assert_eq!(pattern_base("Installers/sub/*.psd"), "Installers/sub");
        assert_eq!(pattern_base("*.psd"), "");
        assert_eq!(pattern_base("a?b"), "");
    }

    #[test]
    fn something_below_a_path_is_not_the_path_itself() {
        assert!(under("a/b.psd", "a"));
        assert!(!under("a", "a"));
        assert!(!under("ab/b.psd", "a"));
    }

    #[test]
    fn a_path_without_a_pattern_character_is_named_exactly() {
        assert!(!is_pattern("Installers/0dCloud.pkg.tar.zst"));
        assert!(is_pattern("Installers/*"));
        assert!(is_pattern("a?b"));
        assert!(is_pattern("a[0-9]"));
        assert!(is_pattern("a{b,c}"));
    }

    #[test]
    fn a_glob_names_the_paths_it_matches_and_nothing_else() {
        let paths = [
            path("Installers/0dCloud.pkg.tar.zst"),
            path("Installers/sub/extra.pkg"),
            path("README.md"),
        ];

        assert_eq!(
            matching("Installers/*", &paths).unwrap(),
            vec![path("Installers/0dCloud.pkg.tar.zst")]
        );
        assert_eq!(
            matching("Installers/**", &paths).unwrap(),
            vec![
                path("Installers/0dCloud.pkg.tar.zst"),
                path("Installers/sub/extra.pkg")
            ]
        );
        assert_eq!(
            matching("Missing/*", &paths).unwrap(),
            Vec::<LayoutPath>::new()
        );
        assert!(matches!(
            matching("Installers/[", &paths),
            Err(Refusal::Argument)
        ));
    }

    #[test]
    fn a_recursive_wildcard_crosses_directories() {
        let paths = [path("a/b.psd"), path("a/c/d.psd"), path("b.psd")];

        assert_eq!(
            matching("**/*.psd", &paths).unwrap(),
            vec![path("a/b.psd"), path("a/c/d.psd"), path("b.psd")]
        );
        assert_eq!(
            matching("a/**", &paths).unwrap(),
            vec![path("a/b.psd"), path("a/c/d.psd")]
        );
        assert_eq!(
            matching("a/**/d.psd", &paths).unwrap(),
            vec![path("a/c/d.psd")]
        );
        assert_eq!(
            matching("a/**/b.psd", &paths).unwrap(),
            vec![path("a/b.psd")]
        );
        // A `**` alone names every path, the way a shell reads it.
        assert_eq!(matching("**", &paths).unwrap().len(), 3);
    }

    #[test]
    fn a_class_names_one_character_of_the_set() {
        let paths = [path("a.psd"), path("b.psd"), path("c.png"), path("d.psd")];

        assert_eq!(
            matching("[ab].psd", &paths).unwrap(),
            vec![path("a.psd"), path("b.psd")]
        );
        assert_eq!(matching("[!ab].psd", &paths).unwrap(), vec![path("d.psd")]);
        assert_eq!(
            matching("[a-c].*", &paths).unwrap(),
            vec![path("a.psd"), path("b.psd"), path("c.png")]
        );
        assert!(matches!(
            matching("[z-a].psd", &paths),
            Err(Refusal::Argument)
        ));
    }

    #[test]
    fn an_alternation_names_any_of_its_branches() {
        let paths = [path("a.psd"), path("b.psd"), path("c.png")];

        assert_eq!(
            matching("*.{psd,ai}", &paths).unwrap(),
            vec![path("a.psd"), path("b.psd")]
        );
        assert_eq!(matching("{a,c}.png", &paths).unwrap(), vec![path("c.png")]);
        // An empty branch is nothing, so the pattern around it is the whole of it.
        assert_eq!(matching("{a,}.psd", &paths).unwrap(), vec![path("a.psd")]);
        assert!(matches!(
            matching("{a,b.psd", &paths),
            Err(Refusal::Argument)
        ));
    }

    #[test]
    fn a_request_reads_its_references_beside_the_common_fields() {
        let request: BringRequest = serde_json::from_str(
            r#"{"workspace":"/w","account":"alice","refs":["a/b.psd","Assets/*"],"to":"here","by_name":true}"#,
        )
        .expect("a request");

        assert_eq!(request.refs, ["a/b.psd", "Assets/*"]);
        assert_eq!(request.to.as_deref(), Some("here"));
        assert!(request.by_name);
        assert_eq!(request.common.workspace, std::path::PathBuf::from("/w"));
    }

    #[test]
    fn a_request_that_names_no_destination_and_no_by_name_reads_the_defaults() {
        let request: BringRequest =
            serde_json::from_str(r#"{"workspace":"/w","account":"alice","refs":["a"]}"#)
                .expect("a request");

        assert_eq!(request.to, None);
        assert!(!request.by_name);
        assert_eq!(request.join, None);
    }

    #[test]
    fn a_request_that_names_a_join_is_read_as_the_variant_merge() {
        // The two brings are one document, so what says which of them was asked for is the one
        // field only the variant merge has, and the references are then empty rather than absent.
        let request: BringRequest = serde_json::from_str(
            r#"{"workspace":"/w","account":"alice","join":"merge/file.txt","variant":"5b4a","file":"merge/file.txt"}"#,
        )
        .expect("a request");

        assert_eq!(request.join.as_deref(), Some("merge/file.txt"));
        assert_eq!(request.refs, Vec::<String>::new());
    }

    #[test]
    fn a_variant_request_reads_its_variant_and_the_file_it_joins_beside_the_common_fields() {
        let request: BringVariantRequest = serde_json::from_str(
            r#"{"workspace":"/w","account":"alice","join":"merge/file.txt","variant":"5b4a","file":"merge/file.txt","to":"here.bin","tree_merge":true}"#,
        )
        .expect("a request");

        assert_eq!(request.variant, "5b4a");
        assert_eq!(request.file, "merge/file.txt");
        assert_eq!(request.to.as_deref(), Some("here.bin"));
        assert!(request.tree_merge);
        assert_eq!(request.common.workspace, std::path::PathBuf::from("/w"));
    }

    #[test]
    fn a_variant_request_that_names_no_place_and_no_tree_merge_reads_the_defaults() {
        let request: BringVariantRequest =
            serde_json::from_str(r#"{"workspace":"/w","variant":"5b4a","file":"a.txt"}"#)
                .expect("a request");

        assert_eq!(request.to, None);
        assert!(!request.tree_merge);
    }

    #[test]
    fn a_variant_report_reads_as_the_three_things_the_run_named() {
        let report = VariantReport {
            target: "merge/file.txt".to_owned(),
            variant: "5b4a".to_owned(),
            path: "merge/file.txt.theirs".to_owned(),
        };

        let answer = WriteBring::of_variant(&report);
        assert!(answer.joined());
        assert_eq!(answer.target(), "merge/file.txt");
        assert_eq!(answer.variant(), "5b4a");
        assert_eq!(answer.path(), "merge/file.txt.theirs");
        assert_eq!(answer.entry_count(), 0);
    }

    #[test]
    fn a_refused_variant_is_a_write_error_a_caller_branches_on() {
        assert!(matches!(
            WriteError::from(VariantRefusal::NotATarget),
            WriteError::Precondition { .. }
        ));
        assert!(matches!(
            WriteError::from(VariantRefusal::NoStore),
            WriteError::NoStore
        ));
        // A cancelled run keeps the error every other write answers with, so a caller has one
        // meaning to branch on rather than a cancellation of its own.
        assert!(matches!(
            WriteError::from(VariantRefusal::Failed(WriteError::Cancelled)),
            WriteError::Cancelled
        ));
    }

    #[test]
    fn an_outcome_reads_as_its_entries_and_what_failed() {
        let outcome = Outcome {
            entries: vec![
                Item {
                    uuid: "u".to_owned(),
                    remote_path: Some("a/b.psd".to_owned()),
                    local_path: "here/b.psd".to_owned(),
                },
                Item {
                    uuid: "v".to_owned(),
                    remote_path: None,
                    local_path: "c.psd".to_owned(),
                },
            ],
            failed: vec!["v: the version's content is not here".to_owned()],
        };

        let answer = WriteBring::of_entries(&outcome);
        assert!(!answer.joined());
        assert_eq!(answer.entry_count(), 2);
        let first = answer.entry_at(0).expect("the first entry is there");
        assert_eq!(first.uuid(), "u");
        assert_eq!(first.remote_path(), "a/b.psd");
        assert_eq!(first.local_path(), "here/b.psd");
        let second = answer.entry_at(1).expect("the second entry is there");
        assert_eq!(second.remote_path(), "");
        assert_eq!(answer.failed_count(), 1);
        assert_eq!(answer.failed_at(0), "v: the version's content is not here");
    }

    #[test]
    fn a_relative_destination_is_read_against_the_workspace_root() {
        let root = std::path::Path::new("root");

        assert_eq!(
            rooted(root, "art"),
            std::path::Path::new("root")
                .join("art")
                .display()
                .to_string()
        );

        // A destination that already says where it is is left there, wherever the root is.
        let absolute = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("art");
        assert_eq!(
            rooted(root, &absolute.display().to_string()),
            absolute.display().to_string()
        );
    }

    #[test]
    fn a_bring_outside_a_workspace_is_refused() {
        let dir = std::env::temp_dir().join(format!("rola-write-bring-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");

        let request = BringInput {
            common: super::super::request::WriteCommon {
                workspace: dir.display().to_string(),
                account: "alice".to_owned(),
                ..Default::default()
            },
            refs: vec!["a".to_owned()].into(),
            ..Default::default()
        };
        let error = bring_at(&request, &WriteProgress::new(), &Cancellation::new())
            .expect_err("no Workspace holds the directory");

        assert!(matches!(error, WriteError::NoWorkspace));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_variant_bring_outside_a_workspace_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-bring-variant-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");

        let request = BringInput {
            common: super::super::request::WriteCommon {
                workspace: dir.display().to_string(),
                account: "alice".to_owned(),
                ..Default::default()
            },
            join: "a.txt".to_owned(),
            variant: "5b4a".to_owned(),
            file: "a.txt".to_owned(),
            ..Default::default()
        };
        let error = bring_at(&request, &WriteProgress::new(), &Cancellation::new())
            .expect_err("no Workspace holds the directory");

        assert!(matches!(error, WriteError::NoWorkspace));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_variant_bring_that_was_cancelled_writes_nothing() {
        let dir =
            std::env::temp_dir().join(format!("rola-write-bring-cancelled-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");

        // Cancellation is checked before the Workspace is even resolved, so a request that names
        // none still stops on the flag rather than on what it does not name.
        let request = BringInput {
            common: super::super::request::WriteCommon {
                workspace: dir.display().to_string(),
                ..Default::default()
            },
            join: "a.txt".to_owned(),
            variant: "5b4a".to_owned(),
            file: "a.txt".to_owned(),
            ..Default::default()
        };
        let cancellation = Cancellation::new();
        cancellation.cancel();
        let error = bring_at(&request, &WriteProgress::new(), &cancellation)
            .expect_err("the run was asked to stop");

        assert!(matches!(error, WriteError::Cancelled));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod cross_tests {
    use uuid::Uuid;

    use std::path::Path;

    use super::{
        HashMiss, Item, Messages, Outcome, Refusal, VariantRefusal, VariantReport, WriteBring,
        WriteBringEntry, WriteError, is_pattern, parent, pattern_base, placeable, prefix, refused,
        resolve_against, rooted, under,
    };

    #[test]
    fn every_variant_refusal_is_the_write_error_a_caller_branches_on() {
        let cases = [
            VariantRefusal::NotAVariantHash {
                hash: "h".to_owned(),
                miss: HashMiss::Malformed,
            },
            VariantRefusal::NotAVariantHash {
                hash: "h".to_owned(),
                miss: HashMiss::Unknown,
            },
            VariantRefusal::NotAVariantHash {
                hash: "h".to_owned(),
                miss: HashMiss::Ambiguous(vec!["a".to_owned(), "b".to_owned()]),
            },
            VariantRefusal::NotATarget,
            VariantRefusal::AlreadyMerging,
            VariantRefusal::AlreadyBroughtIn,
            VariantRefusal::UnknownVariant,
            VariantRefusal::OffChain,
            VariantRefusal::NoStore,
            VariantRefusal::NoContent,
            VariantRefusal::PlaceOutside,
            VariantRefusal::PlaceTaken {
                path: "a".to_owned(),
            },
            VariantRefusal::Chain,
            VariantRefusal::NoTrack,
            VariantRefusal::NoAccount,
            VariantRefusal::NoVault,
        ];

        for refusal in cases {
            let _ = WriteError::from(refusal);
        }

        let failed = WriteError::from(VariantRefusal::Failed(WriteError::NoIndex));
        assert!(matches!(failed, WriteError::NoIndex));

        let malformed = WriteError::from(VariantRefusal::NotAVariantHash {
            hash: "h".to_owned(),
            miss: HashMiss::Malformed,
        });
        assert!(matches!(malformed, WriteError::NotFound { .. }));

        let ambiguous = WriteError::from(VariantRefusal::NotAVariantHash {
            hash: "h".to_owned(),
            miss: HashMiss::Ambiguous(vec!["a".to_owned()]),
        });
        assert!(matches!(ambiguous, WriteError::Precondition { .. }));
    }

    #[test]
    fn every_reference_refusal_is_the_write_error_a_caller_branches_on() {
        let id = Uuid::from_u128(1);

        assert!(matches!(
            refused(Refusal::Argument),
            WriteError::Precondition { .. }
        ));
        assert!(matches!(
            refused(Refusal::Unknown("r".to_owned())),
            WriteError::NotFound { .. }
        ));
        assert!(matches!(
            refused(Refusal::Unnamed(id)),
            WriteError::Precondition { .. }
        ));
        assert!(matches!(
            refused(Refusal::Taken("a".to_owned())),
            WriteError::Precondition { .. }
        ));
        assert!(matches!(
            refused(Refusal::Known(id.to_string())),
            WriteError::Precondition { .. }
        ));
    }

    #[test]
    fn a_bring_of_entries_crosses_its_entries_and_failures() {
        let outcome = Outcome {
            entries: vec![Item {
                uuid: "u".to_owned(),
                remote_path: Some("remote".to_owned()),
                local_path: "local".to_owned(),
            }],
            failed: vec!["bad".to_owned()],
        };

        let answer = WriteBring::of_entries(&outcome);

        assert!(!answer.joined());
        assert_eq!(answer.entry_count(), 1);
        let entry = answer.entry_at(0).unwrap();
        assert_eq!(entry.uuid(), "u");
        assert_eq!(entry.remote_path(), "remote");
        assert_eq!(entry.local_path(), "local");
        assert!(answer.entry_at(1).is_none());
        assert_eq!(answer.failed_count(), 1);
        assert_eq!(answer.failed_at(0), "bad");
        assert_eq!(answer.failed_at(1), "");
        assert_eq!(answer.target(), "");
        assert_eq!(answer.variant(), "");
        assert_eq!(answer.path(), "");
    }

    #[test]
    fn a_variant_merge_crosses_the_three_things_it_names() {
        let report = VariantReport {
            target: "target".to_owned(),
            variant: "variant".to_owned(),
            path: "path".to_owned(),
        };

        let answer = WriteBring::of_variant(&report);

        assert!(answer.joined());
        assert_eq!(answer.entry_count(), 0);
        assert_eq!(answer.failed_count(), 0);
        assert_eq!(answer.target(), "target");
        assert_eq!(answer.variant(), "variant");
        assert_eq!(answer.path(), "path");
    }

    #[test]
    fn an_entry_with_no_remote_path_says_so() {
        let entry = WriteBringEntry::of(&Item {
            uuid: "u".to_owned(),
            remote_path: None,
            local_path: "local".to_owned(),
        });

        assert_eq!(entry.uuid(), "u");
        assert_eq!(entry.remote_path(), "");
        assert_eq!(entry.local_path(), "local");
    }

    #[test]
    fn the_default_words_are_the_ones_the_library_uses() {
        let messages = Messages::default();

        assert_eq!(messages.no_content, "the version's content is not here");
        assert_eq!(messages.tree_mark, "the tree's mark could not be read");
    }

    #[test]
    fn a_variant_request_is_built_from_the_values_a_caller_holds() {
        let request = super::BringVariantRequest::new(
            crate::write::request::Common::default(),
            "variant".to_owned(),
            "file".to_owned(),
            Some("place".to_owned()),
            true,
        );

        assert_eq!(request.variant, "variant");
        assert_eq!(request.file, "file");
        assert_eq!(request.to.as_deref(), Some("place"));
        assert!(request.tree_merge);
    }

    #[test]
    fn a_reference_is_read_as_a_path_or_a_pattern() {
        assert_eq!(parent("a/b/c"), "a/b");
        assert_eq!(parent("a"), "");
        assert_eq!(pattern_base("Assets/*"), "Assets");
        assert_eq!(pattern_base("Assets/**"), "Assets");
        assert_eq!(pattern_base("*"), "");

        assert!(under("a/b", "a"));
        assert!(!under("a", "a"));
        assert!(!under("ab", "a"));

        assert!(is_pattern("a/*"));
        assert!(!is_pattern("a/b"));

        assert_eq!(placeable("@/new/a.fbx"), Some("new/a.fbx"));
        assert_eq!(placeable("@/"), None);
        assert_eq!(placeable("a/b"), Some("a/b"));

        let root = std::env::temp_dir();
        let absolute = root.join("abs");
        assert_eq!(
            rooted(&root, &absolute.display().to_string()),
            absolute.display().to_string()
        );
        assert_eq!(rooted(&root, "rel"), root.join("rel").display().to_string());
        assert_eq!(resolve_against(&root, "rel"), root.join("rel"));
        assert_eq!(
            resolve_against(&root, &absolute.display().to_string()),
            absolute
        );
        assert_eq!(prefix(Path::new("a/b")), "a/b");
    }
}

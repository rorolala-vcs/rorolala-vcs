//! `fetch --with`: bringing the content of the entries, variants and versions a Layout names.
//!
//! A fetch brings a Vault's Layout here, and that copy names versions: which path names which
//! `Uuid`, and what version each entry is at. What a version means is content, and the content is in
//! a store, which may be elsewhere. This is the fetch that closes that gap: it takes the words a run
//! names, reads them as entries of a Layout or as objects of the index, and brings the content they
//! are made of, so each one can be produced here.
//!
//! A word comes in four shapes:
//!
//! 1. a path or a directory of the Layout, which names the entries there;
//! 2. a `Uuid`, which names the entry it is at;
//! 3. a variant's hash, whole or the head of one, which names the content its `storage` field
//!    points at;
//! 4. a version's hash, whole or the head of one, which names the content of the variant it fixes.
//!
//! A hash is resolved among the objects the index holds, and the index is moved whole when one is
//! not here, which is the same exchange the versions of a path need. A word that reads as a hash and
//! names no variant or version is a refusal rather than a word passed over: a path may name nothing
//! yet, since a directory holds files that were never recorded, but a hash was meant for an object.
//!
//! How far a target goes is the run's choice. On an entry or a version, a shallow fetch brings the
//! content of the version named, and a recursive one the whole chain that version stands on. On a
//! variant, a shallow fetch brings the content the variant points at, and a recursive one also the
//! chain of the version it is based on: the variant itself is the top of what it stands on, so
//! nothing above it comes. The line walked is the one `base_version` draws, the same one an upload
//! walks, and a merge's joined side is not part of it.
//!
//! A tree's version names a structure rather than bytes, and the structure is what says what the
//! inside is made of. So a fetch of a tree is two exchanges: the structure, and then what its
//! entries hold. An entry says which kind it is, so a file's bytes are never read as a structure;
//! a version or a variant says nothing of the kind, and what it names is tried instead, which is
//! what the status reading does for the same targets.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use rorolala_errors::Failure as _;
use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_storage::{Resolved, ShortHash, StorageBackend as _};
use rorolala_utils_lazyffi::{StrSpan, lazyffi};
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_utils_progress::Progress;
use rorolala_vcs::{ComputeBlake3 as _, VCSIndex, VCSIndexKind, VCSIndexObject};
use rorolala_workspace::Workspace;
use serde::Deserialize;
use uuid::Uuid;

use crate::daemon::action_sync_index_all_async;
use crate::fetch::Sources;
use crate::layout_read::{self, LayoutReadError};
use crate::protocol::ActionError;
use crate::storage::{Key, StorageSet};
use crate::tree::TreeStructure;
use crate::write::progress::{WriteProgress, watching};
use crate::write::request::{Common, WriteCommon};
use crate::write::sync::{chain_store_keys, store_of};
use crate::write::{Cancellation, WriteError};

/// What a fetch of named content is asked for.
///
/// The words are read in the Layout `layout` names when they are paths: empty or `main` is the one
/// the Workspace works in, `truth` a Vault's own, and `NAME@VAULT` the copy a fetch left. A hashed
/// word needs no Layout, so a run that names only hashes reads none.
#[derive(Debug, Deserialize)]
pub struct FetchWithRequest {
    /// The fields every write request carries.
    #[serde(flatten)]
    pub common: Common,
    /// The Layout the words are read in, empty for the one being worked in.
    #[serde(default)]
    pub layout: String,
    /// The paths, directories, `Uuid`s and hashes whose content comes.
    #[serde(default)]
    pub words: Vec<String>,
    /// Whether each target comes with the chain behind it or alone.
    #[serde(default)]
    pub recursive: bool,
}

/// A content fetch a C caller asks for.
///
/// # FFI
///
/// A `RolaWriteFetchRequest` the caller fills. Every `const char *` is borrowed for the call and
/// released by the caller, so nothing here is freed when the call returns. `words` is a
/// `RolaStrSpan` of the paths, directories, `Uuid`s and hashes whose content comes, and an empty
/// `layout` reads the one being worked in.
#[lazyffi(export = RolaWriteFetchRequest, input)]
#[derive(Debug, Clone)]
pub struct FetchInput {
    /// The fields every write request carries.
    pub common: WriteCommon,
    /// The Layout the words are read in, empty for the one being worked in.
    pub layout: String,
    /// The paths, directories, `Uuid`s and hashes whose content comes.
    pub words: StrSpan,
    /// Whether each target comes with the chain behind it or alone.
    pub recursive: bool,
}

impl From<FetchInput> for FetchWithRequest {
    fn from(input: FetchInput) -> Self {
        Self {
            common: input.common.into(),
            layout: input.layout,
            words: input.words.into_vec(),
            recursive: input.recursive,
        }
    }
}

/// One target whose content was fetched.
#[derive(Debug)]
pub struct FetchWithEntry {
    /// What it was fetched by: a `Uuid`, a variant's hash, or a version's hash.
    pub target: String,
    /// The path it is at, `.` when it is at none or is not an entry.
    pub path: String,
    /// The version the content is of, as hex.
    pub version: String,
}

/// One target whose content is still not here.
#[derive(Debug, Clone)]
pub struct FetchWithMissing {
    /// What it was fetched by.
    pub target: String,
    /// The path it is at, `.` when it is at none or is not an entry.
    pub path: String,
}

/// How many objects one Vault handed over.
#[derive(Debug)]
pub struct FetchWithSource {
    /// The Vault's name.
    pub vault: String,
    /// The objects it handed over.
    pub count: usize,
}

/// What a fetch of content came to.
#[derive(Debug)]
pub struct FetchWithReport {
    /// The targets whose content was fetched, in the order they were named.
    pub entries: Vec<FetchWithEntry>,
    /// Each Vault that handed something over.
    pub brought: Vec<FetchWithSource>,
}

/// Why a word written as a hash named no one object.
#[derive(Debug)]
pub enum HashMiss {
    /// Nothing the index holds starts with the digits, or the key is no object it holds.
    Unknown,
    /// More than one object does.
    Ambiguous(Vec<String>),
}

/// Why a fetch of content could not be made.
#[derive(Debug)]
pub enum FetchWithRefusal {
    /// The run was asked to stop.
    Cancelled,
    /// The Workspace works in no Layout.
    NoLayout,
    /// The copy of a Vault's Layout a name asked for was never fetched here.
    NotCached {
        /// The Layout's own name.
        layout: String,
        /// The Vault it belongs to.
        vault: String,
    },
    /// The name is not one a place has a Layout under.
    LayoutName,
    /// The Layout is there, but it could not be read.
    LayoutFailed {
        /// Why it could not be read.
        reason: String,
    },
    /// A word written as a hash names no one variant or version the index holds.
    Hash {
        /// The word, as it was written.
        hash: String,
        /// Why it named no one object.
        miss: HashMiss,
    },
    /// The index holds no chain for a target the words name.
    Chain {
        /// The target the chain was wanted for.
        target: String,
        /// Why no chain could be read.
        reason: String,
    },
    /// The content of these targets is still not here.
    Incomplete {
        /// The targets still missing content.
        entries: Vec<FetchWithMissing>,
    },
    /// The exchange with the Vault failed.
    Action(ActionError),
    /// A Workspace, a store, an index or a configuration failed in its own way.
    Failed(WriteError),
}

impl FetchWithRequest {
    /// Brings the content the words name out of the Vaults, so each target can be produced here.
    ///
    /// The Layout named is read where it is when a word is a path, the index is moved whole when a
    /// version or a hash it must name is not here, and the content is asked of whichever Vaults hold
    /// it. A path word that names no entry and no directory holding one is passed over: a directory
    /// may hold files that were never recorded, and those are exactly the words that name nothing
    /// yet. A hashed word that names no object is refused, since a hash is meant for one.
    ///
    /// `progress` is what the exchanges say themselves to, and `cancellation` is checked before the
    /// run begins, after the index is asked for, and after the content is.
    ///
    /// # Errors
    ///
    /// Returns [`FetchWithRefusal::Cancelled`] when the run was asked to stop,
    /// [`FetchWithRefusal::Failed`] around [`WriteError::Offline`] when the run may reach no Vault,
    /// [`FetchWithRefusal::NoLayout`], [`FetchWithRefusal::NotCached`],
    /// [`FetchWithRefusal::LayoutName`] or [`FetchWithRefusal::LayoutFailed`] when the Layout named
    /// cannot be read, [`FetchWithRefusal::Hash`] when a hashed word names no one object,
    /// [`FetchWithRefusal::Chain`] when the index holds no chain for a target,
    /// [`FetchWithRefusal::Action`] when moving the index failed,
    /// [`FetchWithRefusal::Incomplete`] when the content of a target did not arrive, and
    /// [`FetchWithRefusal::Failed`] for a Workspace, a store, an index or a configuration that
    /// failed otherwise.
    #[allow(
        clippy::too_many_lines,
        reason = "the run reads a Layout, resolves hashes, moves an index, gathers keys, fetches them in two exchanges and checks every target, in one order a caller has to be able to follow"
    )]
    pub fn fetch_with(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<FetchWithReport, FetchWithRefusal> {
        if cancellation.is_cancelled() {
            return Err(FetchWithRefusal::Cancelled);
        }

        // Content is not here to be read: it is elsewhere, and asking for it is the whole command.
        if self.common.offline {
            return Err(FetchWithRefusal::Failed(WriteError::Offline));
        }

        let workspace = self.common.workspace().map_err(FetchWithRefusal::Failed)?;
        let store = self
            .common
            .store(&workspace)
            .map_err(FetchWithRefusal::Failed)?;
        let index = self
            .common
            .index(&workspace)
            .map_err(FetchWithRefusal::Failed)?;
        let account = self
            .common
            .account(&workspace)
            .map_err(FetchWithRefusal::Failed)?;
        let reached = self
            .common
            .vault(&workspace)
            .map_err(FetchWithRefusal::Failed)?;
        let runtime = tokio::runtime::Runtime::new().map_err(|error| {
            FetchWithRefusal::Failed(WriteError::Internal {
                reason: error.to_string(),
            })
        })?;

        // A hash is an object of the index, and every other word is a path of a Layout. The split is
        // what lets a run of hashes read no Layout at all.
        let mut named_words: Vec<String> = Vec::new();
        let mut hashed_words: Vec<String> = Vec::new();
        for word in &self.words {
            if reads_as_hash(word) {
                hashed_words.push(word.clone());
            } else {
                named_words.push(word.clone());
            }
        }

        let root = workspace.get_root();
        let cwd = std::env::current_dir().unwrap_or_else(|_| root.to_path_buf());
        let layout = if named_words.is_empty() {
            None
        } else {
            Some(self.layout_of(&workspace)?)
        };

        let mut named = layout.as_ref().map_or_else(Vec::new, |layout| {
            resolve_words(layout, root, &cwd, is_remote(&self.layout), &named_words)
        });

        // Nothing named is nothing to fetch: a run that named only files never recorded is the same
        // as a run that named none at all.
        if named.is_empty() && hashed_words.is_empty() {
            return Ok(FetchWithReport {
                entries: Vec::new(),
                brought: Vec::new(),
            });
        }

        // What the index must say before a target can be turned into content: the versions the path
        // words name, and the objects the hashed words name. One exchange covers both, and it is
        // made only when something is not here.
        let mut absent = absent_versions(&index, &runtime, &named)?;
        let mut resolved = Vec::with_capacity(hashed_words.len());
        let mut first_miss: Option<(String, HashMiss)> = None;
        for word in &hashed_words {
            match resolve_hash(&index, &runtime, word) {
                Ok(target) => resolved.push(target),
                Err(FetchWithRefusal::Hash { hash, miss }) => {
                    absent = true;

                    if first_miss.is_none() {
                        first_miss = Some((hash, miss));
                    }
                }
                Err(other) => return Err(other),
            }
        }

        if absent {
            runtime
                .block_on(action_sync_index_all_async(
                    &workspace,
                    &account,
                    reached.address.to_string(),
                    progress.clone(),
                ))
                .map_err(FetchWithRefusal::Action)?;

            if cancellation.is_cancelled() {
                return Err(FetchWithRefusal::Cancelled);
            }

            // The versions are asked about again, and so is every hashed word: a short head that
            // named nothing here may name one object once the Vault's index has arrived.
            for target in &named {
                if !version_held(&index, &runtime, target)? {
                    return Err(FetchWithRefusal::Chain {
                        target: target_name(target),
                        reason: "the index holds no version for it".to_owned(),
                    });
                }
            }

            resolved.clear();
            first_miss = None;
            for word in &hashed_words {
                match resolve_hash(&index, &runtime, word) {
                    Ok(target) => resolved.push(target),
                    Err(FetchWithRefusal::Hash { hash, miss }) => {
                        if first_miss.is_none() {
                            first_miss = Some((hash, miss));
                        }
                    }
                    Err(other) => return Err(other),
                }
            }
        }

        if let Some((hash, miss)) = first_miss {
            return Err(FetchWithRefusal::Hash { hash, miss });
        }

        named.extend(resolved);

        if cancellation.is_cancelled() {
            return Err(FetchWithRefusal::Cancelled);
        }

        // What each target is made of, and for a recursive run the whole line it stands on.
        let mut wanted: Vec<Wanted> = Vec::with_capacity(named.len());
        for target in named {
            let name = target_name(&target);
            let keys = match &target {
                Named::Entry { version, .. } => {
                    version_keys(&index, &runtime, version, self.recursive).map_err(|reason| {
                        FetchWithRefusal::Chain {
                            target: name.clone(),
                            reason,
                        }
                    })?
                }
                Named::Version { key } => {
                    version_keys(&index, &runtime, key.digest(), self.recursive).map_err(
                        |reason| FetchWithRefusal::Chain {
                            target: name.clone(),
                            reason,
                        },
                    )?
                }
                Named::Variant { storage, base, .. } => {
                    let mut keys = vec![*storage];

                    if self.recursive {
                        keys.extend(
                            runtime
                                .block_on(chain_store_keys(&index, base, None, false))
                                .map_err(|reason| FetchWithRefusal::Chain {
                                    target: name.clone(),
                                    reason,
                                })?,
                        );
                    }

                    keys
                }
            };

            if keys.is_empty() {
                return Err(FetchWithRefusal::Chain {
                    target: name,
                    reason: "the index holds no content for it".to_owned(),
                });
            }

            wanted.push(Wanted {
                expand: expands(&target),
                target,
                keys,
            });
        }

        let candidates = self
            .common
            .candidates(&workspace)
            .map_err(FetchWithRefusal::Failed)?;
        if candidates.is_empty() {
            return Err(FetchWithRefusal::Failed(WriteError::Precondition {
                what: "the Vaults to ask".to_owned(),
                reason: "the Workspace has bound none".to_owned(),
            }));
        }

        let sources = Sources::new(&workspace, &account, candidates, false)
            .map_err(|reason| FetchWithRefusal::Failed(WriteError::Internal { reason }))?
            .with_concurrency(self.common.concurrency)
            .reporting(progress.clone());

        // The first exchange brings every target's own content. What a key names is expanded by the
        // side holding it, so a content kept as chunks crosses with its manifest in one pass.
        let mut wanted_keys: Vec<Key> = wanted
            .iter()
            .flat_map(|wanted| wanted.keys.iter().copied())
            .collect();
        sort_dedup(&mut wanted_keys);
        let _ = sources.bring(&store, &wanted_keys);

        if cancellation.is_cancelled() {
            return Err(FetchWithRefusal::Cancelled);
        }

        // A structure is one version's inside, and what its entries hold is a second ask because
        // the structure is what names them. Only a target whose content may be a structure is read
        // as one: a file entry's kind is written down, so its bytes are never mistaken for one.
        let mut contents: Vec<Key> = Vec::new();
        for target in &wanted {
            contents.extend(structure_keys(&store, &runtime, target));
        }
        sort_dedup(&mut contents);
        let _ = sources.bring(&store, &contents);

        // What did not arrive is said per target rather than per key: a caller branches on the thing
        // it asked for, and the keys behind it are this side's business.
        let mut missing: Vec<FetchWithMissing> = Vec::new();
        for target in &wanted {
            let mut keys = target.keys.clone();
            keys.extend(structure_keys(&store, &runtime, target));
            sort_dedup(&mut keys);

            // A store that cannot answer is read as holding nothing, the way the fetch itself reads
            // one: what is not here is what is to be asked for.
            let held = runtime
                .block_on(store.contains_keys(&keys))
                .is_ok_and(|presence| (0..keys.len()).all(|index| presence.held(index)));

            if !held {
                missing.push(FetchWithMissing {
                    target: target_name(&target.target),
                    path: shown_path(&target.target),
                });
            }
        }

        if !missing.is_empty() {
            return Err(FetchWithRefusal::Incomplete { entries: missing });
        }

        let entries = wanted
            .iter()
            .map(|wanted| FetchWithEntry {
                target: target_name(&wanted.target),
                path: shown_path(&wanted.target),
                version: target_version(&wanted.target),
            })
            .collect();

        let brought = sources
            .took()
            .into_iter()
            .map(|(vault, count)| FetchWithSource { vault, count })
            .collect();

        Ok(FetchWithReport { entries, brought })
    }

    /// The Layout the words are read in.
    ///
    /// # Errors
    ///
    /// Returns the refusal [`FetchWithRefusal`] has for each way a name fails to resolve.
    fn layout_of(&self, workspace: &Workspace) -> Result<Layout, FetchWithRefusal> {
        let name = self.layout.trim();

        match layout_read::chosen(workspace.get_root(), name) {
            Ok(layout) => Ok(layout),
            Err(LayoutReadError::NoWorkspace) => {
                Err(FetchWithRefusal::Failed(WriteError::NoWorkspace))
            }
            Err(LayoutReadError::NoLayout) => Err(FetchWithRefusal::NoLayout),
            Err(LayoutReadError::NotCached) => {
                // The name is written `NAME@VAULT`, which is the only way this is reached, so the
                // two halves are what the failure is about. A name without one is answered whole.
                let (layout, vault) = name.split_once('@').unwrap_or((name, ""));

                Err(FetchWithRefusal::NotCached {
                    layout: layout.to_owned(),
                    vault: vault.to_owned(),
                })
            }
            Err(LayoutReadError::Missing) => Err(FetchWithRefusal::LayoutName),
            Err(LayoutReadError::Read { reason }) => Err(FetchWithRefusal::LayoutFailed { reason }),
        }
    }
}

/// The content fetch a request a C caller fills asks for.
///
/// # FFI
///
/// `request` is a `RolaWriteFetchRequest` the caller fills and keeps; every `const char *` in it is
/// borrowed for the call and released by the caller. A `RolaWriteFetch *` the caller releases with
/// `free_rola_write_fetch` answers it, or a `RolaWriteError *` the caller releases with
/// `free_rola_write_error`. `progress` is the handle the caller polls with `rola_progress_next`,
/// and `cancellation` is the flag it sets with `rola_cancellation_cancel`.
///
/// `_entry_count` and `_entry_at` name the targets whose content is here, in the order they were
/// named, and `_brought_count` and `_brought_at` say how many objects each Vault handed over.
/// `target` is the `Uuid` an entry is known by, a variant's hash, or a version's hash; `path` is
/// `.` for anything that is not an entry; `version` is the version the content is of.
///
/// The copy of a Vault's Layout a path word reads is the one a fetch left, so a caller outside the
/// command line fetches that copy first, with `rola_daemon_action_fetch_layout`, and calls this
/// after it. A run that names only hashes reads no Layout.
///
/// # Errors
///
/// Returns what [`FetchWithRequest::fetch_with`] names, folded into [`WriteError`].
#[lazyffi(export = rola_write_fetch)]
pub fn fetch_with_at(
    request: &FetchInput,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<WriteFetch, WriteError> {
    let asked: FetchWithRequest = request.clone().into();
    let watched = watching(progress);
    let report = asked.fetch_with(&watched, cancellation)?;

    Ok(WriteFetch::of(&report))
}

/// What a content fetch came to, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaWriteFetch *` the caller releases with `free_rola_write_fetch`. `_entry_count` and
/// `_entry_at` answer the targets, each a `RolaWriteFetchEntry *` released with
/// `free_rola_write_fetch_entry`; `_brought_count` and `_brought_at` answer what each Vault handed
/// over, each a `RolaWriteFetchSource *` released with `free_rola_write_fetch_source`.
#[lazyffi(export = RolaWriteFetch)]
#[derive(Debug, Clone, Default)]
pub struct WriteFetch {
    /// The targets whose content was fetched, in the order they were named.
    entries: Vec<WriteFetchEntry>,
    /// Each Vault that handed something over.
    brought: Vec<WriteFetchSource>,
}

impl WriteFetch {
    /// The answer a fetch of content is handed over as.
    fn of(report: &FetchWithReport) -> Self {
        Self {
            entries: report.entries.iter().map(WriteFetchEntry::of).collect(),
            brought: report.brought.iter().map(WriteFetchSource::of).collect(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteFetch {
    /// How many targets the run fetched.
    #[must_use]
    pub const fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// One target the run fetched, or null past the end.
    #[must_use]
    pub fn entry_at(&self, index: usize) -> Option<WriteFetchEntry> {
        self.entries.get(index).cloned()
    }

    /// How many Vaults handed something over.
    #[must_use]
    pub const fn brought_count(&self) -> usize {
        self.brought.len()
    }

    /// One Vault that handed something over, or null past the end.
    #[must_use]
    pub fn brought_at(&self, index: usize) -> Option<WriteFetchSource> {
        self.brought.get(index).cloned()
    }
}

/// One target whose content was fetched.
///
/// # FFI
///
/// A `RolaWriteFetchEntry *` the caller releases with `free_rola_write_fetch_entry`. `_target`,
/// `_path` and `_version` each return a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaWriteFetchEntry)]
#[derive(Debug, Clone, Default)]
pub struct WriteFetchEntry {
    /// What it was fetched by: a `Uuid`, a variant's hash, or a version's hash.
    target: String,
    /// The path it is at, `.` when it is at none or is not an entry.
    path: String,
    /// The version the content is of, as hex.
    version: String,
}

impl WriteFetchEntry {
    /// The answer one fetched target is handed over as.
    fn of(entry: &FetchWithEntry) -> Self {
        Self {
            target: entry.target.clone(),
            path: entry.path.clone(),
            version: entry.version.clone(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteFetchEntry {
    /// What it was fetched by.
    #[must_use]
    pub fn target(&self) -> String {
        self.target.clone()
    }

    /// The path it is at, or `.` when it is at none or is not an entry.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The version the content is of, as hex.
    #[must_use]
    pub fn version(&self) -> String {
        self.version.clone()
    }
}

/// How many objects one Vault handed over.
///
/// # FFI
///
/// A `RolaWriteFetchSource *` the caller releases with `free_rola_write_fetch_source`. `_vault`
/// returns a `char *` the caller releases with `free_string`, and `_count` the objects it handed
/// over.
#[lazyffi(export = RolaWriteFetchSource)]
#[derive(Debug, Clone, Default)]
pub struct WriteFetchSource {
    /// The Vault's name.
    vault: String,
    /// The objects it handed over.
    count: u64,
}

impl WriteFetchSource {
    /// The answer one Vault's take is handed over as.
    fn of(source: &FetchWithSource) -> Self {
        Self {
            vault: source.vault.clone(),
            count: u64::try_from(source.count).unwrap_or(u64::MAX),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteFetchSource {
    /// The Vault's name.
    #[must_use]
    pub fn vault(&self) -> String {
        self.vault.clone()
    }

    /// The objects it handed over.
    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }
}

impl From<FetchWithRefusal> for WriteError {
    /// The one failure a caller outside Rust is told, for each refusal this side words.
    ///
    /// The command line branches on the refusal itself and says each one in its own words; what
    /// crosses the ABI is one error with a tag, because a C caller has one thing to do about a fetch
    /// that failed and it does not depend on which of these it was.
    fn from(refusal: FetchWithRefusal) -> Self {
        match refusal {
            FetchWithRefusal::Cancelled => Self::Cancelled,
            FetchWithRefusal::NoLayout => Self::NoLayout,
            FetchWithRefusal::NotCached { .. } => Self::NotCached,
            FetchWithRefusal::LayoutName => Self::Precondition {
                what: "the Layout".to_owned(),
                reason: "no Layout is known under that name".to_owned(),
            },
            FetchWithRefusal::LayoutFailed { reason } => Self::Layout { reason },
            FetchWithRefusal::Hash { hash, miss } => Self::Precondition {
                what: format!("the hash `{hash}`"),
                reason: match miss {
                    HashMiss::Unknown => "it names no variant or version".to_owned(),
                    HashMiss::Ambiguous(candidates) => {
                        format!("it names more than one object: {}", candidates.join(", "))
                    }
                },
            },
            FetchWithRefusal::Chain { target, reason } => Self::Index {
                reason: format!("{target}: {reason}"),
            },
            FetchWithRefusal::Incomplete { entries } => Self::NotFound {
                what: entries
                    .iter()
                    .map(|entry| entry.target.clone())
                    .collect::<Vec<String>>()
                    .join(", "),
            },
            FetchWithRefusal::Action(error) => Self::Action {
                reason: error.to_string(),
            },
            FetchWithRefusal::Failed(error) => error,
        }
    }
}

/// What one word of the request named, once it is resolved.
#[derive(Debug)]
enum Named {
    /// A Layout entry, at the version it records.
    Entry {
        /// The `Uuid` it is known by.
        id: Uuid,
        /// The path it is at.
        path: Option<LayoutPath>,
        /// The version it records.
        version: [u8; 32],
        /// Whether it names a file or a tree.
        kind: EntryKind,
    },
    /// A version, named by its hash.
    Version {
        /// The version's key.
        key: Key,
    },
    /// A variant, named by its hash, and what it points at.
    Variant {
        /// The variant's key.
        key: Key,
        /// The content its `storage` field names.
        storage: Key,
        /// The version that fixes it, whether or not that version was written.
        version: Key,
        /// The version it is based on, which is what a recursive run walks back.
        base: [u8; 32],
    },
}

/// One target and the store keys its content is made of.
struct Wanted {
    /// What was named.
    target: Named,
    /// The keys to fetch.
    keys: Vec<Key>,
    /// Whether its content may be a tree structure, which is what a second exchange is for.
    expand: bool,
}

/// Whether `word` reads as an index object's name rather than as a path of a Layout.
///
/// A whole key and the head of one are both hashes; a path spelled entirely in hex digits of the
/// right length is read as a hash, which is the price of one argument list carrying both, the same
/// price `rola vcs-index` pays.
fn reads_as_hash(word: &str) -> bool {
    Key::from_str(word).is_ok() || ShortHash::new(word).is_ok()
}

/// The entries `words` name, in path order, each with the version it is at.
///
/// A word that reads as a `Uuid` names that entry wherever it is at. Any other word is read as a
/// path: on disk against `cwd` and inside the Workspace `root` when the words are the Workspace's
/// own, and as a path of the Layout in every case. Both readings are tried, so a run made at the
/// root reads `a/b` the same way whichever it meant.
///
/// What a reading names is the entry at that path and every entry under it, so a directory word
/// takes its files and a file word takes itself. A word that names nothing is left out.
fn resolve_words(
    layout: &Layout,
    root: &Path,
    cwd: &Path,
    remote: bool,
    words: &[String],
) -> Vec<Named> {
    let mut found: BTreeMap<(String, Uuid), Named> = BTreeMap::new();

    for word in words {
        if let Ok(id) = Uuid::parse_str(word.trim())
            && let Some(data) = layout.entry(id)
        {
            let path = layout.path_of(id);
            found.insert(
                (shown_path_of(path.as_ref()), id),
                Named::Entry {
                    id,
                    path,
                    version: data.version(),
                    kind: data.kind(),
                },
            );

            continue;
        }

        let mut paths: Vec<LayoutPath> = Vec::new();
        if !remote && let Some(path) = from_disk(root, cwd, word) {
            paths.push(path);
        }
        if let Ok(path) = LayoutPath::new(word)
            && !paths.contains(&path)
        {
            paths.push(path);
        }

        for path in paths {
            let prefix = path.as_str().to_owned();

            if let Some(id) = layout.id_of(&path)
                && let Some(data) = layout.entry(id)
            {
                let shown = layout.path_of(id);
                found.insert(
                    (shown_path_of(shown.as_ref()), id),
                    Named::Entry {
                        id,
                        path: shown,
                        version: data.version(),
                        kind: data.kind(),
                    },
                );
            }

            for (entry_path, id) in layout.paths() {
                if under(&prefix, &entry_path)
                    && let Some(data) = layout.entry(id)
                {
                    found.insert(
                        (entry_path.as_str().to_owned(), id),
                        Named::Entry {
                            id,
                            path: Some(entry_path),
                            version: data.version(),
                            kind: data.kind(),
                        },
                    );
                }
            }
        }
    }

    found.into_values().collect()
}

/// The object `word` names, when it is a whole key or the head of one.
///
/// A whole key is classified by what the index holds under it; the head of one is resolved among
/// the variants and versions the index holds, which is the set a graph and a retrack name.
fn resolve_hash(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    word: &str,
) -> Result<Named, FetchWithRefusal> {
    let key = if let Ok(key) = Key::from_str(word) {
        key
    } else {
        let miss = || FetchWithRefusal::Hash {
            hash: word.to_owned(),
            miss: HashMiss::Unknown,
        };
        let head = ShortHash::new(word).map_err(|_| miss())?;
        let keys = chain_keys(index, runtime)?;

        match head.resolve(keys.iter()) {
            Resolved::ExactlyOne(key) => key,
            Resolved::NoMatch => return Err(miss()),
            Resolved::Ambiguous(keys) => {
                return Err(FetchWithRefusal::Hash {
                    hash: word.to_owned(),
                    miss: HashMiss::Ambiguous(keys.iter().map(Key::hex).collect()),
                });
            }
        }
    };

    let kind = runtime.block_on(index.kind_of(&key)).map_err(|error| {
        FetchWithRefusal::Failed(WriteError::Index {
            reason: error.to_string(),
        })
    })?;

    match kind {
        Some(VCSIndexKind::Variant) => {
            let object = runtime.block_on(index.read(key)).map_err(|error| {
                FetchWithRefusal::Failed(WriteError::Index {
                    reason: error.reason(),
                })
            })?;
            let VCSIndexObject::Variant(variant) = object else {
                return Err(FetchWithRefusal::Hash {
                    hash: word.to_owned(),
                    miss: HashMiss::Unknown,
                });
            };

            Ok(Named::Variant {
                key,
                storage: Key::new(*variant.storage_hash()),
                version: Key::new(variant.new_version().compute_blake3()),
                base: *variant.base_version(),
            })
        }
        Some(VCSIndexKind::Version) => Ok(Named::Version { key }),
        _ => Err(FetchWithRefusal::Hash {
            hash: word.to_owned(),
            miss: HashMiss::Unknown,
        }),
    }
}

/// The keys of the variants and versions the index holds, which is what a head is resolved among.
fn chain_keys(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
) -> Result<Vec<Key>, FetchWithRefusal> {
    let objects = runtime.block_on(index.read_objects()).map_err(|error| {
        FetchWithRefusal::Failed(WriteError::Index {
            reason: error.reason(),
        })
    })?;

    Ok(objects
        .into_iter()
        .filter(|(_, object)| {
            matches!(
                object,
                VCSIndexObject::Variant(_) | VCSIndexObject::Version(_)
            )
        })
        .map(|(key, _)| key)
        .collect())
}

/// Whether any version the path words name is not in the index.
fn absent_versions(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    named: &[Named],
) -> Result<bool, FetchWithRefusal> {
    let versions: Vec<Key> = named
        .iter()
        .filter_map(|target| match target {
            Named::Entry { version, .. } => Some(Key::new(*version)),
            Named::Version { key } => Some(*key),
            Named::Variant { .. } => None,
        })
        .collect();

    if versions.is_empty() {
        return Ok(false);
    }

    let presence = runtime
        .block_on(index.contains_keys(&versions))
        .map_err(|error| {
            FetchWithRefusal::Failed(WriteError::Index {
                reason: error.to_string(),
            })
        })?;

    Ok((0..versions.len()).any(|at| !presence.held(at)))
}

/// Whether the version an entry or a version target names is in the index.
fn version_held(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    target: &Named,
) -> Result<bool, FetchWithRefusal> {
    let version = match target {
        Named::Entry { version, .. } => Key::new(*version),
        Named::Version { key } => *key,
        Named::Variant { .. } => return Ok(true),
    };

    let presence = runtime
        .block_on(index.contains_keys(&[version]))
        .map_err(|error| {
            FetchWithRefusal::Failed(WriteError::Index {
                reason: error.to_string(),
            })
        })?;

    Ok(presence.held(0))
}

/// The store keys the version `version` names, and for a recursive run the chain it stands on.
fn version_keys(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    version: &[u8; 32],
    recursive: bool,
) -> Result<Vec<Key>, String> {
    if recursive {
        runtime.block_on(chain_store_keys(index, version, None, false))
    } else {
        Ok(runtime
            .block_on(store_of(index, version))?
            .into_iter()
            .collect())
    }
}

/// Whether a target's content may be a tree structure.
///
/// An entry says which kind it is, so a file is never read as one. A version or a variant says
/// nothing of the kind, and what it names is tried, which is what the status reading does too.
fn expands(target: &Named) -> bool {
    match target {
        Named::Entry { kind, .. } => *kind == EntryKind::Tree,
        Named::Version { .. } | Named::Variant { .. } => true,
    }
}

/// What `target`'s content names, when it is a structure the store holds.
fn structure_keys(
    store: &StorageSet,
    runtime: &tokio::runtime::Runtime,
    target: &Wanted,
) -> Vec<Key> {
    if !target.expand {
        return Vec::new();
    }

    let mut keys = Vec::new();
    for key in &target.keys {
        let Ok(bytes) = runtime.block_on(store.read_object(key)) else {
            continue;
        };
        let Ok(structure) = TreeStructure::decode(&bytes) else {
            continue;
        };

        keys.extend(structure.content_keys());
    }

    keys
}

/// What a target is named by, for a report and a failure.
fn target_name(target: &Named) -> String {
    match target {
        Named::Entry { id, .. } => id.to_string(),
        Named::Version { key } | Named::Variant { key, .. } => key.hex(),
    }
}

/// The version a target's content is of, as hex.
fn target_version(target: &Named) -> String {
    match target {
        Named::Entry { version, .. } => Key::new(*version).hex(),
        Named::Version { key } => key.hex(),
        Named::Variant { version, .. } => version.hex(),
    }
}

/// The path a target is at, `.` being a target at none.
fn shown_path(target: &Named) -> String {
    match target {
        Named::Entry { path, .. } => shown_path_of(path.as_ref()),
        Named::Version { .. } | Named::Variant { .. } => ".".to_owned(),
    }
}

/// The path an entry is shown at, `.` being an entry at none.
fn shown_path_of(path: Option<&LayoutPath>) -> String {
    path.map_or_else(
        || ".".to_owned(),
        |path| {
            if path.is_root() {
                ".".to_owned()
            } else {
                path.as_str().to_owned()
            }
        },
    )
}

/// The path `word` names on disk, as a path of the Layout, when it is inside the Workspace.
///
/// A word is read against where the run was made, the way `rola track` reads a name. Nothing is
/// named for a path outside the root, and a path that is not UTF-8 names nothing either: a Layout's
/// paths are text.
fn from_disk(root: &Path, cwd: &Path, word: &str) -> Option<LayoutPath> {
    let named = if Path::new(word).is_absolute() {
        PathBuf::from(word)
    } else {
        cwd.join(word)
    };

    let named = normalize(&named);
    let relative = named.strip_prefix(normalize(root)).ok()?;

    LayoutPath::from_relative(relative).ok()
}

/// Whether `name` is a copy of a Vault's Layout, written `NAME@VAULT`.
fn is_remote(name: &str) -> bool {
    name.trim()
        .split_once('@')
        .is_some_and(|(layout, vault)| !layout.is_empty() && !vault.is_empty())
}

/// Whether `path` is `prefix`'s path or under it.
fn under(prefix: &str, path: &LayoutPath) -> bool {
    path.as_str() == prefix
        || path
            .as_str()
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `keys` in one order with each key once, which is what a transfer is given.
fn sort_dedup(keys: &mut Vec<Key>) {
    keys.sort_unstable();
    keys.dedup();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use rorolala_layout::{EntryKind, Layout, LayoutPath, MutableData};
    use uuid::Uuid;

    use super::{reads_as_hash, resolve_words};

    /// The `Uuid`s of the entries the tests lay down.
    const FIRST: Uuid = Uuid::from_u128(0x11);
    const SECOND: Uuid = Uuid::from_u128(0x12);
    const THIRD: Uuid = Uuid::from_u128(0x13);
    const TREE: Uuid = Uuid::from_u128(0x14);

    /// A Layout holding a file, a directory of two files, and the directory as a tree entry.
    fn scratch() -> (PathBuf, Layout) {
        let dir = std::env::temp_dir().join(format!(
            "rola-fetch-with-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");

        let layout = Layout::open(&dir).expect("a Layout");
        for (id, path) in [
            (FIRST, "a.psd"),
            (SECOND, "assets/b.psd"),
            (THIRD, "assets/c.psd"),
        ] {
            layout
                .create_entry(id, MutableData::new(None, [7; 32], String::new()))
                .expect("an entry");
            layout
                .create_path(&LayoutPath::new(path).unwrap(), id)
                .expect("a path");
        }
        layout
            .create_entry(
                TREE,
                MutableData::new(None, [8; 32], String::new()).with_kind(EntryKind::Tree),
            )
            .expect("a tree entry");
        layout
            .create_path(&LayoutPath::new("assets").unwrap(), TREE)
            .expect("a tree path");

        (dir, layout)
    }

    /// The ids `words` resolve to, in one order, for the assertions below.
    fn ids(layout: &Layout, root: &Path, remote: bool, words: &[&str]) -> Vec<Uuid> {
        let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        let mut ids: Vec<Uuid> = resolve_words(layout, root, root, remote, &words)
            .into_iter()
            .map(|target| match target {
                super::Named::Entry { id, .. } => id,
                super::Named::Version { .. } | super::Named::Variant { .. } => {
                    panic!("a path never resolves to a hash")
                }
            })
            .collect();
        ids.sort_unstable();

        ids
    }

    #[test]
    fn a_file_word_names_the_entry_at_it() {
        let (dir, layout) = scratch();

        assert_eq!(ids(&layout, &dir, false, &["a.psd"]), vec![FIRST]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_word_names_what_is_at_it_and_under_it() {
        let (dir, layout) = scratch();
        let mut wanted = vec![TREE, SECOND, THIRD];
        wanted.sort_unstable();

        assert_eq!(ids(&layout, &dir, false, &["assets"]), wanted);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_uuid_names_its_entry_wherever_it_is() {
        let (dir, layout) = scratch();

        assert_eq!(
            ids(&layout, &dir, false, &[&SECOND.to_string()]),
            vec![SECOND]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_word_that_names_nothing_is_passed_over() {
        let (dir, layout) = scratch();

        assert_eq!(ids(&layout, &dir, false, &["not-there.psd"]).len(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_on_disk_is_read_from_where_the_run_was_made() {
        let (dir, layout) = scratch();
        let absolute = dir.join("assets").join("b.psd");

        assert_eq!(
            ids(&layout, &dir, false, &[&absolute.display().to_string()]),
            vec![SECOND]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_of_a_vault_layout_is_never_read_from_disk() {
        let (dir, layout) = scratch();
        let mut wanted = vec![TREE, SECOND, THIRD];
        wanted.sort_unstable();

        // A name reaching out of the Workspace names nothing, since the Layout's own reading
        // refuses a step past its root.
        assert_eq!(ids(&layout, &dir, true, &["../outside"]).len(), 0);
        assert_eq!(ids(&layout, &dir, true, &["assets"]), wanted);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hash_is_a_whole_key_or_the_head_of_one() {
        let key = "0f".repeat(32);

        assert!(reads_as_hash(&key));
        assert!(reads_as_hash(&format!("blake3:{key}")));
        assert!(reads_as_hash(&key[..7]));
        assert!(!reads_as_hash("a.psd"));
        assert!(!reads_as_hash("not-there.psd"));
    }

    #[test]
    fn a_tree_entry_is_marked_as_a_tree() {
        let (dir, layout) = scratch();
        let words = vec!["assets".to_owned()];

        let tree = resolve_words(&layout, &dir, &dir, false, &words)
            .into_iter()
            .find(|target| matches!(target, super::Named::Entry { id, .. } if *id == TREE))
            .expect("the tree entry");

        assert!(matches!(
            tree,
            super::Named::Entry {
                kind: EntryKind::Tree,
                ..
            }
        ));

        let _ = fs::remove_dir_all(&dir);
    }
}

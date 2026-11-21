//! `fetch --with`: bringing the content of the entries a Layout names.
//!
//! A fetch brings a Vault's Layout here, and that copy names versions: which path names which
//! `Uuid`, and what version each entry is at. What a version means is content, and the content is
//! in a store, which may be elsewhere. This is the fetch that closes that gap: it takes the words a
//! run names, reads them as entries of a Layout, and brings the content of the versions those
//! entries are at, so each one can be produced here.
//!
//! What it does not do is fetch the Layout. A copy is what a run reads words from, and it is the one
//! `rola fetch` or `rola layout fetch` has just written: a caller outside the command line, the C
//! ABI among them, fetches the copy first and calls this after it.
//!
//! The index is what turns a version into content, so it is what is read before anything is asked
//! for. It is light, and it is moved whole only when a version the words name is not in the local
//! one, which is the usual state right after a fetch of a Layout whose versions this side never had.
//!
//! How far a version goes is the run's choice. A shallow fetch brings the version each entry is at,
//! which is what producing it needs. A recursive fetch follows each version back to its root and
//! brings the whole chain, which is what a run that may move a pointer back needs. The line walked
//! is the one `base_version` draws, the same one an upload walks, and a merge's joined side is not
//! part of it.
//!
//! A tree's version names a structure rather than bytes, and the structure is what says what the
//! inside is made of. So a shallow fetch of a tree is two exchanges: the structure, and then what
//! its entries hold. That is the same order `bring` reads one in, and for the same reason.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rorolala_layout::{Layout, LayoutPath};
use rorolala_storage::StorageBackend as _;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_utils_progress::Progress;
use rorolala_workspace::Workspace;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon::action_sync_index_all_async;
use crate::fetch::Sources;
use crate::layout_read::{self, LayoutReadError};
use crate::protocol::ActionError;
use crate::storage::Key;
use crate::tree::TreeStructure;
use crate::write::progress::{WriteProgress, watching};
use crate::write::request::Common;
use crate::write::sync::{chain_store_keys, store_of};
use crate::write::{Cancellation, WriteError};

/// What a fetch of named entries' content is asked for.
///
/// The words are read in the Layout `layout` names: empty or `main` is the one the Workspace works
/// in, `truth` a Vault's own, and `NAME@VAULT` the copy a fetch left. A name with an `@` in it is
/// read as paths of that Layout and never as paths on disk, since what it names is elsewhere and may
/// be nowhere in this tree.
#[derive(Debug, Deserialize)]
pub struct FetchWithRequest {
    /// The fields every write request carries.
    #[serde(flatten)]
    pub common: Common,
    /// The Layout the words are read in, empty for the one being worked in.
    #[serde(default)]
    pub layout: String,
    /// The paths, directories or `Uuid`s whose entries come.
    #[serde(default)]
    pub words: Vec<String>,
    /// Whether each version comes with its whole chain or alone.
    #[serde(default)]
    pub recursive: bool,
}

/// One entry whose content was fetched.
#[derive(Debug, Serialize)]
pub struct FetchWithEntry {
    /// The `Uuid` it is known by.
    pub uuid: String,
    /// The path it is at, `.` when it is at none.
    pub path: String,
    /// The version it is at, as hex.
    pub version: String,
}

/// One entry whose content is still not here.
#[derive(Debug, Clone, Serialize)]
pub struct FetchWithMissing {
    /// The `Uuid` it is known by.
    pub uuid: String,
    /// The path it is at, `.` when it is at none.
    pub path: String,
}

/// How many objects one Vault handed over.
#[derive(Debug, Serialize)]
pub struct FetchWithSource {
    /// The Vault's name.
    pub vault: String,
    /// The objects it handed over.
    pub count: usize,
}

/// What a fetch of content came to.
#[derive(Debug, Serialize)]
pub struct FetchWithReport {
    /// The entries whose content was fetched, in path order.
    pub entries: Vec<FetchWithEntry>,
    /// Each Vault that handed something over.
    pub brought: Vec<FetchWithSource>,
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
    /// The index holds no chain for an entry the words name.
    Chain {
        /// The entry the chain was wanted for.
        uuid: String,
        /// Why no chain could be read.
        reason: String,
    },
    /// The content of these entries is still not here.
    Incomplete {
        /// The entries still missing content.
        entries: Vec<FetchWithMissing>,
    },
    /// The exchange with the Vault failed.
    Action(ActionError),
    /// A Workspace, a store, an index or a configuration failed in its own way.
    Failed(WriteError),
}

impl FetchWithRequest {
    /// Brings the content of the entries `words` name out of the Vaults, so each version can be
    /// produced here.
    ///
    /// The Layout named is read where it is, the index is moved whole when a version it must name is
    /// not here, and the content is asked of whichever Vaults hold it. A word that names no entry
    /// and no directory holding one is passed over: a directory may hold files that were never
    /// recorded, and those are exactly the words that name nothing yet.
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
    /// cannot be read, [`FetchWithRefusal::Chain`] when the index holds no chain for an entry,
    /// [`FetchWithRefusal::Action`] when moving the index failed,
    /// [`FetchWithRefusal::Incomplete`] when the content of an entry did not arrive, and
    /// [`FetchWithRefusal::Failed`] for a Workspace, a store, an index or a configuration that
    /// failed otherwise.
    #[allow(
        clippy::too_many_lines,
        reason = "the run reads a Layout, moves an index, gathers keys, fetches them in two exchanges and checks every entry, in one order a caller has to be able to follow"
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
        let layout = self.layout_of(&workspace)?;
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

        // A name that reads against where the run was made is the reading a person wrote it for; a
        // process with no directory of its own reads the words as paths of the Layout instead.
        let root = workspace.get_root();
        let cwd = std::env::current_dir().unwrap_or_else(|_| root.to_path_buf());
        let resolved = resolve_words(&layout, root, &cwd, is_remote(&self.layout), &self.words);

        // Nothing named is nothing to fetch: a run that named only files never recorded is the same
        // as a run that named none at all.
        if resolved.is_empty() {
            return Ok(FetchWithReport {
                entries: Vec::new(),
                brought: Vec::new(),
            });
        }

        // A version is a name until the index says what it stored, so the versions the words name
        // have to be here before anything is asked for. The index is light, so moving it whole is
        // cheaper than the round of questions asking for one object at a time would cost.
        let versions: Vec<Key> = resolved
            .iter()
            .map(|(_, _, version)| Key::new(*version))
            .collect();

        let presence = runtime
            .block_on(index.contains_keys(&versions))
            .map_err(|error| index_unread(&error))?;

        if !(0..versions.len()).all(|at| presence.held(at)) {
            runtime
                .block_on(action_sync_index_all_async(
                    &workspace,
                    &account,
                    reached.address.to_string(),
                    String::new(),
                    progress.clone(),
                ))
                .map_err(FetchWithRefusal::Action)?;

            let after = runtime
                .block_on(index.contains_keys(&versions))
                .map_err(|error| index_unread(&error))?;

            for (at, (id, _, _)) in resolved.iter().enumerate() {
                if !after.held(at) {
                    return Err(FetchWithRefusal::Chain {
                        uuid: id.to_string(),
                        reason: "the index holds no version for this entry".to_owned(),
                    });
                }
            }
        }

        if cancellation.is_cancelled() {
            return Err(FetchWithRefusal::Cancelled);
        }

        // What each entry's version names, and for a recursive run the whole line it stands on.
        let mut per_entry: Vec<Vec<Key>> = Vec::with_capacity(resolved.len());
        for (id, _, version) in &resolved {
            let keys = if self.recursive {
                runtime
                    .block_on(chain_store_keys(&index, version, None, false))
                    .map_err(|reason| FetchWithRefusal::Chain {
                        uuid: id.to_string(),
                        reason,
                    })?
            } else {
                runtime
                    .block_on(store_of(&index, version))
                    .map_err(|reason| FetchWithRefusal::Chain {
                        uuid: id.to_string(),
                        reason,
                    })?
                    .into_iter()
                    .collect()
            };

            if keys.is_empty() {
                return Err(FetchWithRefusal::Chain {
                    uuid: id.to_string(),
                    reason: "the index holds no content for this entry".to_owned(),
                });
            }

            per_entry.push(keys);
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

        // The first exchange brings every version's own content. What a key names is expanded by the
        // side holding it, so a content kept as chunks crosses with its manifest in one pass.
        let mut wanted: Vec<Key> = per_entry.iter().flatten().copied().collect();
        sort_dedup(&mut wanted);
        let _ = sources.bring(&store, &wanted);

        if cancellation.is_cancelled() {
            return Err(FetchWithRefusal::Cancelled);
        }

        // A structure is one version's inside, and what its entries hold is a second ask because
        // the structure is what names them. A key that is not a structure is a file's bytes, and
        // decode refuses it, so the walk leaves it alone.
        let mut contents: Vec<Key> = Vec::new();
        for key in &wanted {
            let Ok(bytes) = runtime.block_on(store.read_object(key)) else {
                continue;
            };
            let Ok(structure) = TreeStructure::decode(&bytes) else {
                continue;
            };

            contents.extend(structure.content_keys());
        }
        sort_dedup(&mut contents);
        let _ = sources.bring(&store, &contents);

        // What did not arrive is said per entry rather than per key: a caller branches on the entry
        // it asked for, and the keys behind it are this side's business.
        let mut missing: Vec<FetchWithMissing> = Vec::new();
        for (at, (id, path, _)) in resolved.iter().enumerate() {
            let mut keys = per_entry[at].clone();
            for key in &per_entry[at] {
                let Ok(bytes) = runtime.block_on(store.read_object(key)) else {
                    continue;
                };
                let Ok(structure) = TreeStructure::decode(&bytes) else {
                    continue;
                };

                keys.extend(structure.content_keys());
            }
            sort_dedup(&mut keys);

            // A store that cannot answer is read as holding nothing, the way the fetch itself reads
            // one: what is not here is what is to be asked for.
            let held = runtime
                .block_on(store.contains_keys(&keys))
                .is_ok_and(|presence| (0..keys.len()).all(|index| presence.held(index)));

            if !held {
                missing.push(FetchWithMissing {
                    uuid: id.to_string(),
                    path: shown_path(path.as_ref()),
                });
            }
        }

        if !missing.is_empty() {
            return Err(FetchWithRefusal::Incomplete { entries: missing });
        }

        let entries = resolved
            .iter()
            .map(|(id, path, version)| FetchWithEntry {
                uuid: id.to_string(),
                path: shown_path(path.as_ref()),
                version: Key::new(*version).hex(),
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

/// The content fetch a JSON request asks for.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, or a `RolaWriteError *` the caller
/// releases with `free_rola_write_error`. `progress` is the handle the caller polls with
/// `rola_progress_next_json`, and `cancellation` is the flag it sets with
/// `rola_cancellation_cancel`.
///
/// The answer is `{"entries":[{"uuid","path","version"}],"brought":[{"vault","count"}]}`: the
/// entries whose content is here, in path order, and how many objects each Vault handed over. Its
/// document is the fields every write request carries (`workspace`, `account`, `target`, `offline`,
/// `force`, `concurrency` and `labels`) at the top level, `target` written the way the other write
/// requests write one, `layout` as the name `rola layout` commands take, `words` as an array of
/// strings, and `recursive` as a bool.
///
/// The copy of a Vault's Layout a name reads is the one a fetch left, so a caller outside the
/// command line fetches that copy first, with `do_action_fetch_layout`, and calls this after it.
///
/// # Errors
///
/// Returns [`WriteError::Json`] when `request` does not read as a [`FetchWithRequest`] or the
/// answer cannot be written, and what [`FetchWithRequest::fetch_with`] names, folded into
/// [`WriteError`].
#[lazyffi(export = rola_write_fetch_with)]
pub fn fetch_with_at(
    request: &str,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<String, WriteError> {
    let asked: FetchWithRequest =
        serde_json::from_str(request).map_err(|error| WriteError::Json {
            reason: error.to_string(),
        })?;
    let watched = watching(progress);
    let report = asked.fetch_with(&watched, cancellation)?;

    serde_json::to_string(&report).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
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
            FetchWithRefusal::Chain { uuid, reason } => Self::Index {
                reason: format!("{uuid}: {reason}"),
            },
            FetchWithRefusal::Incomplete { entries } => Self::NotFound {
                what: entries
                    .iter()
                    .map(|entry| entry.uuid.clone())
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

/// An index that could not be read, in the words a caller reads.
fn index_unread(error: &rorolala_vcs::VCSIndexError) -> FetchWithRefusal {
    FetchWithRefusal::Failed(WriteError::Index {
        reason: error.to_string(),
    })
}

/// An entry the words named: the `Uuid`, where it is, and the version it is at.
type Resolved = (Uuid, Option<LayoutPath>, [u8; 32]);

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
) -> Vec<Resolved> {
    let mut found: BTreeMap<(String, Uuid), Resolved> = BTreeMap::new();

    for word in words {
        if let Ok(id) = Uuid::parse_str(word.trim())
            && let Some(data) = layout.entry(id)
        {
            let path = layout.path_of(id);
            found.insert((shown_path(path.as_ref()), id), (id, path, data.version()));

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
                    (shown_path(shown.as_ref()), id),
                    (id, shown, data.version()),
                );
            }

            for (entry_path, id) in layout.paths() {
                if under(&prefix, &entry_path)
                    && let Some(data) = layout.entry(id)
                {
                    found.insert(
                        (entry_path.as_str().to_owned(), id),
                        (id, Some(entry_path), data.version()),
                    );
                }
            }
        }
    }

    found.into_values().collect()
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

/// The path an entry is shown at, `.` being an entry at none.
fn shown_path(path: Option<&LayoutPath>) -> String {
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

/// `keys` in one order with each key once, which is what a transfer is given.
fn sort_dedup(keys: &mut Vec<Key>) {
    keys.sort_unstable();
    keys.dedup();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use uuid::Uuid;

    use super::resolve_words;

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
            .create_entry(TREE, MutableData::new(None, [8; 32], String::new()))
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
            .map(|(id, _, _)| id)
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
}

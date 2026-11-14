//! Whether the content a variant, a version or a Layout entry points at is stored here.
//!
//! A version names content, and the name is only usable while what it names is here: a retrack to a
//! version, a restore of an entry, and a checkin against it all read the variant the version fixes
//! and the content the variant's `storage` field names. This reading answers whether that chain ends
//! in content this store holds: the version object, the variant object, and the content itself, all
//! of its chunks included.
//!
//! A tree is content too, and its structure names the content of every file it holds. A structure
//! whose files are gone puts nothing back, so a content that begins a tree structure is expanded and
//! the answer then covers the structure, its description and every file it names. Content that
//! begins no structure is one file, and its own key is the whole of what it needs.
//!
//! Nothing here reaches the network and nothing here changes anything: which Vault could provide
//! content that is missing is a caller's question, asked over the exchange rather than here.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr as _;

use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_storage::{Key, RorolalaStorage, StorageBackend as _, TransferableBackend as _};
use rorolala_tree::TreeStructure;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{VCSIndex, VCSIndexObject, Version};
use rorolala_workspace::Workspace;
use serde::{Deserialize, Serialize};

/// What an existence check asks about.
///
/// The three the command line and the C ABI name are a variant, a version and a Layout entry; a
/// stored content and an unidentified key are what a caller that already walked one step of that
/// chain asks about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A variant, by its own key: the content is the one its `storage` field names.
    Variant(Key),
    /// A version, by its own key: the content is the one the variant it fixes names.
    Version(Key),
    /// One entry of a Layout: the content is the one the version it records names.
    Entry(LayoutPath),
    /// Stored content by its own key, which is the key a variant points at.
    Storage(Key),
    /// A key that may name either a version or a variant.
    ///
    /// A run handed a hash does not always know which the hash names: the index may not hold it, or
    /// it may be a name from another index. The reading asks this index which it is when it can, and
    /// answers that the content is not here when it cannot.
    Unknown(Key),
}

/// What one target came to.
#[derive(Debug, Clone, Serialize)]
pub struct Existence {
    /// The version the target names or records, when one was read or named.
    pub version: Option<Key>,
    /// The variant the version fixes, when one was read.
    pub variant: Option<Key>,
    /// The key of the content, when the objects that lead to it were read.
    pub storage: Option<Key>,
    /// Whether every object the target needs is held, the content's chunks included.
    pub held: bool,
    /// The keys that are not held.
    pub missing: Vec<Key>,
    /// Whether the content is a tree structure rather than the content of one file.
    pub tree: bool,
    /// The keys a tree names, when its structure was read and its entries were checked.
    ///
    /// The description's key comes first, when the structure has one. `None` for a file, and for a
    /// tree whose structure could not be read: what a structure that does not read names is not
    /// known.
    pub contents: Option<Vec<Key>>,
}

/// What one target resolved to before the store is asked.
struct Resolved {
    /// The version it names or records, when one was read or named.
    version: Option<Key>,
    /// The variant that version fixes, when one was read.
    variant: Option<Key>,
    /// The key of the content, when the objects that lead to it were read.
    storage: Option<Key>,
    /// What kind of thing the target is, when the Layout said.
    kind: Option<EntryKind>,
    /// The keys that would not read.
    missing: Vec<Key>,
}

impl Resolved {
    /// A target that resolved to nothing.
    const fn empty() -> Self {
        Self {
            version: None,
            variant: None,
            storage: None,
            kind: None,
            missing: Vec::new(),
        }
    }
}

/// Whether each of `targets` is held by `store`, read against `layout` and `index`.
///
/// The answer is one value per target, in the order the targets were given. A target that names
/// nothing this Layout or index holds is answered as not held rather than as a failure: what a
/// caller asked is whether it can be used offline, and a name nothing holds cannot.
///
/// `store` is the Workspace's store when it has one. A Workspace that has never written anything
/// has none, and then every target is answered as not held.
pub async fn local(
    layout: &Layout,
    index: &VCSIndex,
    store: Option<&RorolalaStorage>,
    targets: &[Target],
) -> Vec<Existence> {
    let mut resolved = Vec::with_capacity(targets.len());

    for target in targets {
        resolved.push(resolve(layout, index, target).await);
    }

    // One lookup for every content every target names, rather than one per target: a reading of a
    // version graph asks about hundreds of variants at once, and the store answers a batch. The
    // version and the variant are objects of the index, whose presence the reads above settled.
    let asked: Vec<Key> = resolved
        .iter()
        .filter_map(|resolved| resolved.storage)
        .collect();
    let presence = match store {
        Some(store) => store.contains_keys(&asked).await.ok(),
        None => None,
    };
    let held: BTreeMap<Key, bool> = presence.as_ref().map_or_else(BTreeMap::new, |presence| {
        asked
            .iter()
            .enumerate()
            .map(|(at, key)| (*key, presence.held(at)))
            .collect()
    });

    let mut answers: Vec<Existence> = resolved
        .iter()
        .map(|resolved| {
            let mut missing = resolved.missing.clone();

            if let Some(storage) = resolved.storage
                && !held.get(&storage).copied().unwrap_or(false)
            {
                missing.push(storage);
            }

            Existence {
                version: resolved.version,
                variant: resolved.variant,
                storage: resolved.storage,
                // A root version names no content, and a content nothing could name is not held
                // whatever the store says about the objects on the way.
                held: resolved.storage.is_some() && missing.is_empty(),
                missing,
                tree: resolved.kind == Some(EntryKind::Tree),
                contents: None,
            }
        })
        .collect();

    let Some(store) = store else {
        return answers;
    };

    for (resolved, answer) in resolved.iter().zip(&mut answers) {
        expand(store, resolved, answer).await;
    }

    answers
}

/// Expands a target whose content turned out to be a tree, and folds what its files came to into
/// `answer`.
async fn expand(store: &RorolalaStorage, resolved: &Resolved, answer: &mut Existence) {
    if !answer.held {
        return;
    }

    let Some(storage) = answer.storage else {
        return;
    };

    // A Layout that says the entry is a file is the one kind that needs no look at the content:
    // the entry's kind is written down and does not change over its life.
    let structure = match resolved.kind {
        Some(EntryKind::File) => return,
        Some(EntryKind::Tree) => read_structure(store, storage).await,
        None => sniff_structure(store, storage).await,
    };

    let Some(structure) = structure else {
        // What the Layout records as a tree and the store cannot put back is not content anything
        // can be read from, so it is not held even though its structure key answered.
        if resolved.kind == Some(EntryKind::Tree) {
            answer.held = false;
        }

        return;
    };

    let contents = structure.content_keys();
    let presence = store.contains_keys(&contents).await.ok();
    let missing: Vec<Key> = presence.as_ref().map_or_else(
        || contents.clone(),
        |presence| {
            contents
                .iter()
                .enumerate()
                .filter(|(at, _)| !presence.held(*at))
                .map(|(_, key)| *key)
                .collect()
        },
    );

    answer.tree = true;
    answer.contents = Some(contents);

    if !missing.is_empty() {
        answer.held = false;
        answer.missing.extend(missing);
    }
}

/// Reads what one target names, as far as the Layout, the index and nothing else can say.
async fn resolve(layout: &Layout, index: &VCSIndex, target: &Target) -> Resolved {
    match target {
        Target::Storage(key) => Resolved {
            storage: Some(*key),
            ..Resolved::empty()
        },
        Target::Variant(key) => resolved_variant(index, *key).await,
        Target::Version(key) => resolved_version(index, *key, None).await,
        Target::Unknown(key) => match index.read(*key).await {
            Ok(VCSIndexObject::Version(version)) => from_version(index, *key, &version, None).await,
            Ok(VCSIndexObject::Variant(variant)) => Resolved {
                variant: Some(*key),
                storage: Some(Key::new(*variant.storage_hash())),
                ..Resolved::empty()
            },
            _ => Resolved {
                missing: vec![*key],
                ..Resolved::empty()
            },
        },
        Target::Entry(path) => resolved_entry(layout, index, path).await,
    }
}

/// Reads a variant and the content its `storage` field names.
async fn resolved_variant(index: &VCSIndex, key: Key) -> Resolved {
    let mut resolved = Resolved {
        variant: Some(key),
        ..Resolved::empty()
    };

    match index.read(key).await {
        Ok(VCSIndexObject::Variant(variant)) => {
            resolved.storage = Some(Key::new(*variant.storage_hash()));
        }
        _ => resolved.missing.push(key),
    }

    resolved
}

/// Reads a version, the variant it fixes and the content that variant names.
async fn resolved_version(index: &VCSIndex, key: Key, kind: Option<EntryKind>) -> Resolved {
    match index.read(key).await {
        Ok(VCSIndexObject::Version(version)) => from_version(index, key, &version, kind).await,
        _ => Resolved {
            version: Some(key),
            kind,
            missing: vec![key],
            ..Resolved::empty()
        },
    }
}

/// Reads the variant a version fixes, and the content that variant names.
async fn from_version(
    index: &VCSIndex,
    key: Key,
    version: &Version,
    kind: Option<EntryKind>,
) -> Resolved {
    let mut resolved = Resolved {
        version: Some(key),
        kind,
        ..Resolved::empty()
    };

    // The root is a version that names no variant and no content: every chain starts from it, and
    // nothing can be retracked to it.
    let variant = *version.variant();
    if variant == [0_u8; 32] {
        return resolved;
    }

    let variant_key = Key::new(variant);
    resolved.variant = Some(variant_key);

    match index.read(variant_key).await {
        Ok(VCSIndexObject::Variant(object)) => {
            resolved.storage = Some(Key::new(*object.storage_hash()));
        }
        _ => resolved.missing.push(variant_key),
    }

    resolved
}

/// Reads the version a Layout entry records, and what that version names.
async fn resolved_entry(layout: &Layout, index: &VCSIndex, path: &LayoutPath) -> Resolved {
    let Some(id) = layout.id_of(path) else {
        return Resolved::empty();
    };
    let Some(data) = layout.entry(id) else {
        return Resolved::empty();
    };

    let kind = Some(data.kind());
    if data.version() == [0_u8; 32] {
        return Resolved {
            kind,
            ..Resolved::empty()
        };
    }

    resolved_version(index, Key::new(data.version()), kind).await
}

/// The structure stored under `key`.
async fn read_structure(store: &RorolalaStorage, key: Key) -> Option<TreeStructure> {
    let bytes = store.content(&key).await.ok().flatten()?;

    TreeStructure::decode(&bytes).ok()
}

/// The structure stored under `key`, when what is stored there begins one.
///
/// The opening of a content is what tells a structure from a file, and a content kept cut has its
/// opening in its first chunk. Reading that much and stopping is what keeps a file of any size from
/// being read whole only to learn it is not a structure; a content that begins one is read whole,
/// since its entries are what the answer needs.
async fn sniff_structure(store: &RorolalaStorage, key: Key) -> Option<TreeStructure> {
    let manifest = store.manifest_of(&key).await.ok().flatten();

    // Two or more chunks: the structure's bytes begin in the first of them. One chunk is the whole
    // content, and no manifest is a content kept whole, so both are read as they are.
    if let Some(manifest) = &manifest
        && manifest.len() > 1
        && let Some(chunk) = manifest.chunks().first()
    {
        let head = store.read_object(&chunk.key()).await.ok()?;

        return if TreeStructure::begins(&head) {
            read_structure(store, key).await
        } else {
            None
        };
    }

    read_structure(store, key).await
}

/// One target as a request writes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AskedTarget {
    /// A variant, by its key in hex.
    Variant {
        /// The variant's key, as hex, with or without `blake3:` in front of it.
        hash: String,
    },
    /// A version, by its key in hex.
    Version {
        /// The version's key, as hex, with or without `blake3:` in front of it.
        hash: String,
    },
    /// A Layout entry, by the path it is at.
    Entry {
        /// The path, as the Layout writes it.
        path: String,
    },
}

impl AskedTarget {
    /// The target this names, or why it names none.
    ///
    /// # Errors
    ///
    /// Returns [`ExistenceError::BadRequest`] when the hash is not one this build reads, or the path
    /// is not one a Layout could write.
    pub fn target(&self) -> Result<Target, ExistenceError> {
        match self {
            Self::Variant { hash } => Ok(Target::Variant(key_of(hash)?)),
            Self::Version { hash } => Ok(Target::Version(key_of(hash)?)),
            Self::Entry { path } => {
                LayoutPath::new(path)
                    .map(Target::Entry)
                    .map_err(|_| ExistenceError::BadRequest {
                        reason: format!("`{path}` is not a Layout path"),
                    })
            }
        }
    }

    /// What kind it is, as the answer writes it.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Variant { .. } => "variant",
            Self::Version { .. } => "version",
            Self::Entry { .. } => "entry",
        }
    }

    /// The hash or the path it was written with.
    #[must_use]
    pub fn written(&self) -> &str {
        match self {
            Self::Variant { hash } | Self::Version { hash } => hash,
            Self::Entry { path } => path,
        }
    }
}

/// The key `hash` writes, or the failure of one that does not.
fn key_of(hash: &str) -> Result<Key, ExistenceError> {
    Key::from_str(hash).map_err(|_| ExistenceError::BadRequest {
        reason: format!("`{hash}` is not a key"),
    })
}

/// The request the C ABI carries.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Request {
    /// The targets, in the order the answer keeps.
    targets: Vec<AskedTarget>,
}

/// What the C ABI answers with.
#[derive(Debug, Serialize)]
struct Answer {
    /// One answer per target, in the order they were asked about.
    targets: Vec<AnswerTarget>,
}

/// What one target came to, as the C ABI writes it.
#[derive(Debug, Serialize)]
struct AnswerTarget {
    /// What kind the target was.
    kind: &'static str,
    /// The hash it was written with, for a variant or a version.
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
    /// The path it was written with, for an entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// The version it names or records, as hex.
    version: Option<String>,
    /// The variant that version fixes, as hex.
    variant: Option<String>,
    /// The key of the content, as hex.
    storage: Option<String>,
    /// Whether every object it needs is held, so it can be used offline.
    offline: bool,
    /// Whether its content is a tree structure.
    tree: bool,
    /// The keys that are not held.
    missing: Vec<String>,
}

impl AnswerTarget {
    /// One answer for `asked`, from what the reading came to.
    fn new(asked: &AskedTarget, existence: &Existence) -> Self {
        let (hash, path) = match asked {
            AskedTarget::Variant { hash } | AskedTarget::Version { hash } => {
                (Some(hash.clone()), None)
            }
            AskedTarget::Entry { path } => (None, Some(path.clone())),
        };

        Self {
            kind: asked.kind(),
            hash,
            path,
            version: existence.version.map(|key| key.hex()),
            variant: existence.variant.map(|key| key.hex()),
            storage: existence.storage.map(|key| key.hex()),
            offline: existence.held,
            tree: existence.tree,
            missing: existence.missing.iter().map(Key::hex).collect(),
        }
    }
}

/// Whether the objects a variant, a version or a Layout entry names are stored here.
///
/// The request names the targets, and the answer is one entry per target in the same order. A
/// variant is followed to the content its `storage` field names, a version to the variant it fixes,
/// and a Layout entry to the version it records. An object that is missing on the way is answered as
/// not held, with the key that is missing named beside it; content kept cut is held when every chunk
/// it names is, and a tree is held when its structure, its description and every file it names are.
///
/// # FFI
///
/// `directory` is the directory the Workspace is searched upwards from, and `request` is a UTF-8
/// JSON document of this shape:
///
/// ```json
/// {"targets": [
///   {"kind": "variant", "hash": "blake3:..."},
///   {"kind": "version", "hash": "..."},
///   {"kind": "entry", "path": "art.psd"}
/// ]}
/// ```
///
/// The answer is an owned JSON string the caller releases with `free_string`. It is
/// `{"targets": [...]}`, one answer per target in the order asked, each with `kind`, the `hash` or
/// `path` it was asked about, `version`, `variant`, `storage`, `offline`, `tree` and `missing`. A
/// failure is a `RolaExistenceError *` the caller releases with `free_rola_existence_error`.
///
/// # Errors
///
/// Returns [`ExistenceError::NoWorkspace`] when no Workspace holds `directory`,
/// [`ExistenceError::NoLayout`] when the Workspace works in no Layout,
/// [`ExistenceError::NoIndex`] when it holds no version control index,
/// [`ExistenceError::BadRequest`] when `request` names a target this build cannot read, and
/// [`ExistenceError::Read`] when the reading could not be made.
#[lazyffi(export = rola_can_retrack_offline_json)]
pub fn can_retrack_offline_json(directory: &Path, request: &str) -> Result<String, ExistenceError> {
    let workspace = Workspace::locate(directory).ok_or(ExistenceError::NoWorkspace)?;
    let layout = workspace
        .layout_if_there()
        .map_err(|error| ExistenceError::Read {
            reason: error.to_string(),
        })?
        .ok_or(ExistenceError::NoLayout)?;
    let index = VCSIndex::locate(directory).ok_or(ExistenceError::NoIndex)?;
    let request: Request =
        serde_json::from_str(request).map_err(|error| ExistenceError::BadRequest {
            reason: error.to_string(),
        })?;

    let mut targets = Vec::with_capacity(request.targets.len());
    for asked in &request.targets {
        targets.push(asked.target()?);
    }

    let runtime = tokio::runtime::Runtime::new().map_err(|error| ExistenceError::Read {
        reason: error.to_string(),
    })?;
    let store = workspace.get_current_rola_storage();
    let answers = runtime.block_on(local(&layout, &index, store.as_ref(), &targets));

    serde_json::to_string(&Answer {
        targets: request
            .targets
            .iter()
            .zip(&answers)
            .map(|(asked, existence)| AnswerTarget::new(asked, existence))
            .collect(),
    })
    .map_err(|error| ExistenceError::Read {
        reason: error.to_string(),
    })
}

/// What reading whether a target is held failed with.
#[lazyffi(export = RolaExistenceError)]
#[derive(Debug)]
pub enum ExistenceError {
    /// No Workspace holds the directory the reading was asked of.
    NoWorkspace,
    /// The Workspace works in no Layout.
    NoLayout,
    /// The Workspace holds no version control index.
    NoIndex,
    /// The request does not read as one this build knows.
    BadRequest {
        /// Why it does not.
        reason: String,
    },
    /// The reading, or the answer it was written as, could not be made.
    Read {
        /// Why it could not.
        reason: String,
    },
    /// The account a search was to act as is not here.
    NoAccount {
        /// The account that was named.
        name: String,
    },
    /// No account was named, and the user's current account names none either.
    NoCurrentAccount,
}

impl fmt::Display for ExistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWorkspace => formatter.write_str("no Workspace holds the directory"),
            Self::NoLayout => formatter.write_str("the Workspace works in no Layout"),
            Self::NoIndex => formatter.write_str("the Workspace holds no version control index"),
            Self::BadRequest { reason } => {
                write!(formatter, "the request does not read: {reason}")
            }
            Self::Read { reason } => {
                write!(
                    formatter,
                    "the existence reading could not be made: {reason}"
                )
            }
            Self::NoAccount { name } => {
                write!(formatter, "the account `{name}` is not here")
            }
            Self::NoCurrentAccount => formatter.write_str("no account is named to search as"),
        }
    }
}

impl std::error::Error for ExistenceError {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::{EntryKind, Layout, LayoutPath, MutableData};
    use rorolala_storage::{Codec, Key, RorolalaStorage, StorageBackend as _};
    use rorolala_tree::{FileMode, TreeEntry, TreeStructure};
    use rorolala_utils_location::Locate as _;
    use rorolala_vcs::{
        ComputeBlake3, Creator, Message, VCSIndex, VCSWrite as _, Variant, Version,
    };
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{Target, can_retrack_offline_json, local};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-existence-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A Workspace, its Layout, its index and its store.
    struct Held {
        /// Where it all is.
        dir: PathBuf,
        /// The Layout being worked in.
        layout: Layout,
        /// The version control index.
        index: VCSIndex,
        /// The store content is kept in.
        store: RorolalaStorage,
        /// The runtime the asynchronous readings are waited on.
        runtime: tokio::runtime::Runtime,
    }

    impl Held {
        /// A Workspace with a root version written into its index.
        fn new(label: &str) -> Self {
            let dir = scratch(label);
            Workspace::create(&dir).unwrap();
            let workspace = Workspace::locate(&dir).unwrap();
            let layout = workspace.layout().unwrap();
            let index = VCSIndex::create(dir.join(".rola/index"));
            let store = workspace.get_or_create_rola_storage();
            let runtime = tokio::runtime::Runtime::new().unwrap();

            runtime.block_on(index.write(Version::root())).unwrap();

            Self {
                dir,
                layout,
                index,
                store,
                runtime,
            }
        }

        /// Writes `content` into the store and answers with its key.
        fn content(&self, content: &[u8]) -> Key {
            self.runtime
                .block_on(self.store.write_object(content, Codec::Raw))
                .unwrap()
        }

        /// A variant of `base` holding `storage`, written with its version.
        fn version(&self, base: &Version, storage: Key, seed: u8) -> (Variant, Version) {
            let creator = Creator::try_from(format!("creator {seed}")).unwrap();
            let message = Message::try_from(format!("message {seed}")).unwrap();

            self.runtime
                .block_on(self.index.write(creator.clone()))
                .unwrap();
            self.runtime
                .block_on(self.index.write(message.clone()))
                .unwrap();

            let variant = base.new_variant(
                *storage.digest(),
                creator.compute_blake3(),
                message.compute_blake3(),
            );
            let version = variant.new_version();

            self.runtime
                .block_on(self.index.write(variant.clone()))
                .unwrap();
            self.runtime
                .block_on(self.index.write(version.clone()))
                .unwrap();

            (variant, version)
        }

        /// One entry of the Layout, at `path`, recording `version`.
        fn entry(&self, path: &str, version: &Version, kind: EntryKind) -> LayoutPath {
            let path = LayoutPath::new(path).unwrap();
            let id = Uuid::new_v4();
            let data = MutableData::new(None, *version.hash().digest(), "art").with_kind(kind);

            self.layout.create_entry(id, data).unwrap();
            self.layout.create_path(&path, id).unwrap();

            path
        }

        /// The answers for `targets`.
        fn local(&self, targets: &[Target]) -> Vec<super::Existence> {
            self.runtime
                .block_on(local(&self.layout, &self.index, Some(&self.store), targets))
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    /// A variant and a version whose content is here are held, and one whose content is gone is not.
    #[test]
    fn a_version_is_held_while_its_content_is() {
        let held = Held::new("version");
        let content = held.content(b"the art");
        let (variant, version) = held.version(&Version::root(), content, 1);

        let answers = held.local(&[
            Target::Variant(variant.hash()),
            Target::Version(version.hash()),
        ]);

        assert_eq!(answers.len(), 2);

        for answer in &answers {
            assert!(answer.held, "{answer:?}");
            assert_eq!(answer.storage, Some(content));
            assert!(answer.missing.is_empty(), "{answer:?}");
            assert!(!answer.tree);
        }

        // The version names the variant, and the variant names the content.
        assert_eq!(answers[1].version, Some(version.hash()));
        assert_eq!(answers[1].variant, Some(variant.hash()));

        held.runtime.block_on(held.store.remove(&content)).unwrap();

        let answers = held.local(&[Target::Variant(variant.hash())]);
        assert!(!answers[0].held);
        assert_eq!(answers[0].missing, vec![content]);
    }

    /// A Layout entry is held while the version it records is, and the root records nothing.
    #[test]
    fn an_entry_is_held_while_the_version_it_records_is() {
        let held = Held::new("entry");
        let content = held.content(b"the art");
        let (_variant, version) = held.version(&Version::root(), content, 2);
        let path = held.entry("art.psd", &version, EntryKind::File);

        let answers = held.local(&[Target::Entry(path.clone())]);
        assert!(answers[0].held, "{:?}", answers[0]);
        assert_eq!(answers[0].version, Some(version.hash()));

        held.runtime.block_on(held.store.remove(&content)).unwrap();

        let answers = held.local(&[Target::Entry(path)]);
        assert!(!answers[0].held);
        assert_eq!(answers[0].missing, vec![content]);
    }

    /// A tree is held while its structure and every file it names are, and a file that is gone is
    /// named as what is missing.
    #[test]
    fn a_tree_is_checked_down_to_its_files() {
        let held = Held::new("tree");
        let file = held.content(b"a file in the tree");
        let structure =
            TreeStructure::build(None, [TreeEntry::file("a.psd", FileMode::Regular, file)])
                .unwrap();
        let tree = held.content(&structure.encode());
        let (_variant, version) = held.version(&Version::root(), tree, 3);
        let path = held.entry("art", &version, EntryKind::Tree);

        let answers = held.local(&[Target::Entry(path.clone())]);
        assert!(answers[0].held, "{:?}", answers[0]);
        assert!(answers[0].tree);
        assert_eq!(answers[0].contents, Some(vec![file]));

        held.runtime.block_on(held.store.remove(&file)).unwrap();

        let answers = held.local(&[Target::Entry(path)]);
        assert!(!answers[0].held);
        assert_eq!(answers[0].contents, Some(vec![file]));
        assert_eq!(answers[0].missing, vec![file]);
    }

    /// A tree named by a version rather than by an entry is found by what its content begins.
    #[test]
    fn a_version_of_a_tree_is_expanded_from_its_content() {
        let held = Held::new("sniff");
        let file = held.content(b"a file in the tree");
        let structure =
            TreeStructure::build(None, [TreeEntry::file("a.psd", FileMode::Regular, file)])
                .unwrap();
        let tree = held.content(&structure.encode());
        let (_variant, version) = held.version(&Version::root(), tree, 4);

        let answers = held.local(&[Target::Version(version.hash())]);
        assert!(answers[0].held, "{:?}", answers[0]);
        assert!(answers[0].tree);
        assert_eq!(answers[0].contents, Some(vec![file]));
    }

    /// A key the index does not hold is answered as not held rather than as a failure.
    #[test]
    fn a_key_nothing_holds_is_not_held() {
        let held = Held::new("unknown");
        let key = Key::new([0xab; 32]);

        let answers = held.local(&[Target::Unknown(key)]);
        assert!(!answers[0].held);
        assert_eq!(answers[0].missing, vec![key]);
        assert!(answers[0].storage.is_none());
    }

    /// The C ABI answers in the order it was asked, and says what each target is.
    #[test]
    fn the_c_abi_answers_in_the_order_asked() {
        let held = Held::new("ffi");
        let here = held.content(b"here");
        let (_variant, version) = held.version(&Version::root(), here, 5);
        let there = Key::new([0xcd; 32]);

        let request = format!(
            r#"{{"targets":[
                {{"kind":"version","hash":"{}"}},
                {{"kind":"variant","hash":"{}"}},
                {{"kind":"entry","path":"art.psd"}}
            ]}}"#,
            version.hash().hex(),
            there.hex()
        );
        let answer = can_retrack_offline_json(&held.dir, &request).unwrap();
        let json: serde_json::Value = serde_json::from_str(&answer).unwrap();
        let targets = json["targets"].as_array().unwrap();

        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0]["kind"], "version");
        assert_eq!(targets[0]["offline"], true);
        assert_eq!(targets[1]["kind"], "variant");
        assert_eq!(targets[1]["offline"], false);
        assert_eq!(targets[1]["missing"][0], there.hex());
        assert_eq!(targets[2]["kind"], "entry");
        assert_eq!(targets[2]["path"], "art.psd");
        assert_eq!(targets[2]["offline"], false);

        let refused = can_retrack_offline_json(&held.dir, r#"{"targets":[{"kind":"nope"}]}"#);
        assert!(matches!(
            refused,
            Err(super::ExistenceError::BadRequest { .. })
        ));
    }
}

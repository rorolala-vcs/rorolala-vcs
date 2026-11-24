//! A run's stores, in the order it reaches for them.
//!
//! One [`RorolalaStorage`] is one place, one `rolast.toml` and one set of packs and locks. A run
//! may be told to reach for several of them: a Workspace or a Vault binds the places it shares its
//! content through, and what it writes goes to the first one that is there while what it reads is
//! looked for in each of them in turn. [`StorageSet`] is that order, and keeps the two meanings
//! apart rather than folding several places into one store.
//!
//! The set is built from paths a caller hands in, never from a configuration file: reading a
//! Workspace's or a Vault's configuration is theirs to do, and storage is at the bottom of the crate
//! stack and knows nothing of either.

use std::fs::OpenOptions;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use rorolala_utils_location::{Locate as _, normalize};

use crate::{
    Bytes, Chunking, Codec, Error, Key, Manifest, ObjectBatch, Presence, RorolalaStorage,
    StorageBackend as _, TransferableBackend, store_file,
};

/// The store a set falls back on, opened when first needed.
///
/// A read must not make a store: a status or a listing run in a directory with no `.rola/storage`
/// is a run that has none, not a run that makes one. So the root is kept and the store is opened
/// only when something is written, which is what a [`OnceLock`] allows through `&self` without
/// making the whole call chain mutable.
#[derive(Debug)]
struct OwnStorage {
    /// Where the store is rooted, whether or not it is there yet.
    root: PathBuf,
    /// The store, once it has been opened or made.
    opened: OnceLock<RorolalaStorage>,
    /// Whether the store would accept a write, as the construction pre-check saw it.
    writable: bool,
}

impl OwnStorage {
    /// The store, when it has been opened or made.
    fn opened(&self) -> Option<&RorolalaStorage> {
        self.opened.get()
    }

    /// The store, made where it is not there yet.
    ///
    /// Making a store is best effort, so what comes back is always a store: one that could not be
    /// made fails on the first read or write that needs it, where there is somewhere to report it.
    fn get_or_create(&self) -> &RorolalaStorage {
        self.opened
            .get_or_init(|| RorolalaStorage::create(self.root.clone()))
    }
}

impl Clone for OwnStorage {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            opened: self
                .opened
                .get()
                .cloned()
                .map_or_else(OnceLock::new, OnceLock::from),
            writable: self.writable,
        }
    }
}

/// One store a place bound, as the configuration wrote the path.
#[derive(Debug, Clone)]
pub struct BoundStorage {
    /// The path as the configuration wrote it, which is what a warning names.
    written: String,
    /// The resolved, absolute path the set reads and writes.
    root: PathBuf,
    /// Whether this is the place's own store, written as `.`.
    own: bool,
    /// The opened store, when the directory is one.
    store: Option<RorolalaStorage>,
    /// Whether the store would accept a write, as the construction pre-check saw it.
    writable: bool,
    /// Why the binding is not usable, when it is not.
    reason: Option<String>,
}

impl BoundStorage {
    /// The path as the configuration wrote it.
    #[must_use]
    pub fn written(&self) -> &str {
        &self.written
    }

    /// The resolved, absolute path.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether this is the place's own store.
    #[must_use]
    pub const fn is_own(&self) -> bool {
        self.own
    }

    /// Whether the directory is a store this build can open.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.store.is_some()
    }

    /// Whether the store would accept a write, as the construction pre-check saw it.
    #[must_use]
    pub const fn is_writable(&self) -> bool {
        self.writable
    }

    /// Why the binding is not usable, when it is not.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// Something about a binding a caller may want to say aloud.
///
/// A binding that cannot be used is skipped rather than refusing the run: one path typed wrong is
/// not a reason to leave a Workspace unusable. The note is what a caller says on standard error so
/// the skip is seen rather than silently taken.
#[derive(Debug, Clone)]
pub enum StorageNote {
    /// The directory is missing, not a directory, or carries no `rolast.toml`.
    Unusable {
        /// The path as the configuration wrote it.
        written: String,
        /// The resolved, absolute path.
        root: PathBuf,
        /// Why it cannot be used.
        reason: String,
    },
    /// The store can be read but not written.
    ReadOnly {
        /// The path as the configuration wrote it.
        written: String,
        /// The resolved, absolute path.
        root: PathBuf,
    },
}

impl StorageNote {
    /// The path as the configuration wrote it.
    #[must_use]
    pub fn written(&self) -> &str {
        match self {
            Self::Unusable { written, .. } | Self::ReadOnly { written, .. } => written,
        }
    }

    /// The resolved, absolute path.
    #[must_use]
    pub fn root(&self) -> &Path {
        match self {
            Self::Unusable { root, .. } | Self::ReadOnly { root, .. } => root,
        }
    }

    /// Why the binding cannot be used, for a binding that cannot.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Unusable { reason, .. } => Some(reason),
            Self::ReadOnly { .. } => None,
        }
    }
}

/// What reading a place's store bindings failed with.
///
/// It is the configuration's own trouble rather than a store's: a file that will not read, a path
/// that may not be bound, a directory that is not a store. A caller turns each into its own answer,
/// a configuration that cannot be read, a path that cannot be used, a creation that failed.
#[derive(Debug)]
pub enum StorageConfigError {
    /// The configuration could not be read.
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// Why not.
        reason: String,
    },
    /// The configuration is there but does not read as one.
    Malformed {
        /// The file that does not read.
        path: PathBuf,
        /// Why not.
        reason: String,
    },
    /// A binding names a path that may not be one.
    Forbidden {
        /// The path that was refused.
        path: PathBuf,
        /// Why it may not be bound.
        reason: String,
    },
    /// A binding names a directory that is not a store.
    NotAStore {
        /// The directory that carries no `rolast.toml`.
        path: PathBuf,
    },
    /// A binding names something that is not a directory.
    NotADirectory {
        /// The path that is not a directory.
        path: PathBuf,
    },
}

impl StorageConfigError {
    /// The path the failure is about.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Io { path, .. }
            | Self::Malformed { path, .. }
            | Self::Forbidden { path, .. }
            | Self::NotAStore { path }
            | Self::NotADirectory { path } => path,
        }
    }
}

impl std::fmt::Display for StorageConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, reason } => {
                write!(
                    formatter,
                    "`{}` could not be read: {reason}",
                    path.display()
                )
            }
            Self::Malformed { path, reason } => {
                write!(
                    formatter,
                    "`{}` does not read as one: {reason}",
                    path.display()
                )
            }
            Self::Forbidden { path, reason } => {
                write!(formatter, "`{}` may not be bound: {reason}", path.display())
            }
            Self::NotAStore { path } => {
                write!(formatter, "`{}` is not a store", path.display())
            }
            Self::NotADirectory { path } => {
                write!(formatter, "`{}` is not a directory", path.display())
            }
        }
    }
}

impl std::error::Error for StorageConfigError {}

/// The stores a run works on, in the order it reaches for them.
///
/// The order is the bindings as the configuration lists them, and then the place's own store when
/// `.` is not written among them. Writes go to the first binding that is there, own store
/// included, and never move to another one when that write fails; reads are tried in the same
/// order until one of them answers, because a key names the same content wherever it is kept.
#[derive(Debug, Clone)]
pub struct StorageSet {
    /// The place's own store.
    own: OwnStorage,
    /// The bindings, in the order the configuration lists them.
    bound: Vec<BoundStorage>,
    /// What could not be used, for a caller to say aloud.
    notes: Vec<StorageNote>,
    /// Whether `.` is one of the bindings, so the own store is not added twice.
    own_bound: bool,
}

impl StorageSet {
    /// A set of one store, the place's own.
    #[must_use]
    pub fn sole(own: RorolalaStorage) -> Self {
        let writable = probe_writable(&own);
        let root = own.get_root().to_path_buf();

        Self {
            own: OwnStorage {
                root,
                opened: OnceLock::from(own),
                writable,
            },
            bound: Vec::new(),
            notes: Vec::new(),
            own_bound: false,
        }
    }

    /// A set of the place's own store and the bindings it was given.
    ///
    /// `own` is the store the place already has, or `None` when it has none; a set with none keeps
    /// its root and reads nothing from it until a write makes it. `base` is what a relative binding
    /// is resolved against, and `forbidden` is the place's own data directory, which a binding may
    /// not point inside except at `own_root` itself.
    #[must_use]
    pub fn with_bindings(
        own: Option<RorolalaStorage>,
        own_root: impl Into<PathBuf>,
        base: &Path,
        bindings: impl IntoIterator<Item = String>,
        forbidden: Option<&Path>,
    ) -> Self {
        let own_root = normalize(&own_root.into());
        let base = normalize(base);
        let forbidden = forbidden.map(normalize);
        let writable = own.as_ref().is_none_or(probe_writable);

        let mut set = Self {
            own: OwnStorage {
                root: own_root,
                opened: own.map_or_else(OnceLock::new, OnceLock::from),
                writable,
            },
            bound: Vec::new(),
            notes: Vec::new(),
            own_bound: false,
        };

        for written in bindings {
            set.bind(written, &base, forbidden.as_deref());
        }

        set
    }

    /// The bindings, in the order the configuration lists them.
    #[must_use]
    pub fn bindings(&self) -> &[BoundStorage] {
        &self.bound
    }

    /// The set with `store` added last, for reading only.
    ///
    /// A collecting run reads the store it is about through every place it keeps, and a place that
    /// does not bind that store still needs it to say what its content is. The store added here is
    /// never a write target beyond what the caller already chose: it is the last thing a read tries.
    #[must_use]
    pub fn with_read_fallback(mut self, store: RorolalaStorage) -> Self {
        let writable = probe_writable(&store);
        let root = store.get_root().to_path_buf();

        self.bound.push(BoundStorage {
            written: root.to_string_lossy().into_owned(),
            root,
            own: false,
            store: Some(store),
            writable,
            reason: None,
        });

        self
    }

    /// What could not be used, one note per binding.
    #[must_use]
    pub fn notes(&self) -> &[StorageNote] {
        &self.notes
    }

    /// Where the place's own store is rooted.
    #[must_use]
    pub fn own_root(&self) -> &Path {
        &self.own.root
    }

    /// Whether the place's own store is there.
    #[must_use]
    pub fn own_present(&self) -> bool {
        self.own.opened().is_some()
    }

    /// Whether the place's own store would accept a write, as the construction pre-check saw it.
    #[must_use]
    pub const fn own_writable(&self) -> bool {
        self.own.writable
    }

    /// Whether `.` is one of the bindings, so the own store is already in the order.
    #[must_use]
    pub const fn own_bound(&self) -> bool {
        self.own_bound
    }

    /// The store a write goes to.
    ///
    /// The first binding that is there, whether or not it can be written, and the own store when
    /// no binding is. The own store is made here when it is not there yet.
    #[must_use]
    pub fn write_target(&self) -> &RorolalaStorage {
        self.target().0
    }

    /// Whether the store a write would go to accepts one.
    #[must_use]
    pub fn write_target_writable(&self) -> bool {
        self.target().1
    }

    /// Every store this run can read, in the order it reads them.
    pub fn stores(&self) -> impl Iterator<Item = &RorolalaStorage> {
        let own = if self.own_bound {
            None
        } else {
            self.own.opened()
        };

        self.bound
            .iter()
            .filter_map(|bound| {
                if bound.own {
                    self.own.opened()
                } else {
                    bound.store.as_ref()
                }
            })
            .chain(own)
    }

    /// Reads the object stored under `key`, from the first store that answers.
    ///
    /// # Errors
    ///
    /// Returns the first hard failure when no store answers with the object, and
    /// [`Error::NotFound`] when every store says it does not hold it.
    pub async fn read_object(&self, key: &Key) -> Result<Vec<u8>, Error> {
        let mut first_error = None;

        for store in self.stores() {
            match store.read_object(key).await {
                Ok(bytes) => return Ok(bytes),
                Err(Error::NotFound(_)) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        Err(first_error.unwrap_or(Error::NotFound(*key)))
    }

    /// Reads the object stored under `key` from the first store that answers, as the ranges it is
    /// read as, checking it against its key.
    ///
    /// It is [`read_object`](Self::read_object) for a caller that can use the run as it comes: a
    /// tree structure is decoded out of the range of the file it was read from rather than out of a
    /// copy made to give a decoder one slice.
    ///
    /// # Errors
    ///
    /// As [`read_object`](Self::read_object).
    pub async fn read_object_bytes(&self, key: &Key) -> Result<Bytes, Error> {
        let mut first_error = None;

        for store in self.stores() {
            match store.read_object_bytes(key).await {
                Ok(bytes) => return Ok(bytes),
                Err(Error::NotFound(_)) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        Err(first_error.unwrap_or(Error::NotFound(*key)))
    }

    /// Reads the object stored under `key` from the first store that answers, without checking it
    /// against its key.
    ///
    /// It is [`read_object`](Self::read_object) for a caller that checks the content elsewhere: the
    /// sending side of a transfer, whose peer refuses an entry that does not hash to the key it
    /// arrived under. What that saves is a second pass over every byte that crosses; a local read is
    /// `read_object`, which checks.
    ///
    /// What comes back is the run the object was read as, so a sender hands its pieces to the wire
    /// rather than a copy of them.
    ///
    /// # Errors
    ///
    /// As [`read_object`](Self::read_object), except [`Corrupt`](Error::Corrupt).
    pub async fn read_object_unchecked(&self, key: &Key) -> Result<Bytes, Error> {
        self.open_objects(std::slice::from_ref(key))
            .await?
            .read_unchecked(key)
            .await
    }

    /// Plans a run of object reads as one batch, with every store's packs opened once.
    ///
    /// It is [`RorolalaStorage::open_objects`] for a set: each store is asked to plan every key up
    /// front, so a content of many chunks is a handful of files opened per store rather than every
    /// store walked and its packs opened again for each chunk.
    ///
    /// A store that cannot plan is passed over and its failure kept, the way
    /// [`list_exist_keys`](Self::list_exist_keys) treats one: the batch answers with what the stores
    /// that could be read hold. A set none of them could plan is the failure.
    ///
    /// # Errors
    ///
    /// Returns the first failure when no store could be planned. A key a planned store does not
    /// hold is not one: it is answered for by [`SetBatch::read`].
    pub async fn open_objects(&self, keys: &[Key]) -> Result<SetBatch, Error> {
        let mut plans = Vec::new();
        let mut first_error = None;

        for store in self.stores() {
            match store.open_objects(keys).await {
                Ok(plan) => plans.push(plan),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if let Some(error) = first_error
            && plans.is_empty()
        {
            return Err(error);
        }

        Ok(SetBatch { plans })
    }

    /// Reads the manifest kept under `key`, from the first store that has one.
    ///
    /// # Errors
    ///
    /// Returns the first hard failure when no store answers with a manifest, or `Ok(None)` when no
    /// store keeps one.
    pub async fn manifest_of(&self, key: &Key) -> Result<Option<Manifest>, Error> {
        let mut first_error = None;

        for store in self.stores() {
            match store.manifest_of(key).await {
                Ok(Some(manifest)) => return Ok(Some(manifest)),
                Ok(None) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        first_error.map_or(Ok(None), Err)
    }

    /// The content stored under `key`, put back together however it was laid down.
    ///
    /// # Errors
    ///
    /// Returns the first hard failure when no store produces the content, or `Ok(None)` when no
    /// store holds it.
    pub async fn content(&self, key: &Key) -> Result<Option<Bytes>, Error> {
        let mut first_error = None;

        for store in self.stores() {
            match TransferableBackend::content(store, key).await {
                Ok(Some(content)) => return Ok(Some(content)),
                Ok(None) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        first_error.map_or(Ok(None), Err)
    }

    /// Writes the content stored under `key` out to `destination`.
    ///
    /// `destination` is the caller's, so a store that fails after writing part of it is not
    /// followed by another store writing the same path again: only a store that does not hold the
    /// key is passed over.
    ///
    /// # Errors
    ///
    /// Returns the store's own failure, or [`Error::NotFound`] when no store holds the key.
    pub async fn extract_file(&self, key: &Key, destination: &Path) -> Result<(), Error> {
        for store in self.stores() {
            match store.extract_file(key, destination).await {
                Ok(()) => return Ok(()),
                Err(Error::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }

        Err(Error::NotFound(*key))
    }

    /// Which of `keys` any store holds.
    ///
    /// A key is held when a store says it holds it, so the answer is a union. A store that cannot
    /// be asked holds nothing as far as this answer goes: the alternative is refusing a whole
    /// batch over a binding that is not there.
    ///
    /// # Errors
    ///
    /// This does not fail: a store that cannot answer counts as not holding.
    pub async fn contains_keys(&self, keys: &[Key]) -> Result<Presence, Error> {
        let mut held = vec![Presence::MISSING; keys.len()];

        for store in self.stores() {
            let Ok(presence) = store.contains_keys(keys).await else {
                continue;
            };

            for (index, is_held) in presence.iter().enumerate() {
                if is_held && let Some(flag) = held.get_mut(index) {
                    *flag = Presence::HELD;
                }
            }
        }

        Ok(Presence::new(held))
    }

    /// Every key any store knows of, once each.
    ///
    /// # Errors
    ///
    /// Returns the first store's failure when none can be listed, and the union of what the others
    /// hold when some can.
    pub async fn list_all_keys(&self) -> Result<Vec<Key>, Error> {
        let mut keys = std::collections::BTreeSet::new();
        let mut first_error = None;
        let mut any_listed = false;

        for store in self.stores() {
            match store.list_all_keys().await {
                Ok(listed) => {
                    any_listed = true;
                    keys.extend(listed);
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if let Some(error) = first_error
            && !any_listed
        {
            return Err(error);
        }

        Ok(keys.into_iter().collect())
    }

    /// Every key any store holds, once each.
    ///
    /// # Errors
    ///
    /// Returns the first store's failure when none can be listed, and the union of what the others
    /// hold when some can.
    pub async fn list_exist_keys(&self) -> Result<Vec<Key>, Error> {
        let mut keys = std::collections::BTreeSet::new();
        let mut first_error = None;
        let mut any_listed = false;

        for store in self.stores() {
            match store.list_exist_keys().await {
                Ok(listed) => {
                    any_listed = true;
                    keys.extend(listed);
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if let Some(error) = first_error
            && !any_listed
        {
            return Err(error);
        }

        Ok(keys.into_iter().collect())
    }

    /// Every key any store keeps a manifest for, once each.
    ///
    /// # Errors
    ///
    /// Returns the first store's failure when none can be listed, and the union of what the others
    /// keep when some can.
    pub async fn list_manifest_keys(&self) -> Result<Vec<Key>, Error> {
        let mut keys = std::collections::BTreeSet::new();
        let mut first_error = None;
        let mut any_listed = false;

        for store in self.stores() {
            match store.list_manifest_keys().await {
                Ok(listed) => {
                    any_listed = true;
                    keys.extend(listed);
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if let Some(error) = first_error
            && !any_listed
        {
            return Err(error);
        }

        Ok(keys.into_iter().collect())
    }

    /// Whether any store keeps the content under `key` as a manifest.
    ///
    /// A store that cannot answer counts as not keeping one.
    ///
    /// # Errors
    ///
    /// This does not fail.
    pub async fn holds_manifest(&self, key: &Key) -> Result<bool, Error> {
        for store in self.stores() {
            if store.holds_manifest(key).await.unwrap_or(false) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Writes content in memory to the store a write goes to.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ReadOnly`] when the chosen store cannot be written, and the store's own
    /// failure when it accepts the write and fails it.
    pub async fn write_object(&self, bytes: &[u8], codec: Codec) -> Result<Key, Error> {
        let (store, writable) = self.target();
        refuse_read_only(store, writable)?;

        store.write_object(bytes, codec).await
    }

    /// Writes a file's content to the store a write goes to.
    ///
    /// `cut` and `codec` are what the caller forces; what is not named is the store's own choice,
    /// the same choice it makes when it writes by itself.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ReadOnly`] when the chosen store cannot be written, and the store's own
    /// failure when it accepts the write and fails it.
    pub async fn put_file(
        &self,
        file: &Path,
        cut: Option<Chunking>,
        codec: Option<Codec>,
    ) -> Result<Key, Error> {
        let (store, writable) = self.target();
        refuse_read_only(store, writable)?;

        let mut told = store.clone();
        if let Some(cut) = cut {
            told = told.with_forced_cut(cut);
        }
        if let Some(codec) = codec {
            told = told.with_codec(codec);
        }

        store_file(&told, file).await
    }

    /// Takes content that came from another store, under the key it has to come to.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ReadOnly`] when the chosen store cannot be written, and the store's own
    /// failure when it accepts the content and fails it.
    pub async fn accept(&self, key: &Key, content: Vec<u8>) -> Result<(), Error> {
        let (store, writable) = self.target();
        refuse_read_only(store, writable)?;

        store.accept(key, content).await
    }

    /// The store a write goes to, and whether it accepts one.
    fn target(&self) -> (&RorolalaStorage, bool) {
        for bound in &self.bound {
            if bound.own {
                return (self.own.get_or_create(), bound.writable);
            }
            if let Some(store) = &bound.store {
                return (store, bound.writable);
            }
        }

        (self.own.get_or_create(), self.own.writable)
    }

    /// Resolves one written binding and records it.
    fn bind(&mut self, written: String, base: &Path, forbidden: Option<&Path>) {
        let trimmed = written.trim();

        if trimmed == "." {
            let writable = self.own.writable;
            let root = self.own.root.clone();

            self.own_bound = true;
            self.bound.push(BoundStorage {
                written: written.clone(),
                root: root.clone(),
                own: true,
                store: self.own.opened().cloned(),
                writable,
                reason: (!writable).then(|| "the store is read-only".to_owned()),
            });

            if !writable {
                self.notes.push(StorageNote::ReadOnly { written, root });
            }
            return;
        }

        if trimmed.is_empty() {
            self.dead(written, self.own.root.clone(), "the path is empty");
            return;
        }

        let resolved = normalize(&base.join(Path::new(trimmed)));

        if resolved != self.own.root
            && forbidden.is_some_and(|forbidden| starts_with(&resolved, forbidden))
        {
            self.dead(
                written,
                resolved,
                "the path is inside the place's own data directory",
            );
            return;
        }

        if !resolved.is_dir() {
            self.dead(written, resolved, "the path is not a directory");
            return;
        }

        let Some(store) = RorolalaStorage::at(&resolved) else {
            self.dead(written, resolved, "the directory carries no `rolast.toml`");
            return;
        };

        let writable = probe_writable(&store);
        let reason = (!writable).then(|| "the store is read-only".to_owned());

        if !writable {
            self.notes.push(StorageNote::ReadOnly {
                written: written.clone(),
                root: resolved.clone(),
            });
        }

        self.bound.push(BoundStorage {
            written,
            root: resolved,
            own: false,
            store: Some(store),
            writable,
            reason,
        });
    }

    /// Records a binding that cannot be used.
    fn dead(&mut self, written: String, root: PathBuf, reason: &str) {
        self.notes.push(StorageNote::Unusable {
            written: written.clone(),
            root: root.clone(),
            reason: reason.to_owned(),
        });
        self.bound.push(BoundStorage {
            written,
            root,
            own: false,
            store: None,
            writable: false,
            reason: Some(reason.to_owned()),
        });
    }
}

/// A run of object reads planned as one batch, across every store a [`StorageSet`] reads.
///
/// It is [`ObjectBatch`] over a set: each store's packs are opened once when the batch is made, and
/// a read asks the stores in the order the set reads them, answering with the first that holds the
/// key. See [`StorageSet::open_objects`].
pub struct SetBatch {
    /// One plan per store, in the order the set reads them.
    plans: Vec<ObjectBatch>,
}

impl SetBatch {
    /// The content stored under `key`, from the first store that answers.
    ///
    /// # Errors
    ///
    /// Returns the first hard failure when no store answers with the object, and
    /// [`Error::NotFound`] when every store says it does not hold it.
    pub async fn read(&self, key: &Key) -> Result<Bytes, Error> {
        self.read_where(key, true).await
    }

    /// As [`read`](Self::read), without checking the content against its key.
    ///
    /// # Errors
    ///
    /// As [`read`](Self::read), except [`Corrupt`](Error::Corrupt).
    pub async fn read_unchecked(&self, key: &Key) -> Result<Bytes, Error> {
        self.read_where(key, false).await
    }

    /// Reads `key` from the stores in order, checking it against its key when `check` says to.
    ///
    /// A store that answers with nothing is one more that was asked, while a store that failed is
    /// the account kept in case none answers: the same rule [`StorageSet::read_object`] reads.
    async fn read_where(&self, key: &Key, check: bool) -> Result<Bytes, Error> {
        let mut first_error = None;

        for plan in &self.plans {
            let answer = if check {
                plan.read(key).await
            } else {
                plan.read_unchecked(key).await
            };

            match answer {
                Ok(bytes) => return Ok(bytes),
                Err(Error::NotFound(_)) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        Err(first_error.unwrap_or(Error::NotFound(*key)))
    }
}

/// Refuses a write aimed at a store the pre-check found read-only.
fn refuse_read_only(store: &RorolalaStorage, writable: bool) -> Result<(), Error> {
    if writable {
        return Ok(());
    }

    Err(Error::ReadOnly {
        path: store.get_root().to_path_buf(),
    })
}

/// Whether `store` would accept a write, without writing anything.
///
/// Opening the configuration for writing is the smallest question that answers it: a read-only
/// mount, or a directory the run may not write, refuses here, while a writable one opens and is
/// closed without a byte changed.
fn probe_writable(store: &RorolalaStorage) -> bool {
    OpenOptions::new()
        .write(true)
        .open(store.config_path())
        .is_ok()
}

/// Whether `path` is `root` or inside it, comparing path components.
///
/// A string prefix would take `/srv/shared-two` for a path inside `/srv/shared`, so the components
/// are compared rather than the spelling. Windows names are compared without ASCII case, since two
/// spellings of one directory there are one directory; the rest of the world is compared as written.
fn starts_with(path: &Path, root: &Path) -> bool {
    let mut parts = path.components();

    root.components().all(|expected| {
        parts
            .next()
            .is_some_and(|actual| same_component(actual, expected))
    })
}

/// Whether two path components name the same thing.
///
/// This only decides whether a binding is inside the place's own data directory, never whether two
/// bindings are the same store, so the ASCII case rule Windows needs is enough.
#[cfg(windows)]
fn same_component(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

/// Whether two path components name the same thing.
#[cfg(not(windows))]
fn same_component(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str() == right.as_os_str()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_location::Locate as _;

    use crate::{Chunking, Codec, Error, RorolalaStorage, store_file};

    use super::{StorageNote, StorageSet};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-storage-set-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A store at `root`, made and returned.
    fn store_at(root: &Path) -> RorolalaStorage {
        RorolalaStorage::create(root)
    }

    /// The set a test reads through: `own` at `own_root`, and the bindings as written.
    fn set_of(own: Option<RorolalaStorage>, own_root: &Path, bindings: &[&str]) -> StorageSet {
        StorageSet::with_bindings(
            own,
            own_root,
            own_root,
            bindings.iter().map(|binding| (*binding).to_owned()),
            None,
        )
    }

    #[tokio::test]
    async fn a_read_falls_through_to_the_store_that_holds_the_object() {
        let dir = scratch("read-order");
        let first = dir.join("first");
        let second = dir.join("second");
        let first_store = store_at(&first);
        let second_store = store_at(&second);
        let key = second_store
            .write_object(b"found here", Codec::Raw)
            .await
            .unwrap();

        let set = set_of(
            Some(first_store),
            &dir.join("own"),
            &[first.to_str().unwrap(), second.to_str().unwrap()],
        );

        assert_eq!(set.read_object(&key).await.unwrap(), b"found here");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_broken_copy_is_passed_over_for_one_that_reads() {
        let dir = scratch("read-broken");
        let first = dir.join("first");
        let second = dir.join("second");
        let first_store = store_at(&first);
        let second_store = store_at(&second);
        let key = second_store
            .write_object(b"the good one", Codec::Raw)
            .await
            .unwrap();

        // The copy in the first store does not hash to the key it is stored under.
        let corrupt_store = RorolalaStorage::at(&first).unwrap();
        let path = corrupt_store
            .get_root()
            .join("obj")
            .join(&key.hex()[..2])
            .join(&key.hex()[2..4])
            .join(key.hex());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not the content").unwrap();

        let set = set_of(
            Some(first_store),
            &dir.join("own"),
            &[first.to_str().unwrap(), second.to_str().unwrap()],
        );

        assert_eq!(set.read_object(&key).await.unwrap(), b"the good one");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_key_no_copy_makes_is_a_hard_failure_rather_than_missing() {
        let dir = scratch("read-hard");
        let first = dir.join("first");
        let store = store_at(&first);
        let key = store.write_object(b"wanted", Codec::Raw).await.unwrap();

        // The copy is there and does not read as the frame it should be: a hard failure, not
        // "nothing is stored here".
        let hex = key.hex();
        let path = first
            .join("obj")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(&hex);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"wrong").unwrap();

        let set = set_of(
            Some(store_at(&dir.join("second"))),
            &dir.join("own"),
            &[first.to_str().unwrap()],
        );

        assert!(matches!(set.read_object(&key).await, Err(Error::Malformed)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_manifest_is_read_from_the_store_that_keeps_one() {
        let dir = scratch("read-manifest");
        let whole_side = dir.join("whole");
        let chunked_side = dir.join("chunked");
        let content = vec![7_u8; 64];
        let file = dir.join("content.bin");
        fs::write(&file, &content).unwrap();

        let whole_store = store_at(&whole_side);
        let key = whole_store
            .write_object(&content, Codec::Raw)
            .await
            .unwrap();

        // The other store writes the same content cut into fixed chunks, so it keeps a manifest
        // for it while the whole one keeps no manifest at all.
        let chunked_store = store_at(&chunked_side).with_forced_cut(Chunking::Fixed { size: 16 });
        assert_eq!(store_file(&chunked_store, &file).await.unwrap(), key);
        assert!(chunked_store.holds_manifest(&key).await.unwrap());

        let set = set_of(
            None,
            &dir.join("own"),
            &[whole_side.to_str().unwrap(), chunked_side.to_str().unwrap()],
        );

        assert!(set.manifest_of(&key).await.unwrap().is_some());
        assert_eq!(set.content(&key).await.unwrap().unwrap().to_vec(), content);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn presence_and_listings_are_unions_of_what_the_stores_hold() {
        let dir = scratch("union");
        let first = dir.join("first");
        let second = dir.join("second");
        let first_store = store_at(&first);
        let second_store = store_at(&second);
        let one = first_store.write_object(b"one", Codec::Raw).await.unwrap();
        let two = second_store.write_object(b"two", Codec::Raw).await.unwrap();

        let set = set_of(
            None,
            &dir.join("own"),
            &[first.to_str().unwrap(), second.to_str().unwrap()],
        );

        let presence = set.contains_keys(&[one, two]).await.unwrap();
        assert_eq!(presence.iter().collect::<Vec<_>>(), [true, true]);

        let mut listed = set.list_all_keys().await.unwrap();
        listed.sort_unstable();
        assert_eq!(listed, [one, two]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_write_goes_to_the_first_binding_that_is_there() {
        let dir = scratch("write-target");
        let first = dir.join("first");
        let second = dir.join("second");
        let _ = store_at(&first);
        let _ = store_at(&second);

        let set = set_of(
            None,
            &dir.join("own"),
            &[
                dir.join("missing").to_str().unwrap(),
                second.to_str().unwrap(),
                first.to_str().unwrap(),
            ],
        );

        let key = set.write_object(b"content", Codec::Raw).await.unwrap();
        assert!(
            RorolalaStorage::at(&second)
                .unwrap()
                .read_object(&key)
                .await
                .is_ok()
        );
        assert!(
            first
                .join("obj")
                .read_dir()
                .is_ok_and(|mut entries| entries.next().is_none())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_write_refuses_a_read_only_store_rather_than_moving_on() {
        let dir = scratch("write-readonly");
        let first = dir.join("first");
        let second = dir.join("second");
        let _ = store_at(&first);
        let _ = store_at(&second);

        // The configuration is the store's writable surface: making it read-only is what a
        // read-only mount looks like to the pre-check.
        let config = first.join("rolast.toml");
        let original = fs::metadata(&config).unwrap().permissions();
        let mut permissions = original.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&config, permissions).unwrap();

        let set = StorageSet::with_bindings(
            None,
            dir.join("own"),
            &dir,
            [
                first.to_str().unwrap().to_owned(),
                second.to_str().unwrap().to_owned(),
            ],
            None,
        );

        assert_eq!(set.write_target().get_root(), first.as_path());
        assert!(!set.write_target_writable());
        assert!(matches!(
            set.write_object(b"content", Codec::Raw).await,
            Err(Error::ReadOnly { .. })
        ));
        assert!(matches!(
            set.notes()
                .iter()
                .find(|note| matches!(note, StorageNote::ReadOnly { .. })),
            Some(StorageNote::ReadOnly { .. })
        ));

        fs::set_permissions(&config, original).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_set_with_no_usable_binding_writes_to_the_store_it_makes() {
        let dir = scratch("write-own");
        let own_root = dir.join("own").join("storage");

        let set = set_of(None, &own_root, &[dir.join("missing").to_str().unwrap()]);

        let key = set.write_object(b"kept", Codec::Raw).await.unwrap();
        assert_eq!(set.write_target().get_root(), own_root.as_path());
        assert_eq!(
            RorolalaStorage::at(&own_root)
                .unwrap()
                .read_object(&key)
                .await
                .unwrap(),
            b"kept"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_binding_that_cannot_be_used_is_noted_once() {
        let dir = scratch("notes");
        let first = dir.join("first");
        let _ = store_at(&first);
        let not_a_store = dir.join("plain");
        fs::create_dir_all(&not_a_store).unwrap();
        let forbidden = dir.join(".rola");
        let own_root = forbidden.join("storage");
        fs::create_dir_all(&own_root).unwrap();

        let set = StorageSet::with_bindings(
            Some(store_at(&own_root)),
            &own_root,
            &own_root,
            [
                "missing".to_owned(),
                not_a_store.to_str().unwrap().to_owned(),
                "../".to_owned(),
            ],
            Some(&forbidden),
        );

        // A missing directory, one with no configuration, and one inside the place's own data
        // directory: three notes, one each.
        assert_eq!(set.notes().len(), 3);
        assert_eq!(set.bindings().len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dot_puts_the_own_store_where_it_is_written() {
        let dir = scratch("dot");
        let own_root = dir.join("own");
        let first = dir.join("first");
        let _ = store_at(&own_root);
        let _ = store_at(&first);
        let own = RorolalaStorage::at(&own_root).unwrap();

        let set = StorageSet::with_bindings(
            Some(own),
            &own_root,
            &own_root,
            [first.to_str().unwrap().to_owned(), ".".to_owned()],
            None,
        );

        let roots: Vec<PathBuf> = set
            .stores()
            .map(|store| store.get_root().to_path_buf())
            .collect();
        assert_eq!(roots, [first, own_root]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_batch_finds_a_key_in_the_store_that_holds_it() {
        let dir = scratch("batch-order");
        let first = dir.join("first");
        let second = dir.join("second");
        let first_store = store_at(&first);
        let second_store = store_at(&second);
        let key = second_store
            .write_object(b"found in the second", Codec::Raw)
            .await
            .unwrap();

        let set = set_of(
            Some(first_store),
            &first,
            &[first.to_str().unwrap(), second.to_str().unwrap()],
        );
        let batch = set.open_objects(&[key]).await.unwrap();

        // The first store holds nothing under the key, so the second is the one that answers. The
        // plan is made once for both, which is what makes a content of many chunks one open per
        // store rather than one per chunk.
        assert_eq!(
            batch.read(&key).await.unwrap().to_vec(),
            b"found in the second"
        );
        assert_eq!(
            batch.read_unchecked(&key).await.unwrap().to_vec(),
            b"found in the second"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_batch_answers_bytes_a_key_does_not_cover_when_it_is_not_asked_to_check() {
        let dir = scratch("batch-unchecked");
        let root = dir.join("store");
        let store = store_at(&root);

        let one = store.write_object(b"one", Codec::Raw).await.unwrap();
        let other = store.write_object(b"other", Codec::Raw).await.unwrap();

        // The entry under `one` now holds `other`'s bytes: a frame that reads, whose payload is not
        // the content the key names.
        fs::copy(store.object_path(&other), store.object_path(&one)).unwrap();

        let set = set_of(Some(store), &root, &[]);
        let batch = set.open_objects(&[one]).await.unwrap();

        assert!(matches!(batch.read(&one).await, Err(Error::Corrupt(_))));
        assert_eq!(batch.read_unchecked(&one).await.unwrap().to_vec(), b"other");

        let _ = fs::remove_dir_all(&dir);
    }
}

//! The version control index: where a run's variants and versions are kept.
//!
//! The index is a store of its own, kept in a directory of its own and addressed by the same
//! kind of key a store is — but it is not the store: what it holds are the index objects the
//! version control logic works in, not the content a project is made of. It implements the two
//! storage traits so that the transfer two ends speak is the same one, and reuses storage's
//! framing, keys and pack directories rather than growing a second of each.

use std::collections::BTreeSet;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::atomic::{AtomicU64, Ordering};

use rorolala_storage::{Codec, FRAME_VERSION, Frame, Layout, LockError, Lockable, LockingGuard};
use rorolala_utils_constants::{
    LOCK_FILE, VAULT_CONFIG_PATH, VAULT_INDEX_DIR, WORKSPACE_DATA_DIR, WORKSPACE_INDEX_DIR,
};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::{Locate, LocateHelper};
use serde::Serialize;

use crate::error::{
    ParseVCSIndexObjectError, VCSIndexError, VCSIndexReadingError, VCSIndexWritingError,
};
use crate::{Creator, Hash, Message, Variant, Version};

mod backend;
mod object;
mod pack;

pub use object::{VCSIndexKind, VCSWrite};

/// The directory loose objects are kept under, inside an index.
const OBJECTS_DIR: &str = "obj";

/// How many hex characters of a key name its first directory level.
const SLICE_FIRST: usize = 2;

/// How many hex characters of a key name its second directory level.
const SLICE_SECOND: usize = 2;

/// The suffix a temporary file's name ends in.
const TEMPORARY_SUFFIX: &str = ".tmp";

/// The Rorolala version control index
///
/// Records every index object of the version control system, to be queried and read.
///
/// It is the index directory of whichever the run is inside — a Vault's or a Workspace's —
/// so locating one is locating that directory, not the Vault or Workspace that holds it.
#[lazyffi(export = RolaVCSIndex)]
#[derive(Default, Clone)]
pub struct VCSIndex {
    /// The directory this index keeps its files in
    path: PathBuf,
}

/// Locates the [`VCSIndex`] the given directory is inside
///
/// Starting at `index_dir`, this walks up the directory tree looking for the marker that
/// makes a directory a Vault or a Workspace, and answers with the index directory that
/// marker names; see [`Locate::locate`]. `None` if neither is found all the way up.
#[must_use]
#[lazyffi(export = locate_rola_vcs_index)]
pub fn locate_vcs_index(index_dir: &Path) -> Option<VCSIndex> {
    VCSIndex::locate(index_dir)
}

impl Locate for VCSIndex {
    /// Finds the index directory of the nearest Vault or Workspace above `cwd`.
    ///
    /// It is one walk up the tree, not two: whichever marker is nearer is the one the run
    /// is inside, so a Workspace nested in a Vault is the Workspace's index. Where a single
    /// directory is both — it holds `vault.toml` and `.rola` — the Vault is taken, since
    /// that is the one a run reached from there is served by.
    fn locate(cwd: &Path) -> Option<Self> {
        let root = cwd.locate(|dir| {
            dir.join(VAULT_CONFIG_PATH).exists() || dir.join(WORKSPACE_DATA_DIR).is_dir()
        })?;

        // A Vault keeps its index at its root; a Workspace keeps it inside its data
        // directory. The path is walked back into the shape it names, so the `./` the
        // layout writes is not left in the middle of a path a reader is shown.
        let layout = if root.join(VAULT_CONFIG_PATH).exists() {
            VAULT_INDEX_DIR
        } else {
            WORKSPACE_INDEX_DIR
        };

        Some(Self {
            path: root.join(layout).components().collect(),
        })
    }

    fn get_root(&self) -> &Path {
        self.path.as_path()
    }
}

impl Lockable for VCSIndex {
    /// The lock sits at the index's root, beside the objects it guards.
    ///
    /// An index is one place whether it is reached through a Vault or a Workspace, so there is one
    /// lock for it either way, and a run that holds it through a Vault holds it against a run that
    /// reached the same index directly.
    fn lock_path(&self) -> PathBuf {
        self.path.join(LOCK_FILE)
    }

    async fn lock(&self) -> Result<LockingGuard<Self>, LockError> {
        LockingGuard::acquire(self.clone(), self.lock_path()).await
    }
}

/// An object a version control index holds
///
/// The kinds of object an index can store.
#[lazyffi(export = RolaVCSIndexObject)]
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum VCSIndexObject {
    /// A Variant object
    Variant(Variant),
    /// A Version object
    Version(Version),
    /// A Creator object
    Creator(Creator),
    /// A Message object
    Message(Message),
}

#[lazyffi(export = rola_vcs_index_object_)]
impl VCSIndexObject {
    /// The Variant this object is, or an error if it is not one
    ///
    /// # Errors
    ///
    /// Returns [`ParseVCSIndexObjectError::ExpectVariant`] if this object is another kind.
    #[lazyffi(export = expect_object_is_variant)]
    pub fn expect_variant(self) -> Result<Variant, ParseVCSIndexObjectError> {
        match self {
            Self::Variant(variant) => Ok(variant),
            _ => Err(ParseVCSIndexObjectError::ExpectVariant),
        }
    }

    /// The Version this object is, or an error if it is not one
    ///
    /// # Errors
    ///
    /// Returns [`ParseVCSIndexObjectError::ExpectVersion`] if this object is another kind.
    #[lazyffi(export = expect_object_is_version)]
    pub fn expect_version(self) -> Result<Version, ParseVCSIndexObjectError> {
        match self {
            Self::Version(version) => Ok(version),
            _ => Err(ParseVCSIndexObjectError::ExpectVersion),
        }
    }

    /// The Creator this object is, or an error if it is not one
    ///
    /// # Errors
    ///
    /// Returns [`ParseVCSIndexObjectError::ExpectCreator`] if this object is another kind.
    #[lazyffi(export = expect_object_is_creator)]
    pub fn expect_creator(self) -> Result<Creator, ParseVCSIndexObjectError> {
        match self {
            Self::Creator(creator) => Ok(creator),
            _ => Err(ParseVCSIndexObjectError::ExpectCreator),
        }
    }

    /// The Message this object is, or an error if it is not one
    ///
    /// # Errors
    ///
    /// Returns [`ParseVCSIndexObjectError::ExpectMessage`] if this object is another kind.
    #[lazyffi(export = expect_object_is_message)]
    pub fn expect_message(self) -> Result<Message, ParseVCSIndexObjectError> {
        match self {
            Self::Message(message) => Ok(message),
            _ => Err(ParseVCSIndexObjectError::ExpectMessage),
        }
    }

    /// The Variant this object is, or an error if it is not one
    ///
    /// The same as [`expect_variant`](Self::expect_variant), under the name a `try_` reads by.
    ///
    /// # Errors
    ///
    /// Returns [`ParseVCSIndexObjectError::ExpectVariant`] if this object is another kind.
    #[lazyffi(export = try_get_variant_from_object)]
    pub fn variant(self) -> Result<Variant, ParseVCSIndexObjectError> {
        self.expect_variant()
    }

    /// The Version this object is, or an error if it is not one
    ///
    /// The same as [`expect_version`](Self::expect_version), under the name a `try_` reads by.
    ///
    /// # Errors
    ///
    /// Returns [`ParseVCSIndexObjectError::ExpectVersion`] if this object is another kind.
    #[lazyffi(export = try_get_version_from_object)]
    pub fn version(self) -> Result<Version, ParseVCSIndexObjectError> {
        self.expect_version()
    }

    /// Whether this object is a Variant
    #[must_use]
    #[lazyffi(export = is_object_variant)]
    pub fn is_variant(self) -> bool {
        matches!(self, Self::Variant(_))
    }

    /// Whether this object is a Version
    #[must_use]
    #[lazyffi(export = is_object_version)]
    pub fn is_version(self) -> bool {
        matches!(self, Self::Version(_))
    }

    /// Whether this object is a Creator
    #[must_use]
    #[lazyffi(export = is_object_creator)]
    pub fn is_creator(self) -> bool {
        matches!(self, Self::Creator(_))
    }

    /// Whether this object is a Message
    #[must_use]
    #[lazyffi(export = is_object_message)]
    pub fn is_message(self) -> bool {
        matches!(self, Self::Message(_))
    }
}

impl TryInto<Variant> for VCSIndexObject {
    type Error = ParseVCSIndexObjectError;

    fn try_into(self) -> Result<Variant, Self::Error> {
        self.expect_variant()
    }
}

impl TryInto<Version> for VCSIndexObject {
    type Error = ParseVCSIndexObjectError;

    fn try_into(self) -> Result<Version, Self::Error> {
        self.expect_version()
    }
}

impl From<Variant> for VCSIndexObject {
    fn from(variant: Variant) -> Self {
        Self::Variant(variant)
    }
}

impl From<Version> for VCSIndexObject {
    fn from(version: Version) -> Self {
        Self::Version(version)
    }
}

impl TryInto<Creator> for VCSIndexObject {
    type Error = ParseVCSIndexObjectError;

    fn try_into(self) -> Result<Creator, Self::Error> {
        self.expect_creator()
    }
}

impl TryInto<Message> for VCSIndexObject {
    type Error = ParseVCSIndexObjectError;

    fn try_into(self) -> Result<Message, Self::Error> {
        self.expect_message()
    }
}

impl From<Creator> for VCSIndexObject {
    fn from(creator: Creator) -> Self {
        Self::Creator(creator)
    }
}

impl From<Message> for VCSIndexObject {
    fn from(message: Message) -> Self {
        Self::Message(message)
    }
}

impl VCSIndex {
    /// The index rooted at `root`, if a directory of objects is there.
    ///
    /// Nothing is made: this only reads whether the directory is an index already.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Option<Self> {
        let root = root.into();

        root.join(OBJECTS_DIR)
            .is_dir()
            .then_some(Self { path: root })
    }

    /// Makes the index rooted at `root`: somewhere for its loose objects to go.
    ///
    /// A Vault or a Workspace makes its own index when it is made, so this is for a caller that
    /// has a directory and wants one to be an index. The pack directory is made when the first
    /// pack is, so nothing else is laid down here.
    #[must_use]
    pub fn create(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let _ = std::fs::create_dir_all(root.join(OBJECTS_DIR));

        Self { path: root }
    }
}

impl VCSIndex {
    /// Writes `object` into the index, answering with the key it is stored under.
    ///
    /// The key is the object's own hash, so writing the same object twice stores it once and
    /// answers with the same key either time.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexWritingError`] if the index cannot be written to.
    pub async fn write<W: VCSWrite>(&self, object: W) -> Result<Hash, VCSIndexWritingError> {
        self.write_object_held(object).await.map_err(Into::into)
    }

    /// Reads the object stored under `key`, whichever kind it is.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError::NotFound`] if nothing is stored under `key`,
    /// [`VCSIndexReadingError::Malformed`] if what is stored is not an index object this build
    /// knows, [`VCSIndexReadingError::Corrupt`] if what came back does not hash to `key`, and
    /// [`VCSIndexReadingError::Io`] if the index itself fails.
    pub async fn read(&self, key: Hash) -> Result<VCSIndexObject, VCSIndexReadingError> {
        self.read_object_typed(key).await.map_err(Into::into)
    }

    /// Writes `object` and answers with its key, or why the index could not take it.
    async fn write_object_held<W: VCSWrite>(&self, object: W) -> Result<Hash, VCSIndexError> {
        let key = object.hash();
        self.write_object(&key, &object.encode(), Codec::Raw)
            .await?;

        Ok(key)
    }

    /// Reads the object under `key`, checked against the key it was read under.
    async fn read_object_typed(&self, key: Hash) -> Result<VCSIndexObject, VCSIndexError> {
        let plain = self.read_object(&key).await?;
        let object = VCSIndexObject::decode(&plain)?;

        // The key is the object's own hash, so what came back is checked against it rather
        // than taken for what the key names.
        if object.hash() != key {
            return Err(VCSIndexError::Corrupt(key));
        }

        Ok(object)
    }

    /// The number of `version` in its chain, worked out by tracing back to the root
    ///
    /// A number is derived rather than stored, so this is what a read leaves for the caller to
    /// ask: the chain is walked one link at a time — the version's variant, that variant's base
    /// version, and so on — until the root, which is one before the first version. It costs a read
    /// per link, so it is meant to be asked once and kept rather than in a loop.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError`] if a link in the chain cannot be read or does not read as
    /// the kind it should be.
    pub async fn version_num(&self, version: &Version) -> Result<u64, VCSIndexReadingError> {
        self.trace_version(version).await.map_err(Into::into)
    }

    /// The number of the Version `variant` is based on
    ///
    /// A variant's number is the number of its base version, so this is the same trace stopped one
    /// link short; see [`version_num`](Self::version_num).
    ///
    /// # Errors
    ///
    /// As [`version_num`](Self::version_num).
    pub async fn variant_num(&self, variant: &Variant) -> Result<u64, VCSIndexReadingError> {
        let base = self
            .read_object_typed(Hash::new(*variant.base_version()))
            .await
            .and_then(expect_version)
            .map_err(VCSIndexReadingError::from)?;

        self.version_num(&base).await
    }

    /// Walks the chain back from `version` to the root, counting the links.
    async fn trace_version(&self, version: &Version) -> Result<u64, VCSIndexError> {
        let mut steps = 0_u64;
        let mut current = version.clone();

        while !current.is_root() {
            let variant = self
                .read_object_typed(Hash::new(*current.variant()))
                .await
                .and_then(expect_variant)?;
            current = self
                .read_object_typed(Hash::new(*variant.base_version()))
                .await
                .and_then(expect_version)?;
            steps = steps.wrapping_add(1);
        }

        // The root is one before the first version, so a chain of `n` links past it is numbered
        // `n - 1`: the first version a file has is numbered from nothing.
        Ok(steps.wrapping_sub(1))
    }

    /// Every object the index holds, with the key it is held under, in key order
    ///
    /// Each object is read whole, so this costs a read per object — which is what a listing is.
    /// The kinds are not told apart here: what a caller does with an object is the caller's.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError`] if the keys cannot be listed or an object cannot be read.
    pub async fn read_objects(&self) -> Result<Vec<(Hash, VCSIndexObject)>, VCSIndexReadingError> {
        let keys = self
            .list_object_keys()
            .await
            .map_err(VCSIndexReadingError::from)?;

        let mut objects = Vec::with_capacity(keys.len());
        for key in keys {
            objects.push((key, self.read(key).await?));
        }

        Ok(objects)
    }

    /// Every key the index holds an object under, loose or in a pack
    ///
    /// This is what a caller that has to know what the index holds — without reading any of it —
    /// asks: the keys alone, which the pack index and the loose tree name without an object being
    /// read. It is the cheap half of a listing, and what a reader compares against to see whether
    /// something built from the index still describes it.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError`] if the keys cannot be listed.
    pub async fn object_keys(&self) -> Result<Vec<Hash>, VCSIndexReadingError> {
        self.list_object_keys().await.map_err(Into::into)
    }

    /// Where the object stored under `key` sits, once it is loose.
    ///
    /// The digest is sharded two levels deep, the way a store's is, so no directory collects
    /// an unbounded number of entries.
    fn object_path(&self, key: &Hash) -> PathBuf {
        let hex = key.hex();

        self.path
            .join(OBJECTS_DIR)
            .join(&hex[..SLICE_FIRST])
            .join(&hex[SLICE_FIRST..SLICE_FIRST + SLICE_SECOND])
            .join(&hex)
    }

    /// Writes `plain` as the object stored under `key`, if it is not held already.
    ///
    /// The key is the object's own hash, so an object that is already held is the very object
    /// in hand and writing it again would only put a second copy beside the first.
    pub(crate) async fn write_object(
        &self,
        key: &Hash,
        plain: &[u8],
        codec: Codec,
    ) -> Result<(), VCSIndexError> {
        if self.holds_object(key).await? {
            return Ok(());
        }

        let frame = Frame {
            version: FRAME_VERSION,
            codec,
            layout: Layout::Single,
            plain_len: plain.len() as u64,
        };
        let bytes = codec.encode(plain)?;

        self.write_entry(&self.object_path(key), &frame, &bytes)
            .await
    }

    /// The object stored under `key`, framed as this build writes it.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::NotFound`] if nothing is stored under `key`, and
    /// [`VCSIndexError::Malformed`] if what is there does not read as an object.
    pub(crate) async fn read_object(&self, key: &Hash) -> Result<Vec<u8>, VCSIndexError> {
        let bytes = self
            .entry_bytes(key)
            .await?
            .ok_or(VCSIndexError::NotFound(*key))?;
        let (frame, header) = Frame::decode(&bytes)?;

        // An index object is stored whole: it is never cut, so a frame that says otherwise is
        // not one this build wrote.
        if frame.layout != Layout::Single {
            return Err(VCSIndexError::Malformed);
        }

        let payload = bytes.get(header..).ok_or(VCSIndexError::Malformed)?;
        let plain_len = usize::try_from(frame.plain_len).map_err(|_| VCSIndexError::Malformed)?;

        Ok(frame.codec.decode(payload, plain_len)?)
    }

    /// Whether an object is stored under `key`, loose or in a pack.
    pub(crate) async fn holds_object(&self, key: &Hash) -> Result<bool, VCSIndexError> {
        match tokio::fs::metadata(self.object_path(key)).await {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        Ok(self.find_packed(key).await?.is_some())
    }

    /// Every key the index holds an object under, loose or in a pack.
    pub(crate) async fn list_object_keys(&self) -> Result<Vec<Hash>, VCSIndexError> {
        let mut keys = BTreeSet::new();
        collect(&self.path.join(OBJECTS_DIR), &mut keys).await?;

        // A pack holds objects too, so what a caller listing the index wants is every key in it
        // whichever way the object happens to be kept.
        for index in self.packs().await? {
            for entry in self.read_pack_index(index).await?.entries() {
                keys.insert(entry.key());
            }
        }

        Ok(keys.into_iter().collect())
    }

    /// Drops the object stored under `key`, loose or in a pack.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexError::NotFound`] if nothing is stored under `key`.
    pub(crate) async fn remove_object(&self, key: &Hash) -> Result<(), VCSIndexError> {
        match tokio::fs::remove_file(self.object_path(key)).await {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        // Nothing loose under the key, so what is left is a pack, and taking it away means taking
        // it out of the pack.
        self.remove_object_packed(key).await
    }

    /// The framed bytes of the entry stored under `key`, loose or in a pack, or `None` when there
    /// is none.
    async fn entry_bytes(&self, key: &Hash) -> Result<Option<Vec<u8>>, VCSIndexError> {
        if let Some(bytes) = self.read_entry(key).await? {
            return Ok(Some(bytes));
        }

        match self.find_packed(key).await? {
            Some(source) => Ok(Some(self.read_source(&source).await?)),
            None => Ok(None),
        }
    }

    /// Writes an entry — its frame and its payload — where `path` says.
    ///
    /// It is written beside where it belongs and moved into place, so a write that is cut
    /// short leaves the index as it was rather than one holding half an entry.
    async fn write_entry(
        &self,
        path: &Path,
        frame: &Frame,
        payload: &[u8],
    ) -> Result<(), VCSIndexError> {
        let parent = path.parent().ok_or(VCSIndexError::Malformed)?;
        tokio::fs::create_dir_all(parent).await?;

        let mut bytes = frame.encode();
        bytes.extend_from_slice(payload);

        let temporary = temporary_beside(path)?;
        tokio::fs::write(&temporary, &bytes).await?;
        tokio::fs::rename(&temporary, path).await?;

        Ok(())
    }

    /// The bytes of the entry stored under `key`, or `None` when there is none.
    async fn read_entry(&self, key: &Hash) -> Result<Option<Vec<u8>>, VCSIndexError> {
        match tokio::fs::read(self.object_path(key)).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

#[lazyffi(export = rola_vcs_index_)]
impl VCSIndex {
    /// Writes `variant` into the index, answering with the key it is stored under.
    ///
    /// The write is asynchronous and this cannot be, so the two meet here: a runtime of this
    /// call's own waits for it. C is handed the key in the hex it is written down by.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexWritingError`] if the index cannot be written to.
    ///
    /// # FFI
    /// Blocks until the write is done, and answers with a `char *` the caller releases with
    /// `free_string`.
    #[lazyffi(export = write_variant_into_index)]
    pub fn write_variant(&self, variant: &Variant) -> Result<String, VCSIndexWritingError> {
        let key = run(self.write(variant.clone()))
            .map_err(|error| VCSIndexWritingError::Io(error.into()))??;

        Ok(key.to_string())
    }

    /// Writes `version` into the index, answering with the key it is stored under.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexWritingError`] if the index cannot be written to.
    ///
    /// # FFI
    /// Blocks until the write is done, and answers with a `char *` the caller releases with
    /// `free_string`.
    #[lazyffi(export = write_version_into_index)]
    pub fn write_version(&self, version: &Version) -> Result<String, VCSIndexWritingError> {
        let key = run(self.write(version.clone()))
            .map_err(|error| VCSIndexWritingError::Io(error.into()))??;

        Ok(key.to_string())
    }

    /// Reads the Variant stored under `hash`, which is the hex a write answered with.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError`] if nothing is stored under `hash`, what is stored is
    /// not an index object, what came back does not hash to it, or the index fails.
    ///
    /// # FFI
    /// Blocks until the read is done.
    #[lazyffi(export = read_variant_from_index)]
    pub fn read_variant(&self, hash: &str) -> Result<Variant, VCSIndexReadingError> {
        let key = Hash::from_str(hash).map_err(|_| VCSIndexReadingError::Malformed)?;
        let object =
            run(self.read(key)).map_err(|error| VCSIndexReadingError::Io(error.into()))??;

        object
            .expect_variant()
            .map_err(|_| VCSIndexReadingError::Malformed)
    }

    /// Reads the Version stored under `hash`, which is the hex a write answered with.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError`] if nothing is stored under `hash`, what is stored is
    /// not an index object, what came back does not hash to it, or the index fails.
    ///
    /// # FFI
    /// Blocks until the read is done.
    #[lazyffi(export = read_version_from_index)]
    pub fn read_version(&self, hash: &str) -> Result<Version, VCSIndexReadingError> {
        let key = Hash::from_str(hash).map_err(|_| VCSIndexReadingError::Malformed)?;
        let object =
            run(self.read(key)).map_err(|error| VCSIndexReadingError::Io(error.into()))??;

        object
            .expect_version()
            .map_err(|_| VCSIndexReadingError::Malformed)
    }

    /// Writes `creator` into the index, answering with the key it is stored under.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexWritingError`] if the index cannot be written to.
    ///
    /// # FFI
    /// Blocks until the write is done, and answers with a `char *` the caller releases with
    /// `free_string`.
    #[lazyffi(export = write_creator_into_index)]
    pub fn write_creator(&self, creator: &Creator) -> Result<String, VCSIndexWritingError> {
        let key = run(self.write(creator.clone()))
            .map_err(|error| VCSIndexWritingError::Io(error.into()))??;

        Ok(key.to_string())
    }

    /// Writes `message` into the index, answering with the key it is stored under.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexWritingError`] if the index cannot be written to.
    ///
    /// # FFI
    /// Blocks until the write is done, and answers with a `char *` the caller releases with
    /// `free_string`.
    #[lazyffi(export = write_message_into_index)]
    pub fn write_message(&self, message: &Message) -> Result<String, VCSIndexWritingError> {
        let key = run(self.write(message.clone()))
            .map_err(|error| VCSIndexWritingError::Io(error.into()))??;

        Ok(key.to_string())
    }

    /// Reads the Creator stored under `hash`, which is the hex a write answered with.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError`] if nothing is stored under `hash`, what is stored is not
    /// a creator, or the index fails.
    ///
    /// # FFI
    /// Blocks until the read is done.
    #[lazyffi(export = read_creator_from_index)]
    pub fn read_creator(&self, hash: &str) -> Result<Creator, VCSIndexReadingError> {
        let key = Hash::from_str(hash).map_err(|_| VCSIndexReadingError::Malformed)?;
        let object =
            run(self.read(key)).map_err(|error| VCSIndexReadingError::Io(error.into()))??;

        object
            .expect_creator()
            .map_err(|_| VCSIndexReadingError::Malformed)
    }

    /// Reads the Message stored under `hash`, which is the hex a write answered with.
    ///
    /// # Errors
    ///
    /// Returns [`VCSIndexReadingError`] if nothing is stored under `hash`, what is stored is not
    /// a message, or the index fails.
    ///
    /// # FFI
    /// Blocks until the read is done.
    #[lazyffi(export = read_message_from_index)]
    pub fn read_message(&self, hash: &str) -> Result<Message, VCSIndexReadingError> {
        let key = Hash::from_str(hash).map_err(|_| VCSIndexReadingError::Malformed)?;
        let object =
            run(self.read(key)).map_err(|error| VCSIndexReadingError::Io(error.into()))??;

        object
            .expect_message()
            .map_err(|_| VCSIndexReadingError::Malformed)
    }
}

/// The Variant `object` is, or a read that did not hold one.
fn expect_variant(object: VCSIndexObject) -> Result<Variant, VCSIndexError> {
    object
        .expect_variant()
        .map_err(|_| VCSIndexError::Malformed)
}

/// The Version `object` is, or a read that did not hold one.
fn expect_version(object: VCSIndexObject) -> Result<Version, VCSIndexError> {
    object
        .expect_version()
        .map_err(|_| VCSIndexError::Malformed)
}

/// Waits for `future` on a runtime of this call's own, so a synchronous export can drive an
/// asynchronous write or read.
fn run<T>(future: impl Future<Output = T>) -> Result<T, io::Error> {
    let runtime = tokio::runtime::Runtime::new()?;

    Ok(runtime.block_on(future))
}

/// The temporary name a write to `path` is laid down under before it is moved into place.
///
/// The name is unique among every write in this process, so two of them writing one path never
/// write through each other's file.
fn temporary_beside(path: &Path) -> Result<PathBuf, VCSIndexError> {
    /// How many temporaries this process has named, so that no two are the same.
    static NAMED: AtomicU64 = AtomicU64::new(0);

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(VCSIndexError::Malformed)?;
    let unique = NAMED.fetch_add(1, Ordering::Relaxed);

    Ok(path.with_file_name(format!(
        "{name}.{}.{unique}{TEMPORARY_SUFFIX}",
        std::process::id()
    )))
}

/// Every key named by a file under `directory`, which is laid out two levels deep by digest.
async fn collect(directory: &Path, keys: &mut BTreeSet<Hash>) -> Result<(), VCSIndexError> {
    for first in entries(directory).await? {
        if !is_directory(&first).await? {
            continue;
        }

        for second in entries(&first).await? {
            for file in entries(&second).await? {
                if let Some(name) = file.file_name().and_then(|name| name.to_str())
                    && let Ok(key) = Hash::from_str(name)
                {
                    keys.insert(key);
                }
            }
        }
    }

    Ok(())
}

/// The paths directly inside `directory`, or none when there is no directory there.
async fn entries(directory: &Path) -> Result<Vec<PathBuf>, VCSIndexError> {
    let mut entries = match tokio::fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };

    let mut paths = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        paths.push(entry.path());
    }

    Ok(paths)
}

/// Whether there is a directory at `path`.
async fn is_directory(path: &Path) -> Result<bool, VCSIndexError> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

impl From<rorolala_storage::Error> for VCSIndexError {
    fn from(error: rorolala_storage::Error) -> Self {
        match error {
            rorolala_storage::Error::NotFound(key) => Self::NotFound(key),
            rorolala_storage::Error::Corrupt(key) => Self::Corrupt(key),
            rorolala_storage::Error::Io(source) => Self::Io(source),
            _ => Self::Malformed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::transfer;
    use rorolala_utils_constants::{
        VAULT_CONFIG_PATH, VAULT_INDEX_DIR, WORKSPACE_DATA_DIR, WORKSPACE_INDEX_DIR,
    };
    use rorolala_utils_location::Locate;

    use super::{VCSIndex, VCSWrite, run};
    use crate::error::VCSIndexReadingError;
    use crate::{Creator, Message, Variant, Version};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-vcs-index-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// Where a layout names its index, with the `./` walked back out.
    fn laid_out(root: &Path, layout: &str) -> PathBuf {
        root.join(layout).components().collect()
    }

    /// An index rooted at a directory of its own.
    fn index(label: &str) -> VCSIndex {
        VCSIndex {
            path: scratch(label),
        }
    }

    /// A Variant that is progressive (nothing joined in).
    fn variant() -> Variant {
        Variant::new_bare_variant([0_u8; 32], [1_u8; 32], None, [0_u8; 32], [0_u8; 32], 0)
    }

    #[test]
    fn locating_nothing_hands_back_none() {
        let dir = scratch("absent");

        assert!(VCSIndex::locate(&dir).is_none(), "{dir:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_is_found_by_its_configuration_and_indexes_at_its_root() {
        let dir = scratch("vault");
        let nested = dir.join("deep").join("down");
        fs::create_dir_all(&nested).unwrap();
        fs::write(dir.join(VAULT_CONFIG_PATH), b"").unwrap();

        let index = VCSIndex::locate(&nested).unwrap();

        assert_eq!(index.get_root(), laid_out(&dir, VAULT_INDEX_DIR).as_path());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workspace_is_found_by_its_data_directory_and_indexes_inside_it() {
        let dir = scratch("workspace");
        let nested = dir.join("deep").join("down");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(dir.join(WORKSPACE_DATA_DIR)).unwrap();

        let index = VCSIndex::locate(&nested).unwrap();

        assert_eq!(
            index.get_root(),
            laid_out(&dir, WORKSPACE_INDEX_DIR).as_path()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_marker_nearest_the_directory_asked_about_wins() {
        // A Vault at the outer directory with a Workspace nested inside it: a run here is
        // in the Workspace, so it is the Workspace's index that is located.
        let vault = scratch("nearest");
        fs::write(vault.join(VAULT_CONFIG_PATH), b"").unwrap();
        let workspace = vault.join("inner");
        fs::create_dir_all(workspace.join(WORKSPACE_DATA_DIR)).unwrap();

        let index = VCSIndex::locate(&workspace).unwrap();

        assert_eq!(
            index.get_root(),
            laid_out(&workspace, WORKSPACE_INDEX_DIR).as_path()
        );

        let _ = fs::remove_dir_all(&vault);
    }

    #[test]
    fn a_vault_is_taken_where_one_directory_is_both() {
        let dir = scratch("both");
        fs::write(dir.join(VAULT_CONFIG_PATH), b"").unwrap();
        fs::create_dir_all(dir.join(WORKSPACE_DATA_DIR)).unwrap();

        let index = VCSIndex::locate(&dir).unwrap();

        assert_eq!(index.get_root(), laid_out(&dir, VAULT_INDEX_DIR).as_path());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_variant_round_trips_through_the_index() {
        let index = index("variant-round-trip");
        let variant = variant();

        let key = run(index.write(variant.clone())).unwrap().unwrap();
        let read = run(index.read(key)).unwrap().unwrap();

        assert_eq!(read, crate::VCSIndexObject::Variant(variant));

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn a_version_round_trips_through_the_index() {
        let index = index("version-round-trip");
        let version = Version::new_bare_version([2_u8; 32], 0);

        let key = run(index.write(version.clone())).unwrap().unwrap();
        let read = run(index.read(key)).unwrap().unwrap();

        assert_eq!(read, crate::VCSIndexObject::Version(version));

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn writing_one_object_twice_stores_it_once_under_one_key() {
        let index = index("idempotent");

        let first = run(index.write(variant())).unwrap().unwrap();
        let second = run(index.write(variant())).unwrap().unwrap();

        assert_eq!(first, second);

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn reading_a_key_nothing_is_stored_under_says_not_found() {
        let index = index("missing");
        let absent = variant().hash();

        let read = run(index.read(absent)).unwrap();

        assert!(matches!(read, Err(VCSIndexReadingError::NotFound { .. })));

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn an_object_moves_between_two_indexes_over_a_transfer() {
        let left = index("transfer-left");
        let right = index("transfer-right");
        let variant = variant();
        let key = run(left.write(variant.clone())).unwrap().unwrap();

        // The two ends are driven over a pair of buffers in one process, which is all a
        // transfer needs: the stream is the caller's, not the store's.
        let (one, other) = tokio::io::duplex(4096);
        let keys = [key];
        run(async {
            let (left_side, right_side) = tokio::join!(
                transfer::initiate(&left, one, &keys),
                transfer::respond(&right, other),
            );
            left_side.unwrap();
            right_side.unwrap();
        })
        .unwrap();

        assert!(run(right.holds_object(&key)).unwrap().unwrap());
        assert_eq!(
            run(right.read(key)).unwrap().unwrap(),
            crate::VCSIndexObject::Variant(variant)
        );
    }

    #[test]
    fn repacking_lays_loose_objects_into_one_pack() {
        let index = index("repack");
        let one = variant();
        let other = Version::new_bare_version([3_u8; 32], 0);
        run(index.write(one.clone())).unwrap().unwrap();
        run(index.write(other.clone())).unwrap().unwrap();
        assert!(!run(index.list_loose_keys()).unwrap().unwrap().is_empty());

        // The first repacking moves both loose objects into one pack; the second finds them there
        // already and changes nothing.
        assert!(run(index.repack()).unwrap().unwrap());
        assert_eq!(run(index.packs()).unwrap().unwrap(), vec![0]);
        assert!(run(index.list_loose_keys()).unwrap().unwrap().is_empty());
        assert!(!run(index.repack()).unwrap().unwrap());

        // What each object is does not change: every read that worked before works after.
        assert_eq!(
            run(index.read(one.hash())).unwrap().unwrap(),
            crate::VCSIndexObject::Variant(one)
        );
        assert_eq!(
            run(index.read(other.hash())).unwrap().unwrap(),
            crate::VCSIndexObject::Version(other)
        );

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn removing_a_packed_object_takes_it_out_of_the_pack() {
        let index = index("remove-packed");
        let kept = variant();
        let removed = Version::new_bare_version([4_u8; 32], 0);
        let kept_key = run(index.write(kept.clone())).unwrap().unwrap();
        let removed_key = run(index.write(removed)).unwrap().unwrap();
        run(index.repack()).unwrap().unwrap();

        run(index.remove_object(&removed_key)).unwrap().unwrap();

        assert!(!run(index.holds_object(&removed_key)).unwrap().unwrap());
        assert!(matches!(
            run(index.read(removed_key)).unwrap(),
            Err(VCSIndexReadingError::NotFound { .. })
        ));
        assert_eq!(
            run(index.read(kept_key)).unwrap().unwrap(),
            crate::VCSIndexObject::Variant(kept)
        );

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn a_creator_and_a_message_round_trip_through_the_index() {
        let index = index("text");
        let creator = Creator::try_from("Wei Cao").unwrap();
        let message = Message::try_from("made the first cut").unwrap();

        let creator_key = run(index.write(creator.clone())).unwrap().unwrap();
        let message_key = run(index.write(message.clone())).unwrap().unwrap();

        let read_creator = run(index.read(creator_key)).unwrap().unwrap();
        let read_message = run(index.read(message_key)).unwrap().unwrap();

        assert_eq!(read_creator, crate::VCSIndexObject::Creator(creator));
        assert_eq!(read_message, crate::VCSIndexObject::Message(message));

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn a_creator_and_a_message_survive_a_pack() {
        let index = index("text-pack");
        let creator = Creator::try_from("Wei Cao").unwrap();
        let message = Message::try_from("packed, still there").unwrap();
        run(index.write(creator.clone())).unwrap().unwrap();
        run(index.write(message.clone())).unwrap().unwrap();

        run(index.repack()).unwrap().unwrap();

        assert_eq!(run(index.list_loose_keys()).unwrap().unwrap(), Vec::new());
        assert_eq!(
            run(index.read(creator.hash())).unwrap().unwrap(),
            crate::VCSIndexObject::Creator(creator)
        );
        assert_eq!(
            run(index.read(message.hash())).unwrap().unwrap(),
            crate::VCSIndexObject::Message(message)
        );

        let _ = fs::remove_dir_all(index.get_root());
    }

    #[test]
    fn a_text_object_longer_than_it_may_be_is_refused() {
        let too_long = "x".repeat(300);

        assert!(Creator::try_from(too_long.clone()).is_err());
        assert!(Message::try_from(too_long).is_err());
    }

    #[test]
    fn a_text_object_only_round_trips_through_utf8() {
        // A name read from bytes that are not UTF-8 is refused rather than guessed at.
        assert!(Creator::try_from(vec![0xff_u8, 0xfe]).is_err());
        assert_eq!(
            Message::try_from("你好，世界".as_bytes().to_vec())
                .unwrap()
                .read_to_string(),
            "你好，世界"
        );
    }

    #[test]
    fn a_version_number_is_worked_out_by_tracing_the_chain() {
        let index = index("chain");

        // root -> variant -> version 0 -> variant -> version 1, and a merge off the second
        // variant that stands where the variant it merges stands.
        let root = Version::root();
        run(index.write(root.clone())).unwrap().unwrap();

        let first_variant = root.new_variant([0x11; 32], [0xaa; 32], [0xbb; 32]);
        run(index.write(first_variant.clone())).unwrap().unwrap();
        let first = first_variant.new_version();
        run(index.write(first.clone())).unwrap().unwrap();

        let second_variant = first.new_variant([0x22; 32], [0xcc; 32], [0xdd; 32]);
        run(index.write(second_variant.clone())).unwrap().unwrap();
        let second = second_variant.new_version();
        run(index.write(second.clone())).unwrap().unwrap();

        let merge = second_variant.new_merge_variant([0x33; 32], &first_variant);
        run(index.write(merge.clone())).unwrap().unwrap();
        let merged = merge.new_version();
        run(index.write(merged.clone())).unwrap().unwrap();

        // In memory, the number is carried as the objects are made: only `new_version` moves it.
        assert_eq!(root.version_num(), crate::ROOT_VERSION);
        assert_eq!(first.version_num(), 0);
        assert_eq!(second.version_num(), 1);
        assert_eq!(merge.base_version_num(), second_variant.base_version_num());
        assert_eq!(merged.version_num(), 1);

        // Out of the index, it is derived by walking the chain back to the root.
        assert_eq!(
            run(index.version_num(&root)).unwrap().unwrap(),
            crate::ROOT_VERSION
        );
        assert_eq!(run(index.version_num(&first)).unwrap().unwrap(), 0);
        assert_eq!(run(index.version_num(&second)).unwrap().unwrap(), 1);
        assert_eq!(run(index.version_num(&merged)).unwrap().unwrap(), 1);
        assert_eq!(run(index.variant_num(&second_variant)).unwrap().unwrap(), 0);
        assert_eq!(run(index.variant_num(&merge)).unwrap().unwrap(), 0);

        let _ = fs::remove_dir_all(index.get_root());
    }
}

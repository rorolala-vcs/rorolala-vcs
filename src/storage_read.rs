//! Reading a store, as the C ABI reads it.
//!
//! A store is content-addressed: what it holds is named by the hash of the content, so the listing
//! is keys and the extraction is one key in. What crosses is the keys `rola storage ls-storaged` and
//! `ls-manifests` list, and the one key `storage extract-file` takes out.
//!
//! Extraction writes the content where the caller names it. The store is the thing read; the path is
//! the caller's, and a file already there is refused rather than written over, which is the same
//! judgement the command makes.

use std::fmt;
use std::path::Path;
use std::str::FromStr as _;

use rorolala_storage::{Error as StorageError, Key, StorageSet};
use rorolala_utils_lazyffi::lazyffi;

/// What reading a store failed with.
///
/// # FFI
///
/// The tag says which case it is. `NotFound` carries the key nothing is stored under, `Exists` the
/// path that is already there, and `Read` the reason the store failed.
#[lazyffi(export = RolaStorageError)]
#[derive(Debug)]
pub enum StorageReadError {
    /// No Workspace or Vault holds the directory, so there is no store to read.
    NoStore,
    /// Nothing is stored under the key.
    NotFound {
        /// The key nothing is stored under, as the hex it is written by.
        key: String,
    },
    /// The destination the content was to be written to is already there.
    Exists {
        /// The path that is already there.
        path: String,
    },
    /// The key does not read as one this build knows.
    Malformed,
    /// The content is not a tree structure this build reads.
    NotAStructure,
    /// The store itself failed, or what it holds does not read.
    Read {
        /// Why it failed.
        reason: String,
    },
}

impl fmt::Display for StorageReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStore => formatter.write_str("no Workspace or Vault holds the directory"),
            Self::NotFound { key } => write!(formatter, "no object is stored under {key}"),
            Self::Exists { path } => write!(formatter, "`{path}` is already there"),
            Self::Malformed => formatter.write_str("the key is not one this build knows"),
            Self::NotAStructure => formatter.write_str("the content is not a tree structure"),
            Self::Read { reason } => write!(formatter, "the store could not be read: {reason}"),
        }
    }
}

impl std::error::Error for StorageReadError {}

/// The keys a store holds.
#[lazyffi(export = RolaStorageKeys)]
#[derive(Debug, Clone, Default)]
pub struct StorageKeys {
    /// The keys, as `blake3:<hex>`.
    keys: Vec<String>,
}

#[lazyffi(export = rola_)]
impl StorageKeys {
    /// How many keys the store answered with.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.keys.len()
    }

    /// The key at `index`, or an empty string past the end.
    #[must_use]
    pub fn at(&self, index: usize) -> String {
        self.keys.get(index).cloned().unwrap_or_default()
    }
}

/// Every object the store holding `directory` holds, as `rola storage ls-storaged` lists them.
///
/// # FFI
///
/// A `RolaStorageKeys *` the caller releases with `free_rola_storage_keys`, in the order the store
/// answers. A failure is a `RolaStorageError *` the caller releases with `free_rola_storage_error`.
///
/// # Errors
///
/// Returns [`StorageReadError::NoStore`] when no Workspace or Vault holds `directory`, and
/// [`StorageReadError::Read`] when the store cannot be listed.
#[lazyffi(export = rola_storage_objects)]
pub fn objects(directory: &Path) -> Result<StorageKeys, StorageReadError> {
    let set = stores_of(directory)?;
    let runtime = runtime()?;
    let keys = runtime
        .block_on(set.list_exist_keys())
        .map_err(storage_error)?;

    Ok(StorageKeys {
        keys: keys.iter().map(ToString::to_string).collect(),
    })
}

/// Every content key the store holding `directory` keeps as a manifest, as
/// `rola storage ls-manifests` lists them.
///
/// # FFI
///
/// A `RolaStorageKeys *` the caller releases with `free_rola_storage_keys`, in the order the store
/// answers. A failure is a `RolaStorageError *` the caller releases with `free_rola_storage_error`.
///
/// # Errors
///
/// Returns [`StorageReadError::NoStore`] when no Workspace or Vault holds `directory`, and
/// [`StorageReadError::Read`] when the manifests cannot be listed.
#[lazyffi(export = rola_storage_manifests)]
pub fn manifests(directory: &Path) -> Result<StorageKeys, StorageReadError> {
    let set = stores_of(directory)?;
    let runtime = runtime()?;
    let keys = runtime
        .block_on(set.list_manifest_keys())
        .map_err(storage_error)?;

    Ok(StorageKeys {
        keys: keys.iter().map(ToString::to_string).collect(),
    })
}

/// Writes the content stored under `hash` to `destination`.
///
/// The key is the full one a listing answered with, as hex with or without the `blake3:` name in
/// front; a short hash is the command line's to resolve against a listing, and not this.
///
/// # FFI
///
/// `Ok` carries nothing; a failure is a `RolaStorageError *` the caller releases with
/// `free_rola_storage_error`.
///
/// # Errors
///
/// Returns [`StorageReadError::NoStore`] when no Workspace or Vault holds `directory`,
/// [`StorageReadError::Malformed`] when `hash` is not a key this build reads,
/// [`StorageReadError::Exists`] when `destination` is already there, and
/// [`StorageReadError::NotFound`] when the store does not hold the key, or
/// [`StorageReadError::Read`] when the write fails.
#[lazyffi(export = rola_storage_extract)]
pub fn extract(directory: &Path, hash: &str, destination: &Path) -> Result<(), StorageReadError> {
    let set = stores_of(directory)?;
    let key = Key::from_str(hash).map_err(|_| StorageReadError::Malformed)?;

    if destination.exists() {
        return Err(StorageReadError::Exists {
            path: destination.display().to_string(),
        });
    }

    let runtime = runtime()?;
    runtime
        .block_on(set.extract_file(&key, destination))
        .map_err(storage_error)
}

/// The chunks a manifest names, when the content under `hash` was stored chunked.
///
/// A manifest is what a chunked object is put back together from; a caller that has the key but
/// not the chunks cannot produce the content. A manifest with no chunks means the content is kept
/// whole, which is a fact about how it was stored rather than a failure.
///
/// # FFI
///
/// A `RolaStorageManifest *` the caller releases with `free_rola_storage_manifest`. A manifest with
/// no chunks means the content is kept whole. A failure is a `RolaStorageError *` the caller
/// releases with `free_rola_storage_error`.
///
/// # Errors
///
/// Returns [`StorageReadError::NoStore`] when no Workspace or Vault holds `directory`,
/// [`StorageReadError::Malformed`] when `hash` is not a key this build reads,
/// [`StorageReadError::NotFound`] when the store does not hold the key, or
/// [`StorageReadError::Read`] when the manifest cannot be read.
#[lazyffi(export = rola_storage_manifest)]
pub fn manifest(directory: &Path, hash: &str) -> Result<StorageManifest, StorageReadError> {
    let set = stores_of(directory)?;
    let key = Key::from_str(hash).map_err(|_| StorageReadError::Malformed)?;
    let runtime = runtime()?;

    let manifest = runtime
        .block_on(set.manifest_of(&key))
        .map_err(storage_error)?;

    let chunks = manifest.map_or_else(Vec::new, |manifest| {
        manifest
            .chunks()
            .iter()
            .map(|chunk| StorageChunk {
                key: chunk.key().to_string(),
                len: chunk.len(),
            })
            .collect()
    });

    Ok(StorageManifest { chunks })
}

/// The structure stored under `hash`, as the entries a tree version names.
///
/// A tree entry's storage key is the structure itself, written whole; a caller that has the key but
/// not the format has no way to reach the files inside it. The bytes are decoded here and cross as
/// entries, the same shape [`tree_entries`](crate::tree_entries) reads from hex.
///
/// # FFI
///
/// A `RolaTreeEntries *` the caller releases with `free_rola_tree_entries`. A failure is a
/// `RolaStorageError *` the caller releases with `free_rola_storage_error`.
///
/// # Errors
///
/// Returns [`StorageReadError::NoStore`] when no Workspace or Vault holds `directory`,
/// [`StorageReadError::Malformed`] when `hash` is not a key this build reads,
/// [`StorageReadError::NotFound`] when the store does not hold the key,
/// [`StorageReadError::NotAStructure`] when the content does not decode as a tree structure, and
/// [`StorageReadError::Read`] when the content cannot be read.
#[lazyffi(export = rola_tree_structure)]
pub fn tree_structure(
    directory: &Path,
    hash: &str,
) -> Result<crate::TreeEntries, StorageReadError> {
    let set = stores_of(directory)?;
    let key = Key::from_str(hash).map_err(|_| StorageReadError::Malformed)?;
    let runtime = runtime()?;

    let Some(bytes) = runtime.block_on(set.content(&key)).map_err(storage_error)? else {
        return Err(StorageReadError::NotFound {
            key: key.to_string(),
        });
    };

    // A structure is one object, so what was read is one range or one buffer: the decoder takes it
    // as it lies rather than through a copy made to give it one run of bytes.
    let encoded = bytes.as_slice().ok_or(StorageReadError::NotAStructure)?;

    crate::TreeEntries::decode(encoded).ok_or(StorageReadError::NotAStructure)
}

/// The chunks of a chunked object.
#[lazyffi(export = RolaStorageManifest)]
#[derive(Debug, Clone)]
pub struct StorageManifest {
    /// The chunks, in the order the content is put back together.
    chunks: Vec<StorageChunk>,
}

#[lazyffi(export = rola_)]
impl StorageManifest {
    /// How many chunks the manifest names, or 0 when the content is kept whole.
    #[must_use]
    pub const fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// The chunk at `index`, or null past the end.
    #[must_use]
    pub fn chunk_at(&self, index: usize) -> Option<StorageChunk> {
        self.chunks.get(index).cloned()
    }
}

/// One chunk of a manifest.
#[lazyffi(export = RolaStorageChunk)]
#[derive(Debug, Clone)]
pub struct StorageChunk {
    /// The key the chunk is stored under.
    key: String,
    /// How many bytes the chunk holds.
    len: u64,
}

#[lazyffi(export = rola_)]
impl StorageChunk {
    /// The key the chunk is stored under.
    #[must_use]
    pub fn key(&self) -> String {
        self.key.clone()
    }

    /// How many bytes the chunk holds.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Whether the chunk holds no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A runtime of this call's own, so a synchronous export can drive an asynchronous store.
fn runtime() -> Result<tokio::runtime::Runtime, StorageReadError> {
    tokio::runtime::Runtime::new().map_err(|error| StorageReadError::Read {
        reason: error.to_string(),
    })
}

/// The stores `directory` works on, as a reader meets them.
fn stores_of(directory: &Path) -> Result<StorageSet, StorageReadError> {
    crate::storage_set::for_directory(directory)
        .map_err(|error| StorageReadError::Read {
            reason: error.to_string(),
        })?
        .ok_or(StorageReadError::NoStore)
}

/// What a store's failure is reported as.
fn storage_error(error: StorageError) -> StorageReadError {
    match error {
        StorageError::NotFound(key) => StorageReadError::NotFound {
            key: key.to_string(),
        },
        other => StorageReadError::Read {
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_storage::Codec;
    use rorolala_storage::RorolalaStorage;
    use rorolala_tree::{FileMode, TreeEntry, TreeStructure};
    use rorolala_utils_location::Locate as _;

    use super::{StorageReadError, extract, manifest, manifests, objects, tree_structure};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let name = format!(
            "rorolala-storage-read-{}-{label}-{}",
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

    /// Puts `content` in the store at `dir`, answering the key it is kept under.
    fn kept(dir: &Path, content: &[u8]) -> String {
        let store = RorolalaStorage::locate(dir).expect("a store");
        let file = dir.join("content.bin");
        fs::write(&file, content).unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();

        runtime
            .block_on(rorolala_storage::store_file(&store, &file))
            .unwrap()
            .to_string()
    }

    #[test]
    fn a_directory_with_no_store_is_no_store() {
        let dir = scratch("absent");

        assert!(matches!(objects(&dir), Err(StorageReadError::NoStore)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stored_object_is_listed_by_its_key() {
        let dir = scratch("objects");
        let _ = RorolalaStorage::create(&dir);
        let key = kept(&dir, b"stored content");

        let keys = objects(&dir).unwrap();

        assert!(
            (0..keys.count()).any(|index| keys.at(index) == key),
            "{keys:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_small_object_is_not_a_manifest() {
        let dir = scratch("manifests");
        let _ = RorolalaStorage::create(&dir);
        let _ = kept(&dir, b"stored content");

        assert_eq!(manifests(&dir).unwrap().count(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn content_is_extracted_where_it_is_asked_for_once() {
        let dir = scratch("extract");
        let _ = RorolalaStorage::create(&dir);
        let key = kept(&dir, b"stored content");
        let target = dir.join("out.bin");

        extract(&dir, &key, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"stored content");

        // The same content written twice into one place is a mistake worth being told about rather
        // than a silent replacement.
        assert!(matches!(
            extract(&dir, &key, &target),
            Err(StorageReadError::Exists { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_that_does_not_read_is_malformed() {
        let dir = scratch("malformed");
        let _ = RorolalaStorage::create(&dir);
        let target = dir.join("out.bin");

        assert!(matches!(
            extract(&dir, "not a key", &target),
            Err(StorageReadError::Malformed)
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_nothing_is_stored_under_is_not_found() {
        let dir = scratch("not-found");
        let _ = RorolalaStorage::create(&dir);
        let target = dir.join("out.bin");
        let missing = format!("blake3:{}", "11".repeat(32));

        assert!(matches!(
            extract(&dir, &missing, &target),
            Err(StorageReadError::NotFound { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn whole_content_has_no_manifest() {
        let dir = scratch("whole");
        let _ = RorolalaStorage::create(&dir);
        let key = kept(&dir, b"stored whole");

        assert_eq!(manifest(&dir, &key).unwrap().chunk_count(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_structure_reads_back_as_entries() {
        let dir = scratch("structure");
        let store = RorolalaStorage::create(&dir);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let content = runtime
            .block_on(store.write_object(b"inside", Codec::Raw))
            .unwrap();
        let structure =
            TreeStructure::build(None, [TreeEntry::file("a", FileMode::Regular, content)]).unwrap();
        let key = runtime
            .block_on(store.write_object(&structure.encode(), Codec::Raw))
            .unwrap();

        let entries = tree_structure(&dir, &key.to_string()).unwrap();

        assert_eq!(entries.count(), 1);
        assert_eq!(entries.at(0).unwrap().path(), "a");

        let _ = fs::remove_dir_all(&dir);
    }
}

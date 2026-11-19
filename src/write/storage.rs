//! Putting content into a store, and taking it back out.

use std::path::Path;

use rorolala_storage::{
    Chunking, Codec, Key, RorolalaStorage, StorageBackend as _, policy as chunking_policy,
    store_file,
};
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use serde::Serialize;

use crate::write::WriteError;

/// What a store answered when content went in.
#[derive(Debug, Clone, Serialize)]
pub struct Stored {
    /// The key the content is kept under.
    pub key: Key,
    /// Whether the content is kept as a manifest of chunks.
    pub chunked: bool,
}

/// Puts a file's content into the store, cutting it by `policy` when one is named.
///
/// # Errors
///
/// Returns [`WriteError::Precondition`] when `policy` names no cut this build carries, and
/// [`WriteError::Storage`] when the store refuses the content.
pub async fn put_file(
    store: &RorolalaStorage,
    file: &Path,
    policy: Option<&str>,
) -> Result<Stored, WriteError> {
    let told = match policy {
        Some(name) => {
            let found = chunking_policy(name).ok_or_else(|| WriteError::Precondition {
                what: format!("chunking policy `{name}`"),
                reason: "this build carries no such policy".to_owned(),
            })?;
            store.clone().with_forced_cut(Chunking::Policy(found.name))
        }
        None => store.clone(),
    };

    let key = store_file(&told, file)
        .await
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })?;
    let chunked = store
        .holds_manifest(&key)
        .await
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })?;

    Ok(Stored { key, chunked })
}

/// Puts content in memory into the store, keeping it as one object.
///
/// # Errors
///
/// Returns [`WriteError::Storage`] when the store refuses the content.
pub async fn put_bytes(store: &RorolalaStorage, bytes: &[u8]) -> Result<Stored, WriteError> {
    let key = store
        .write_object(bytes, Codec::Raw)
        .await
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })?;

    Ok(Stored {
        key,
        chunked: false,
    })
}

/// Takes the content stored under `key` out to `destination`.
///
/// # Errors
///
/// Returns [`WriteError::Storage`] when the store does not hold the key or cannot write it.
pub async fn extract_file(
    store: &RorolalaStorage,
    key: &Key,
    destination: &Path,
) -> Result<(), WriteError> {
    store
        .extract_file(key, destination)
        .await
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })
}

/// Puts a file's content into the store a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, `{"key":"blake3:...","chunked":false}`,
/// or a `RolaWriteError *` the caller releases with `free_rola_write_error`. `policy` is the empty
/// string when the store is to choose the cut.
///
/// # Errors
///
/// Returns [`WriteError::NoStore`] when no Workspace or Vault holds `directory`,
/// [`WriteError::Precondition`] when `policy` names no cut, and [`WriteError::Storage`] when the
/// content cannot be stored.
#[lazyffi(export = rola_write_storage_put_file)]
pub fn put_file_to(directory: &Path, file: &Path, policy: &str) -> Result<String, WriteError> {
    let store = RorolalaStorage::locate(directory).ok_or(WriteError::NoStore)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let policy = (!policy.is_empty()).then_some(policy);
    let stored = runtime
        .block_on(put_file(&store, file, policy))
        .map_err(|error| match error {
            WriteError::Precondition { what, reason } => WriteError::Precondition { what, reason },
            other => other,
        })?;

    serde_json::to_string(&stored).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{put_file, put_file_to};
    use crate::write::WriteError;

    #[test]
    fn a_file_is_stored_under_its_content_key() {
        let dir = std::env::temp_dir().join(format!("rola-write-storage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = rorolala_storage::RorolalaStorage::create(&dir);
        let file = dir.join("content.bin");
        fs::write(&file, b"stored content").unwrap();

        let stored = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_file(&store, &file, None))
            .unwrap();

        assert!(!stored.chunked);
        assert_eq!(stored.key.to_string().len(), 71);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_policy_is_refused() {
        let dir = std::env::temp_dir().join(format!("rola-write-policy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = rorolala_storage::RorolalaStorage::create(&dir);
        let file = dir.join("content.bin");
        fs::write(&file, b"content").unwrap();

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_file(&store, &file, Some("no-such-policy")))
            .unwrap_err();

        assert!(matches!(error, WriteError::Precondition { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_export_names_the_stored_key() {
        let dir = std::env::temp_dir().join(format!("rola-write-export-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_storage::RorolalaStorage::create(&dir);
        let file = dir.join("content.bin");
        fs::write(&file, b"content").unwrap();

        let json = put_file_to(&dir, &file, "").unwrap();

        assert!(json.contains("blake3:"));

        let _ = fs::remove_dir_all(&dir);
    }
}

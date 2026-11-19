//! Putting content into a store, and taking it back out.

use std::path::Path;

use rorolala_storage::{Chunking, Codec, Key, StorageSet, policy as chunking_policy};
use rorolala_utils_lazyffi::lazyffi;
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

/// Puts a file's content into the store a set writes to, cutting it by `policy` when one is named.
///
/// # Errors
///
/// Returns [`WriteError::Precondition`] when `policy` names no cut this build carries, and
/// [`WriteError::Storage`] or [`WriteError::ReadOnly`] when the store refuses the content.
pub async fn put_file(
    set: &StorageSet,
    file: &Path,
    policy: Option<&str>,
) -> Result<Stored, WriteError> {
    let cut = match policy {
        Some(name) => {
            let found = chunking_policy(name).ok_or_else(|| WriteError::Precondition {
                what: format!("chunking policy `{name}`"),
                reason: "this build carries no such policy".to_owned(),
            })?;
            Some(Chunking::Policy(found.name))
        }
        None => None,
    };

    let key = set
        .put_file(file, cut, None)
        .await
        .map_err(storage_failure)?;
    let chunked = set.holds_manifest(&key).await.map_err(storage_failure)?;

    Ok(Stored { key, chunked })
}

/// Puts content in memory into the store a set writes to, keeping it as one object.
///
/// # Errors
///
/// Returns [`WriteError::Storage`] or [`WriteError::ReadOnly`] when the store refuses the content.
pub async fn put_bytes(set: &StorageSet, bytes: &[u8]) -> Result<Stored, WriteError> {
    let key = set
        .write_object(bytes, Codec::Raw)
        .await
        .map_err(storage_failure)?;

    Ok(Stored {
        key,
        chunked: false,
    })
}

/// Takes the content stored under `key` out to `destination`, from whichever store holds it.
///
/// # Errors
///
/// Returns [`WriteError::Storage`] when no store holds the key or cannot write it.
pub async fn extract_file(
    set: &StorageSet,
    key: &Key,
    destination: &Path,
) -> Result<(), WriteError> {
    set.extract_file(key, destination)
        .await
        .map_err(storage_failure)
}

/// Puts a file's content into the store a directory works on.
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
    let set = crate::storage_set::for_directory(directory)
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })?
        .ok_or(WriteError::NoStore)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
        reason: error.to_string(),
    })?;
    let policy = (!policy.is_empty()).then_some(policy);
    let stored = runtime
        .block_on(put_file(&set, file, policy))
        .map_err(|error| match error {
            WriteError::Precondition { what, reason } => WriteError::Precondition { what, reason },
            other => other,
        })?;

    serde_json::to_string(&stored).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

/// A storage failure as the write error a caller branches on.
fn storage_failure(error: rorolala_storage::Error) -> WriteError {
    match error {
        rorolala_storage::Error::ReadOnly { path } => WriteError::ReadOnly {
            path: path.display().to_string(),
        },
        other => WriteError::Storage {
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_storage::StorageSet;

    use super::{put_file, put_file_to};
    use crate::write::WriteError;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-write-storage-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// The set a directory that is a store works on.
    fn set_at(root: &std::path::Path) -> StorageSet {
        StorageSet::sole(rorolala_storage::RorolalaStorage::create(root))
    }

    #[test]
    fn a_file_is_stored_under_its_content_key() {
        let dir = scratch("stored");
        let set = set_at(&dir);
        let file = dir.join("content.bin");
        fs::write(&file, b"stored content").unwrap();

        let stored = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_file(&set, &file, None))
            .unwrap();

        assert!(!stored.chunked);
        assert_eq!(stored.key.to_string().len(), 71);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_policy_is_refused() {
        let dir = scratch("policy");
        let set = set_at(&dir);
        let file = dir.join("content.bin");
        fs::write(&file, b"content").unwrap();

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(put_file(&set, &file, Some("no-such-policy")))
            .unwrap_err();

        assert!(matches!(error, WriteError::Precondition { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_export_names_the_stored_key() {
        let dir = scratch("export");
        let _ = rorolala_storage::RorolalaStorage::create(&dir);
        let file = dir.join("content.bin");
        fs::write(&file, b"content").unwrap();

        let json = put_file_to(&dir, &file, "").unwrap();

        assert!(json.contains("blake3:"));

        let _ = fs::remove_dir_all(&dir);
    }
}

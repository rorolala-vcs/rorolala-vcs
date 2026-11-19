//! One read the C ABI asks for by name, so a caller that only reads JSON has one call to make.
//!
//! The per-surface exports stay: a caller that wants one thing asks for it and pays for nothing
//! else. This is the aggregate for a caller that is about to read several, and for the cancellation
//! a long read is given.

use std::fmt;
use std::path::{Path, PathBuf};

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use serde::Deserialize;

use crate::write::Cancellation;

/// Why an aggregate read did not produce a document.
#[lazyffi(export = RolaV2ReadError)]
#[derive(Debug)]
pub enum ReadError {
    /// The request is not one this build knows, or not one of its fields reads.
    InvalidRequest {
        /// Why it did not read.
        reason: String,
    },
    /// No Workspace holds the directory.
    NoWorkspace,
    /// No Layout is there.
    NoLayout,
    /// No store is there.
    NoStore,
    /// No index is there.
    NoIndex,
    /// What the request names is not there.
    NotFound,
    /// What is there did not read.
    Unreadable {
        /// Why it did not.
        reason: String,
    },
    /// The caller asked the read to stop.
    Cancelled,
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest { reason } | Self::Unreadable { reason } => {
                formatter.write_str(reason)
            }
            Self::NoWorkspace => formatter.write_str("no Workspace holds the directory"),
            Self::NoLayout => formatter.write_str("no Layout is there"),
            Self::NoStore => formatter.write_str("no store is there"),
            Self::NoIndex => formatter.write_str("no index is there"),
            Self::NotFound => formatter.write_str("what the request names is not there"),
            Self::Cancelled => formatter.write_str("the read was cancelled"),
        }
    }
}

impl std::error::Error for ReadError {}

/// One read, as a request document names it.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Request {
    /// The Layouts a place holds, as names.
    Layouts { path: String },
    /// One Layout's entries.
    LayoutEntries {
        path: String,
        layout: Option<String>,
    },
    /// The working tree beside the Layout it works from.
    TreeStatus { path: String },
    /// The keys a store holds.
    StorageObjects { path: String },
    /// The content keys a store keeps as manifests.
    StorageManifests { path: String },
    /// One manifest's chunks.
    Manifest { path: String, hash: String },
    /// One stored tree structure.
    TreeStructure { path: String, hash: String },
    /// The three listings a VCS index holds.
    VcsIndexList { path: String },
    /// The Vaults a daemon serves.
    DaemonVaults,
}

/// Runs one read named by a request document.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`, or a `RolaV2ReadError *` the caller
/// releases with `free_rola_v2read_error`. The request is one object,
/// `{"kind":"tree_status","path":"..."}`, and the cancellation is checked before the read starts.
///
/// # Errors
///
/// Returns [`ReadError::InvalidRequest`] when the document does not read, [`ReadError::Cancelled`]
/// when the caller has asked to stop, and the failure the named read answers with.
#[lazyffi(export = rola_v2_read_json)]
pub fn read_json(
    directory: &Path,
    request: &str,
    cancellation: &Cancellation,
) -> Result<String, ReadError> {
    if cancellation.is_cancelled() {
        return Err(ReadError::Cancelled);
    }

    let request: Request =
        serde_json::from_str(request).map_err(|error| ReadError::InvalidRequest {
            reason: error.to_string(),
        })?;

    match request {
        Request::Layouts { path } => Ok(crate::layout_read::layouts_json(&at(directory, &path))),
        Request::LayoutEntries { path, layout } => crate::layout_read::layout_entries_json(
            &at(directory, &path),
            layout.as_deref().unwrap_or_default(),
        )
        .map_err(layout_error),
        Request::TreeStatus { path } => {
            let json = crate::status::tree_status_json(&at(directory, &path));
            if json == "null" {
                Err(ReadError::NoWorkspace)
            } else {
                Ok(json)
            }
        }
        Request::StorageObjects { path } => {
            crate::storage_read::objects_json(&at(directory, &path)).map_err(storage_error)
        }
        Request::StorageManifests { path } => {
            crate::storage_read::manifests_json(&at(directory, &path)).map_err(storage_error)
        }
        Request::Manifest { path, hash } => {
            crate::storage_read::manifest_json(&at(directory, &path), &hash).map_err(storage_error)
        }
        Request::TreeStructure { path, hash } => {
            crate::storage_read::tree_structure_json(&at(directory, &path), &hash)
                .map_err(storage_error)
        }
        Request::VcsIndexList { path } => {
            let index = VCSIndex::locate(&at(directory, &path)).ok_or(ReadError::NoIndex)?;
            let json = index.list_json();
            if json == "null" {
                Err(ReadError::Unreadable {
                    reason: "the index could not be listed".to_owned(),
                })
            } else {
                Ok(json)
            }
        }
        Request::DaemonVaults => Ok(crate::config_read::daemon_vaults_json()),
    }
}

/// The path a request names, read against the directory the call was made in.
fn at(directory: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);

    if path.is_absolute() || path.as_os_str().is_empty() {
        if path.as_os_str().is_empty() {
            directory.to_path_buf()
        } else {
            path.to_path_buf()
        }
    } else {
        directory.join(path)
    }
}

/// The failure a Layout read carried.
fn layout_error(error: crate::layout_read::LayoutReadError) -> ReadError {
    match error {
        crate::layout_read::LayoutReadError::NoWorkspace => ReadError::NoWorkspace,
        crate::layout_read::LayoutReadError::NoLayout => ReadError::NoLayout,
        crate::layout_read::LayoutReadError::NotCached
        | crate::layout_read::LayoutReadError::Missing => ReadError::NotFound,
        crate::layout_read::LayoutReadError::Read { reason } => ReadError::Unreadable { reason },
    }
}

/// The failure a storage read carried.
fn storage_error(error: crate::storage_read::StorageReadError) -> ReadError {
    match error {
        crate::storage_read::StorageReadError::NoStore => ReadError::NoStore,
        crate::storage_read::StorageReadError::NotFound { .. } => ReadError::NotFound,
        other => ReadError::Unreadable {
            reason: format!("{other:?}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{ReadError, read_json};
    use crate::write::Cancellation;

    #[test]
    fn a_layout_listing_reads_through_the_one_call() {
        let dir = std::env::temp_dir().join(format!("rola-read-v2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_workspace::Workspace::create(&dir);

        let json = read_json(
            &dir,
            r#"{"kind":"layouts","path":""}"#,
            &Cancellation::new(),
        )
        .unwrap();

        assert!(json.contains("main"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_read_answers_cancelled() {
        let cancellation = Cancellation::new();
        cancellation.cancel();

        let error = read_json(
            std::path::Path::new("."),
            r#"{"kind":"daemon_vaults"}"#,
            &cancellation,
        )
        .unwrap_err();

        assert!(matches!(error, ReadError::Cancelled));
    }
}

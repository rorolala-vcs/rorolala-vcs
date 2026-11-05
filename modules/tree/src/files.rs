//! Writing a tree's own files whole.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::TreeError;

/// Writes `bytes` to `path` whole, by writing beside it and moving it into place.
///
/// A reader that looks while a write is going finds the old file or the new one, never half of
/// either: what a run was doing when it stopped is then something a later reading can say.
///
/// # Errors
///
/// Returns [`TreeError::Io`] when the file cannot be written or moved into place.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), TreeError> {
    /// How many temporaries this process has named, so that no two are the same.
    static NAMED: AtomicU64 = AtomicU64::new(0);

    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| TreeError::Io(path.display().to_string()))?;
    let temporary = path.with_file_name(format!(
        "{name}.{}.{}.tmp",
        std::process::id(),
        NAMED.fetch_add(1, Ordering::Relaxed)
    ));

    fs::write(&temporary, bytes)?;

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);

        return Err(error.into());
    }

    Ok(())
}

/// The text of `path`, or nothing when there is no file there.
///
/// # Errors
///
/// Returns [`TreeError::Io`] when the file is there and cannot be read.
pub fn read_text(path: &Path) -> Result<Option<String>, TreeError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Reads `path` as bytes, or answers nothing when there is no file there.
///
/// # Errors
///
/// Returns [`TreeError::Io`] when the file is there and cannot be read.
pub fn read(path: &Path) -> Result<Option<Vec<u8>>, TreeError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Removes `path`, answering whether there was one.
///
/// # Errors
///
/// Returns [`TreeError::Io`] when the file is there and cannot be removed.
pub fn remove(path: &Path) -> Result<bool, TreeError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

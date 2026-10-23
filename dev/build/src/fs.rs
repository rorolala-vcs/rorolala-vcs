//! Copying and removing, with the file named when it cannot be done.
//!
//! What an export is made of is files moved from where cargo and .NET put them to where a hand-off
//! reads them, so the same handful of operations is used all over it; naming the pair in the failure
//! is the difference between a build that says where it stopped and one that says "denied".

use std::path::{Path, PathBuf};

use crate::Error;

/// Copies a file, making the directory it goes into when it is not there.
///
/// # Errors
///
/// Reports [`Error::Io`] naming both paths when it cannot be done.
pub fn copy_file(from: &Path, to: &Path) -> Result<(), Error> {
    if let Some(directory) = to.parent() {
        create_directory(directory)?;
    }

    std::fs::copy(from, to).map_err(|error| Error::Io {
        doing: "copying a file",
        detail: format!("{} -> {}: {error}", from.display(), to.display()),
    })?;

    Ok(())
}

/// Copies a whole directory under another one, entry by entry.
///
/// # Errors
///
/// Reports [`Error::Io`] naming both paths when it cannot be done.
pub fn copy_directory(from: &Path, to: &Path) -> Result<(), Error> {
    create_directory(to)?;

    for entry in read_entries(from)? {
        let path = entry.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        let destination = to.join(name);

        if path.is_dir() {
            copy_directory(&path, &destination)?;
        } else {
            copy_file(&path, &destination)?;
        }
    }

    Ok(())
}

/// Creates a directory and everything above it.
///
/// # Errors
///
/// Reports [`Error::Io`] naming the directory when it cannot be created.
pub fn create_directory(directory: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(directory).map_err(|error| Error::Io {
        doing: "creating a directory",
        detail: format!("{}: {error}", directory.display()),
    })
}

/// Removes a file or a directory tree, saying nothing when it is not there.
///
/// A clean removes what a build produced, and a build that produced nothing is one that is already
/// clean rather than a failure.
///
/// # Errors
///
/// Reports [`Error::Io`] naming the path when it is there and cannot be removed.
pub fn remove(path: &Path) -> Result<(), Error> {
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };

    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Io {
            doing: "removing",
            detail: format!("{}: {error}", path.display()),
        }),
    }
}

/// The entries of a directory.
///
/// # Errors
///
/// Reports [`Error::Io`] naming the directory when it cannot be read.
pub fn read_entries(directory: &Path) -> Result<Vec<std::fs::DirEntry>, Error> {
    let entries = std::fs::read_dir(directory).map_err(|error| Error::Io {
        doing: "reading a directory",
        detail: format!("{}: {error}", directory.display()),
    })?;

    entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Error::Io {
            doing: "reading a directory",
            detail: error.to_string(),
        })
}

/// The directories among a directory's entries.
///
/// # Errors
///
/// Reports [`Error::Io`] naming the directory when it cannot be read.
pub fn read_directories(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(read_entries(directory)?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect())
}

/// The files among a directory's entries.
///
/// # Errors
///
/// Reports [`Error::Io`] naming the directory when it cannot be read.
pub fn read_files(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(read_entries(directory)?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect())
}

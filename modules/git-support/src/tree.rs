//! Placing a tree material, and reading a tree on disk against the structure it was placed from.
//!
//! A tree's content is a structure naming the files under its path, so placing one is writing every
//! path the structure names. What is read back is compared path by path rather than by re-scanning
//! the directory: a scan would need the ignore rules, which are the `rola` program's to read, and
//! the tree's format already allows a directory to hold paths its structure does not name.

use std::fs;
use std::io;
use std::path::Path;

use rola_define_format::ContentHash;
use rorolala_tree::{EntryKind, TreeStructure};

use crate::content::{ContentSource, digest_of};
use crate::error::Error;

/// One place where a tree on disk is not the structure it was placed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// The path inside the tree, `/` separated, as the structure names it.
    pub at: String,
    /// What the place is.
    pub kind: DifferenceKind,
}

/// What a place a tree differs at is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferenceKind {
    /// The structure names the path and nothing is there.
    Missing,
    /// Something is there, and it is not what the structure names.
    Modified,
}

/// The structure the content `content` is, read through `source`.
///
/// # Errors
///
/// Returns [`Error::Unavailable`] when no source holds the content, [`Error::Io`] when it cannot be
/// read, and [`Error::Structure`] when what is held is not a structure this build reads.
pub fn structure(source: &dyn ContentSource, content: ContentHash) -> Result<TreeStructure, Error> {
    let bytes = source.read(content)?;

    // A structure is one object, so what a source hands back is one range or one buffer. A run of
    // more than one piece is content that is not a structure this build reads.
    let encoded = bytes.as_slice().ok_or_else(|| Error::Structure {
        content: content.to_token(),
    })?;

    TreeStructure::decode(encoded).map_err(|_| Error::Structure {
        content: content.to_token(),
    })
}

/// Every place `root` is not the tree `structure` names.
///
/// The paths the structure names are read one by one. A path the structure does not name is not
/// walked: a tree's directory may hold files the structure never recorded, and deciding whether
/// they are work is the rules' business rather than a placement's.
///
/// # Errors
///
/// Returns [`Error::Io`] when a place cannot be read.
pub fn differences(root: &Path, structure: &TreeStructure) -> Result<Vec<Difference>, Error> {
    let mut differences = Vec::new();

    for entry in structure.entries() {
        let at = root.join(entry.path());

        let kind = match entry.kind() {
            EntryKind::Directory => match fs::symlink_metadata(&at) {
                Ok(metadata) if metadata.is_dir() => continue,
                Ok(_) => DifferenceKind::Modified,
                Err(error) if error.kind() == io::ErrorKind::NotFound => DifferenceKind::Missing,
                Err(error) => return Err(Error::Io(error)),
            },
            EntryKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };

                match fs::symlink_metadata(&at) {
                    Ok(metadata) if metadata.is_file() => {
                        if digest_of(&at)? == ContentHash::from_digest(*storage.digest()) {
                            continue;
                        }

                        DifferenceKind::Modified
                    }
                    Ok(_) => DifferenceKind::Modified,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        DifferenceKind::Missing
                    }
                    Err(error) => return Err(Error::Io(error)),
                }
            }
            EntryKind::Symlink => {
                let Some(target) = entry.target() else {
                    continue;
                };

                match fs::read_link(&at) {
                    Ok(link) if link == Path::new(target) => continue,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        DifferenceKind::Missing
                    }
                    // A path that is not a link at all answers with the platform's own refusal
                    // rather than with a missing file, so anything else is a place holding
                    // something other than the link the structure names.
                    _ => DifferenceKind::Modified,
                }
            }
        };

        differences.push(Difference {
            at: entry.path().to_owned(),
            kind,
        });
    }

    Ok(differences)
}

/// Writes `structure` under `root`, over whatever the paths it names hold.
///
/// Paths the structure does not name are left where they are.
///
/// # Errors
///
/// Returns [`Error::Unavailable`] when the source does not hold a file's content, [`Error::Io`]
/// when a place cannot be written, and [`Error::Taken`] when a directory stands where a file is to
/// be written.
pub fn write(
    source: &dyn ContentSource,
    structure: &TreeStructure,
    root: &Path,
) -> Result<(), Error> {
    fs::create_dir_all(root).map_err(Error::Io)?;

    for entry in structure.entries() {
        let at = root.join(entry.path());

        match entry.kind() {
            EntryKind::Directory => {
                fs::create_dir_all(&at).map_err(Error::Io)?;
            }
            EntryKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };

                if let Some(parent) = at.parent() {
                    fs::create_dir_all(parent).map_err(Error::Io)?;
                }

                // A link standing where the structure names a file is taken away rather than
                // followed: the content placed is this material's, and writing through the link
                // would put it somewhere else.
                remove_place(&at)?;

                source.write(ContentHash::from_digest(*storage.digest()), &at)?;
            }
            EntryKind::Symlink => {
                let Some(target) = entry.target() else {
                    continue;
                };

                if let Some(parent) = at.parent() {
                    fs::create_dir_all(parent).map_err(Error::Io)?;
                }

                remove_place(&at)?;
                make_link(target, &at)?;
            }
        }
    }

    Ok(())
}

/// Writes `structure` under `root`, replacing what the paths it names hold and leaving every other
/// path alone.
///
/// It is what a restore does: a path the structure names is written over, so a change to it is
/// thrown away rather than kept. A file the structure does not name is untouched, the same way a
/// branch switch leaves the paths git does not track.
///
/// # Errors
///
/// Returns [`Error::Unavailable`] when the source does not hold a file's content, [`Error::Io`]
/// when a place cannot be written, and [`Error::Taken`] when a directory stands where a file is to
/// be written.
pub fn replace(
    source: &dyn ContentSource,
    structure: &TreeStructure,
    root: &Path,
) -> Result<(), Error> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => fs::remove_file(root).map_err(Error::Io)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::Io(error)),
    }

    fs::create_dir_all(root).map_err(Error::Io)?;

    for entry in structure.entries() {
        let at = root.join(entry.path());

        match entry.kind() {
            EntryKind::Directory => {
                match fs::symlink_metadata(&at) {
                    Ok(metadata) if metadata.is_dir() => {}
                    Ok(_) => fs::remove_file(&at).map_err(Error::Io)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(Error::Io(error)),
                }

                fs::create_dir_all(&at).map_err(Error::Io)?;
            }
            EntryKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };

                if fs::symlink_metadata(&at).is_ok_and(|metadata| metadata.is_dir()) {
                    return Err(Error::Taken {
                        path: at.display().to_string(),
                    });
                }

                if let Some(parent) = at.parent() {
                    fs::create_dir_all(parent).map_err(Error::Io)?;
                }

                remove_place(&at)?;

                source.write(ContentHash::from_digest(*storage.digest()), &at)?;
            }
            EntryKind::Symlink => {
                let Some(target) = entry.target() else {
                    continue;
                };

                if let Some(parent) = at.parent() {
                    fs::create_dir_all(parent).map_err(Error::Io)?;
                }

                remove_place(&at)?;
                make_link(target, &at)?;
            }
        }
    }

    Ok(())
}

/// Moves the directory `from` to `to`, renaming where the two are on one volume.
///
/// A copy already at `to` is this program's own: a tree placed again takes its path back, and one
/// left behind is a run that was interrupted rather than something a person put there.
///
/// # Errors
///
/// Returns [`Error::Io`] when the directory cannot be moved or copied.
pub fn move_directory(from: &Path, to: &Path) -> Result<(), Error> {
    if fs::symlink_metadata(to).is_ok() {
        remove_directory(to)?;
    }

    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(Error::Io)?;
    }

    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // WORKAROUND: a staging area and a working tree can be on different volumes, a
            // project spread over mounts, where a rename is not one operation. The copy is the
            // fallback, and the original goes only once the copy is whole; a copy that cannot be
            // made either is the rename's failure as it was met.
            match copy_directory(from, to) {
                Ok(()) => fs::remove_dir_all(from).map_err(Error::Io),
                Err(_) => Err(Error::Io(rename_error)),
            }
        }
    }
}

/// Takes `at` away when it is a file or a link, and leaves a directory in place.
fn remove_place(at: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(at) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => fs::remove_file(at).map_err(Error::Io),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Io(error)),
    }
}

/// Takes the directory `at` away, whatever it holds.
fn remove_directory(at: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(at) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(at).map_err(Error::Io),
        Ok(_) => fs::remove_file(at).map_err(Error::Io),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Io(error)),
    }
}

/// Makes `at` a symbolic link to `target`.
fn make_link(target: &str, at: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, at).map_err(Error::Io)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, at).map_err(Error::Io)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, at);

        Err(Error::Io(io::Error::other(
            "a symbolic link cannot be made on this platform",
        )))
    }
}

/// Copies the directory `from` into `to`, making what `to` needs.
fn copy_directory(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;

    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source)?;

        if metadata.is_dir() {
            copy_directory(&source, &target)?;
        } else if metadata.is_symlink() {
            copy_link(&source, &target)?;
        } else {
            fs::copy(&source, &target)?;
        }
    }

    Ok(())
}

/// Copies the link `from` to `to`, with the target it names.
fn copy_link(from: &Path, to: &Path) -> io::Result<()> {
    let target = fs::read_link(from)?;
    let target = target.to_str().map(str::to_owned);
    let Some(target) = target else {
        return Err(io::Error::other(
            "a link whose target is not UTF-8 cannot be copied",
        ));
    };

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, to)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, to)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, to);

        Err(io::Error::other(
            "a symbolic link cannot be made on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_storage::Key;
    use rorolala_tree::{FileMode, TreeEntry, TreeStructure};

    use super::{Difference, DifferenceKind, differences, move_directory};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-git-support-tree-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// The key content `bytes` is stored under.
    fn key(bytes: &[u8]) -> Key {
        Key::new(*blake3::hash(bytes).as_bytes())
    }

    #[test]
    fn a_file_the_structure_does_not_name_is_not_a_difference() {
        let dir = scratch("extras");
        fs::write(dir.join("notes.txt"), b"notes").unwrap();

        let found = differences(&dir, &TreeStructure::empty()).unwrap();

        assert!(found.is_empty(), "{found:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changed_file_and_a_missing_file_are_both_differences() {
        let dir = scratch("differences");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.png"), b"changed").unwrap();

        let structure = TreeStructure::build(
            None,
            [
                TreeEntry::file("a.png", FileMode::Regular, key(b"a")),
                TreeEntry::directory("sub"),
                TreeEntry::file("sub/b.psd", FileMode::Regular, key(b"b")),
            ],
        )
        .unwrap();

        let found = differences(&dir, &structure).unwrap();

        assert_eq!(
            found,
            [
                Difference {
                    at: "a.png".to_owned(),
                    kind: DifferenceKind::Modified,
                },
                Difference {
                    at: "sub/b.psd".to_owned(),
                    kind: DifferenceKind::Missing,
                },
            ]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn moving_a_directory_over_a_stale_copy_replaces_it() {
        let dir = scratch("move");
        let from = dir.join("from");
        let to = dir.join("to");
        fs::create_dir_all(&from).unwrap();
        fs::write(from.join("a"), b"a").unwrap();
        fs::create_dir_all(&to).unwrap();
        fs::write(to.join("stale"), b"stale").unwrap();

        move_directory(&from, &to).unwrap();

        assert!(!from.exists());
        assert!(to.join("a").is_file());
        assert!(!to.join("stale").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}

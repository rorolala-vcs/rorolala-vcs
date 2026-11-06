//! Walking the tree a Workspace holds.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rorolala_layout::{EntryKind, LayoutPath};

use crate::tree_diff::FailedPath;

/// The directory a Workspace keeps for itself, which is not part of the work it holds.
///
/// A directory by this name is never walked into, and a directory that keeps one of its own is a
/// Workspace in its own right rather than part of this one.
const DATA_DIR: &str = ".rola";

/// The directory that marks a directory as a tree, whose contents are one unit of work.
const TREE_MARK: &str = ".rola-tree";

/// The marker in the name of a conflict copy: what a merge left beside the file it conflicts with.
const CONFLICT_MARK: &str = "~theirs";

/// One file, or one directory that is a tree, the tree holds.
pub struct Found {
    /// The path it is named by, relative to the root the walk started at.
    pub path: LayoutPath,

    /// Where it sits on disk.
    pub disk: PathBuf,

    /// Whether this is a file or a directory that is a tree.
    pub kind: EntryKind,

    /// How long it is, as bytes; a tree directory is a directory and not a file.
    pub size: u64,

    /// When it was last changed, in whole seconds since the epoch.
    pub seconds: u64,

    /// When it was last changed, the part of a second after them.
    pub nanos: u32,
}

/// Every file under `root`, every directory that is a tree, and every path that could not be read.
///
/// The walk skips the data directory and anything held by a directory that keeps one, ignores
/// symbolic links, and takes only plain files. A directory that keeps a `.rola-tree` of its own is a
/// tree: it is answered as one entry of kind [`EntryKind::Tree`] and not walked into, so what is
/// inside it is the tree's own reading rather than this one's. An empty file is a file like any
/// other. A name that does not read as a path a Layout may name is not walked past: it is recorded,
/// since the caller is the one that has to say what to do about it. A directory that cannot be read
/// is recorded the same way rather than failing the whole walk: one unreadable directory is
/// something to be told about, not a reason the tree has nothing to say.
#[must_use]
pub fn walk(root: &Path) -> (Vec<Found>, Vec<FailedPath>) {
    let mut found = Vec::new();
    let mut failed = Vec::new();

    // The root itself is the one place a tree has no name of its own: a Workspace whose root is a
    // tree is one entry at the root path, and what is under it is that tree's own reading.
    if root.join(TREE_MARK).is_dir() {
        match fs::metadata(root) {
            Ok(meta) => found.push(found_at(
                LayoutPath::root(),
                root.to_path_buf(),
                EntryKind::Tree,
                &meta,
            )),
            Err(error) => failed.push(FailedPath::new(
                root.display().to_string(),
                error.to_string(),
            )),
        }

        return (found, failed);
    }

    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                failed.push(FailedPath::new(
                    dir.display().to_string(),
                    error.to_string(),
                ));
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failed.push(FailedPath::new(
                        dir.display().to_string(),
                        error.to_string(),
                    ));
                    continue;
                }
            };

            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    failed.push(FailedPath::new(
                        entry.path().display().to_string(),
                        error.to_string(),
                    ));
                    continue;
                }
            };

            // A symbolic link is not a file the work holds: what it points at is somewhere else.
            if kind.is_symlink() {
                continue;
            }

            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                failed.push(FailedPath::new(
                    entry.path().display().to_string(),
                    "the name does not read as UTF-8".to_owned(),
                ));
                continue;
            };

            // The Workspace's own data and a tree's own data are not work, and a conflict copy is
            // what a merge left behind rather than something anybody recorded.
            if name == DATA_DIR || name == TREE_MARK || name.contains(CONFLICT_MARK) {
                continue;
            }

            let disk = entry.path();

            if kind.is_dir() {
                // A directory keeping a data directory of its own is a Workspace of its own, and
                // its work is not this Workspace's to read.
                if disk.join(DATA_DIR).is_dir() {
                    continue;
                }

                // A directory keeping a tree mark is one unit of work of its own, and what is
                // inside it is that tree's own reading.
                if disk.join(TREE_MARK).is_dir() {
                    match found_beside(root, &disk, EntryKind::Tree) {
                        Some(entry) => found.push(entry),
                        None => failed.push(FailedPath::new(
                            disk.display().to_string(),
                            "the name does not read as a path".to_owned(),
                        )),
                    }

                    continue;
                }

                stack.push(disk);
                continue;
            }

            if !kind.is_file() {
                continue;
            }

            match found_beside(root, &disk, EntryKind::File) {
                Some(entry) => found.push(entry),
                None => failed.push(FailedPath::new(
                    disk.display().to_string(),
                    "the name does not read as a path".to_owned(),
                )),
            }
        }
    }

    (found, failed)
}

/// The entry `disk` makes, named against `root`, if both its path and its metadata read.
fn found_beside(root: &Path, disk: &Path, kind: EntryKind) -> Option<Found> {
    let relative = disk
        .strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .map(str::to_owned)?;

    let path = LayoutPath::new(&relative).ok()?;
    let meta = fs::metadata(disk).ok()?;

    Some(found_at(path, disk.to_path_buf(), kind, &meta))
}

/// The entry at `path`, with the metadata of the file or directory it names.
fn found_at(path: LayoutPath, disk: PathBuf, kind: EntryKind, meta: &fs::Metadata) -> Found {
    let (seconds, nanos) = stamp(meta);

    Found {
        path,
        disk,
        kind,
        size: meta.len(),
        seconds,
        nanos,
    }
}

/// When a file was last changed, as whole seconds since the epoch and the part of a second after.
pub fn stamp(meta: &fs::Metadata) -> (u64, u32) {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or((0, 0), |since| (since.as_secs(), since.subsec_nanos()))
}

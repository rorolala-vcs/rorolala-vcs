//! Walking the tree a Workspace holds.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rorolala_layout::LayoutPath;

use crate::tree_diff::FailedPath;

/// The directory a Workspace keeps for itself, which is not part of the work it holds.
///
/// A directory by this name is never walked into, and a directory that keeps one of its own is a
/// Workspace in its own right rather than part of this one.
const DATA_DIR: &str = ".rola";

/// One file the tree holds.
pub struct Found {
    /// The path it is named by, relative to the root the walk started at.
    pub path: LayoutPath,

    /// Where it sits on disk.
    pub disk: PathBuf,

    /// How long it is.
    pub size: u64,

    /// When it was last changed, in whole seconds since the epoch.
    pub seconds: u64,

    /// When it was last changed, the part of a second after them.
    pub nanos: u32,
}

/// Every file under `root`, and every path that could not be read.
///
/// The walk skips the data directory and anything held by a directory that keeps one, ignores
/// symbolic links, and takes only plain files; an empty file is a file like any other. A name that
/// does not read as a path a Layout may name is not walked past — it is recorded, since the caller
/// is the one that has to say what to do about it. A directory that cannot be read is recorded the
/// same way rather than failing the whole walk: one unreadable directory is something to be told
/// about, not a reason the tree has nothing to say.
pub fn walk(root: &Path) -> (Vec<Found>, Vec<FailedPath>) {
    let mut found = Vec::new();
    let mut failed = Vec::new();
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

            if entry.file_name().to_str() == Some(DATA_DIR) {
                continue;
            }

            let disk = entry.path();

            if kind.is_dir() {
                // A directory keeping a data directory of its own is a Workspace of its own, and
                // its work is not this Workspace's to read.
                if disk.join(DATA_DIR).is_dir() {
                    continue;
                }

                stack.push(disk);
                continue;
            }

            if !kind.is_file() {
                continue;
            }

            let relative = disk
                .strip_prefix(root)
                .ok()
                .and_then(Path::to_str)
                .map(str::to_owned);

            let Some(relative) = relative else {
                failed.push(FailedPath::new(
                    disk.display().to_string(),
                    "the name does not read as UTF-8".to_owned(),
                ));
                continue;
            };

            let path = match LayoutPath::new(&relative) {
                Ok(path) => path,
                Err(error) => {
                    failed.push(FailedPath::new(relative, error.to_string()));
                    continue;
                }
            };

            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(error) => {
                    failed.push(FailedPath::new(relative, error.to_string()));
                    continue;
                }
            };

            let (seconds, nanos) = stamp(&meta);
            found.push(Found {
                path,
                disk,
                size: meta.len(),
                seconds,
                nanos,
            });
        }
    }

    (found, failed)
}

/// When a file was last changed, as whole seconds since the epoch and the part of a second after.
pub fn stamp(meta: &fs::Metadata) -> (u64, u32) {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or((0, 0), |since| (since.as_secs(), since.subsec_nanos()))
}

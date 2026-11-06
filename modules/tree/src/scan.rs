//! Reading the work a tree holds, as the structure it would be recorded as.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rorolala_layout::LayoutPath;
use rorolala_storage::Key;
use rorolala_workspace_analyze::{Cache, Entry, FailedPath, fingerprint};

use crate::TreeError;
use crate::consts::{CONFLICT_MARK, DATA, MARK};
use crate::rules::TreeRules;
use crate::structure::{FileMode, TreeEntry, TreeStructure};

/// What one reading of a tree found.
pub struct Scanned {
    /// The work as a structure: the tree version its files would make.
    pub structure: TreeStructure,

    /// What each file looked like, to be kept for the next reading and for a move to be told.
    pub cache: Cache,

    /// Paths the reading could not look at.
    pub failed: Vec<FailedPath>,
}

/// Every path of the tree at `root`, read into the structure its version would hold.
///
/// The reading asks `rules` about each path, and a path the rules ignore is left out even when it
/// is there, unless `include_ignored` says the run wants it. What is under a tree, and the
/// Workspace's own data, are never read: a nested tree is one entry of its own, and a conflict copy
/// is what a merge left behind rather than work.
///
/// A file whose time and length are what the cache holds is not read again; the rest are read, and
/// what was read is the cache a caller writes back. A file that cannot be read is recorded rather
/// than failing the whole reading.
///
/// # Errors
///
/// Returns [`TreeError::Io`] when the tree cannot be walked, [`TreeError::Path`] when a name is not
/// one a tree may hold, and [`TreeError::Conflict`] when two names make one path.
// The walk is one pass over the tree: splitting it would scatter the view of what is control,
// what is ignored and what is work across helpers that each have to restate the rest.
#[allow(clippy::too_many_lines)]
pub fn scan(
    root: &Path,
    rules: &TreeRules,
    previous: &Cache,
    include_ignored: bool,
) -> Result<Scanned, TreeError> {
    let mut entries: Vec<TreeEntry> = Vec::new();
    let mut cache = Cache::empty();
    let mut failed: Vec<FailedPath> = Vec::new();

    let mut stack = vec![(root.to_path_buf(), String::new())];

    while let Some((directory, prefix)) = stack.pop() {
        let held = match fs::read_dir(&directory) {
            Ok(held) => held,
            Err(error) => {
                failed.push(FailedPath::new(
                    directory.display().to_string(),
                    error.to_string(),
                ));
                continue;
            }
        };

        for entry in held {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failed.push(FailedPath::new(
                        directory.display().to_string(),
                        error.to_string(),
                    ));
                    continue;
                }
            };

            let Ok(relative) = entry.path().strip_prefix(root).map(Path::to_path_buf) else {
                continue;
            };
            let Some(name) = relative.file_name().and_then(|name| name.to_str()) else {
                failed.push(FailedPath::new(
                    entry.path().display().to_string(),
                    "the name does not read as UTF-8".to_owned(),
                ));
                continue;
            };

            // A name a tree never holds is left out rather than reported: it is what the Workspace
            // and the merge put there, not something a person recorded.
            if name == DATA || name == MARK || name.contains(CONFLICT_MARK) {
                continue;
            }

            let path = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };

            if TreeRules::is_control(&path) {
                continue;
            }

            let disk = entry.path();
            let Ok(kind) = entry.file_type() else {
                failed.push(FailedPath::new(
                    disk.display().to_string(),
                    "its kind cannot be read".to_owned(),
                ));
                continue;
            };

            if kind.is_dir() {
                // A directory that is another tree of its own is not work of this one, and a
                // directory that holds a Workspace is not this Workspace's either.
                if disk.join(MARK).is_dir() || disk.join(DATA).is_dir() {
                    continue;
                }

                if !include_ignored && rules.ignored(&path, true) {
                    continue;
                }

                entries.push(TreeEntry::directory(path.clone()));
                stack.push((disk, path));
                continue;
            }

            // `.rolaignore` is work like any other file, and it is always read, since it is the file
            // that says what the rest of the rules are.
            let keep =
                name == crate::consts::IGNORE || include_ignored || !rules.ignored(&path, false);
            if !keep {
                continue;
            }

            if kind.is_symlink() {
                match fs::read_link(&disk) {
                    Ok(target) => match target.to_str() {
                        Some(target) => {
                            entries.push(TreeEntry::symlink(path, target));
                        }
                        None => failed.push(FailedPath::new(
                            path,
                            "the link's target does not read as UTF-8".to_owned(),
                        )),
                    },
                    Err(error) => failed.push(FailedPath::new(path, error.to_string())),
                }

                continue;
            }

            if !kind.is_file() {
                continue;
            }

            match read_file(&disk, &path, previous, &mut cache) {
                Ok((mode, digest)) => {
                    entries.push(TreeEntry::file(path, mode, Key::new(digest)));
                }
                Err(error) => failed.push(FailedPath::new(path, error)),
            }
        }
    }

    // The reading is sorted and its directories are made entries of their own by the structure
    // itself, so what comes back is one shape however the filesystem handed the paths over.
    let structure = TreeStructure::build(None, entries)?;

    Ok(Scanned {
        structure,
        cache,
        failed,
    })
}

/// What one file holds: its mode, and the digest of its content.
///
/// A file whose time and length are what `previous` wrote down is not read again. What is read is
/// recorded in `cache`, with the spans a move of it may be guessed from.
fn read_file(
    disk: &Path,
    path: &str,
    previous: &Cache,
    cache: &mut Cache,
) -> Result<(FileMode, [u8; 32]), String> {
    let meta = fs::metadata(disk).map_err(|error| error.to_string())?;
    let (seconds, nanos) = stamp(&meta);

    let layout_path = LayoutPath::new(path).map_err(|error| error.to_string())?;
    let size = meta.len();

    let reused = previous
        .get(&layout_path)
        .filter(|entry| entry.same_stamp(seconds, nanos, size));

    let (digest, spans) = if let Some(entry) = reused {
        (entry.digest(), entry.spans().map(<[(u32, u32)]>::to_vec))
    } else {
        let bytes = fs::read(disk).map_err(|error| error.to_string())?;
        let digest = fingerprint::digest(&bytes);
        let binary = fingerprint::is_binary(&bytes);
        let spans = (!binary).then(|| fingerprint::spans(&bytes, false));

        (digest, spans)
    };

    cache.insert(layout_path, Entry::new(seconds, nanos, size, digest, spans));

    Ok((mode_of(&meta), digest))
}

/// Whether the file at `meta` is executable, in the two modes a tree writes down.
#[cfg(unix)]
pub fn mode_of(meta: &fs::Metadata) -> FileMode {
    use std::os::unix::fs::PermissionsExt as _;

    if meta.permissions().mode() & 0o100 != 0 {
        FileMode::Executable
    } else {
        FileMode::Regular
    }
}

/// Whether the file at `meta` is executable, in the two modes a tree writes down.
///
/// A platform without Unix permission bits has no executable mode of its own, so every file is
/// regular there. The function is constant on those platforms, which is why it stays so.
#[cfg(not(unix))]
pub const fn mode_of(_: &fs::Metadata) -> FileMode {
    FileMode::Regular
}

/// When a file was last changed, as whole seconds since the epoch and the part of a second after.
fn stamp(meta: &fs::Metadata) -> (u64, u32) {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or((0, 0), |since| (since.as_secs(), since.subsec_nanos()))
}

/// The cache path a tree keeps what its reading found under.
#[must_use]
pub fn cache_path(tree_root: &Path) -> PathBuf {
    tree_root
        .join(MARK)
        .join(crate::consts::BASELINE)
        .join(crate::consts::BASELINE_ENTRIES)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_ignore::IgnoreRuleSet;
    use rorolala_workspace_analyze::Cache;

    use super::scan;
    use crate::rules::TreeRules;
    use crate::structure::EntryKind;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-tree-scan-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// What reading the tree at `root` with no rules finds.
    fn read(root: &Path) -> crate::Scanned {
        let rules = TreeRules::load(root, IgnoreRuleSet::empty()).unwrap();

        scan(root, &rules, &Cache::empty(), false).unwrap()
    }

    #[test]
    fn a_file_a_directory_and_an_empty_directory_are_all_entries() {
        let root = scratch("shape");
        fs::write(root.join("a.psd"), b"a").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/b.psd"), b"b").unwrap();
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::create_dir_all(root.join(".rola-tree")).unwrap();

        let scanned = read(&root);
        let paths: Vec<&str> = scanned
            .structure
            .entries()
            .iter()
            .map(crate::TreeEntry::path)
            .collect();

        assert_eq!(paths, ["a.psd", "empty", "sub", "sub/b.psd"]);
        assert_eq!(
            scanned.structure.entry("empty").map(crate::TreeEntry::kind),
            Some(EntryKind::Directory)
        );
        assert!(scanned.failed.is_empty(), "{:?}", scanned.failed);
    }

    #[test]
    fn what_belongs_to_the_workspace_or_a_merge_is_not_work() {
        let root = scratch("control");
        fs::write(root.join("work.psd"), b"work").unwrap();
        fs::write(root.join("raw~theirs.psd"), b"theirs").unwrap();
        fs::create_dir_all(root.join(".rola")).unwrap();
        fs::create_dir_all(root.join(".rola-tree")).unwrap();

        let scanned = read(&root);
        let paths: Vec<&str> = scanned
            .structure
            .entries()
            .iter()
            .map(crate::TreeEntry::path)
            .collect();

        assert_eq!(paths, ["work.psd"]);
    }

    #[test]
    fn a_nested_tree_is_not_part_of_the_tree_that_holds_it() {
        let root = scratch("nested");
        fs::write(root.join("outer.psd"), b"outer").unwrap();
        fs::create_dir_all(root.join("inner/.rola-tree")).unwrap();
        fs::write(root.join("inner/inside.psd"), b"inside").unwrap();

        let scanned = read(&root);
        let paths: Vec<&str> = scanned
            .structure
            .entries()
            .iter()
            .map(crate::TreeEntry::path)
            .collect();

        assert_eq!(paths, ["outer.psd"]);
    }

    #[test]
    fn the_rules_of_the_tree_are_part_of_its_work() {
        let root = scratch("ignore");
        fs::write(root.join(".rolaignore"), "*.mb\n").unwrap();
        fs::write(root.join("scene.mb"), b"ignored").unwrap();
        fs::write(root.join("keep.psd"), b"kept").unwrap();

        let scanned = read(&root);
        let paths: Vec<&str> = scanned
            .structure
            .entries()
            .iter()
            .map(crate::TreeEntry::path)
            .collect();

        // The rules file itself is work; what its rules cover is not.
        assert_eq!(paths, [".rolaignore", "keep.psd"]);
    }

    #[test]
    fn one_reading_can_include_what_the_rules_ignore() {
        let root = scratch("include-ignored");
        fs::write(root.join(".rolaignore"), "*.mb\n").unwrap();
        fs::write(root.join("scene.mb"), b"ignored").unwrap();

        let rules = TreeRules::load(&root, IgnoreRuleSet::empty()).unwrap();
        let scanned = scan(&root, &rules, &Cache::empty(), true).unwrap();

        assert!(scanned.structure.entry("scene.mb").is_some());
    }
}

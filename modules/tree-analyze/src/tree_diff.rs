//! Reading a Workspace's tree against the Layout it works from.
//!
//! A Layout names the paths a Workspace works with; the tree is what those paths hold now. Reading
//! the two together answers what changed between them: a path the Layout names that the tree no
//! longer holds ([`TreeDiff::lost`]), a path the tree holds that the Layout does not name
//! ([`TreeDiff::untagged`]), a path that moved ([`TreeDiff::renamed`]), and a path that stayed
//! where it was and whose content changed ([`TreeDiff::modified`]).
//!
//! # How a move is told from a loss and a gain
//!
//! A path that moved is a path that is gone and another that appeared, holding what the first one
//! held. Telling the two apart costs reading content, so it is done in two passes: a file whose
//! digest is one a lost path was last seen with has moved unchanged, and a text file that is alike
//! enough has moved and been edited. The likeness is git's — see [`crate::fingerprint`] — and how
//! alike is alike enough is the caller's to say.
//!
//! A file that moved and was edited is counted as a move *and* as a change: what it holds is not
//! what the Layout names, so it is [`TreeDiff::modified`] as well as [`TreeDiff::renamed`].
//!
//! # What it remembers
//!
//! A move is worked out from what a lost path held, which is not on disk any more — so the reading
//! keeps what it found: see [`crate::Cache`]. A file whose time and length have not changed is not
//! read again, and a lost path is recognised by what the last reading wrote down about it. A
//! reading of a tree that has never been read has no such memory, so a move that happened before
//! the first reading is seen as a loss and a gain rather than as a move.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use rorolala_layout::{Layout, LayoutPath};
use rorolala_utils_constants::WORKSPACE_CACHE_DIR;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;

use crate::cache::{Cache, Entry};
use crate::error::TreeDiffError;
use crate::{fingerprint, scan};

/// A path that moved: where the Layout named it, and where the tree holds it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRename {
    /// The path the Layout named it by.
    pub from: LayoutPath,

    /// The path the tree holds it at now.
    pub to: LayoutPath,
}

/// A path the reading could not look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedPath {
    /// The path as it was seen, which may be one a Layout could not name.
    pub path: String,

    /// Why it could not be looked at.
    pub reason: String,
}

impl FailedPath {
    /// A path that could not be looked at, and why.
    #[must_use]
    pub const fn new(path: String, reason: String) -> Self {
        Self { path, reason }
    }
}

/// How a Workspace's tree stands beside the Layout it works from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeDiff {
    /// Paths the Layout names that the tree no longer holds.
    pub lost: Vec<LayoutPath>,

    /// Paths the tree holds that the Layout does not name.
    pub untagged: Vec<LayoutPath>,

    /// Paths that moved.
    pub renamed: Vec<PathRename>,

    /// Paths that stayed where they were and whose content changed.
    pub modified: Vec<LayoutPath>,

    /// Paths that could not be looked at.
    pub failed: Vec<FailedPath>,
}

/// How the tree under `workspace` stands beside `layout`.
///
/// Every list is in [`LayoutPath`] order, so the same tree answers the same way twice. `alike` is
/// how alike two text files have to be to count as the same file moved: `0.0` is any likeness,
/// `1.0` is no change at all, and anything past either end is taken as the end. It never touches a
/// binary file, which is the same file moved only when its digest is the one it had.
///
/// # Errors
///
/// Returns [`TreeDiffError::Layout`] if the Layout cannot be read back, and [`TreeDiffError::Io`]
/// if the tree cannot be walked or what was found cannot be written down. A single file that
/// cannot be read is not one of these: it is recorded in [`TreeDiff::failed`].
#[allow(clippy::too_many_lines)]
pub fn tree_diff(
    layout: &Layout,
    workspace: &Workspace,
    alike: f32,
) -> Result<TreeDiff, TreeDiffError> {
    layout.refresh()?;

    let root = workspace.get_root();
    let (found, mut failed) = scan::walk(root);

    let tracked: BTreeSet<LayoutPath> = layout.paths().into_iter().map(|(path, _)| path).collect();

    let cache_path = cache_path(root, layout);
    let previous = Cache::read(&cache_path);

    let mut disk: BTreeMap<LayoutPath, scan::Found> = BTreeMap::new();
    for file in found {
        disk.insert(file.path.clone(), file);
    }

    // What each file on disk holds now. A file whose time and length are what the last reading
    // wrote down is not read again; the rest are read together, since reading and hashing is where
    // the whole reading spends its time.
    let mut current: BTreeMap<LayoutPath, Entry> = BTreeMap::new();
    let mut modified: Vec<LayoutPath> = Vec::new();
    let mut unread: Vec<(&LayoutPath, &scan::Found)> = Vec::new();
    for (path, file) in &disk {
        match previous.get(path) {
            Some(was) if was.same_stamp(file.seconds, file.nanos, file.size) => {
                current.insert(path.clone(), was.clone());
            }
            _ => unread.push((path, file)),
        }
    }

    for (path, result) in read_together(&unread) {
        let entry = match result {
            Ok(entry) => entry,
            Err(error) => {
                failed.push(FailedPath::new(path.as_str().to_owned(), error.to_string()));
                continue;
            }
        };

        // A file that was not read again holds what it held, so only one that was read can have
        // changed; what it changed from is what the last reading wrote down.
        if tracked.contains(path)
            && previous
                .get(path)
                .is_some_and(|was| was.digest() != entry.digest())
        {
            modified.push(path.clone());
        }

        current.insert(path.clone(), entry);
    }

    let mut lost: Vec<LayoutPath> = tracked
        .iter()
        .filter(|path| !disk.contains_key(*path))
        .cloned()
        .collect();
    let mut untagged: Vec<LayoutPath> = disk
        .keys()
        .filter(|path| !tracked.contains(*path))
        .cloned()
        .collect();

    let mut renamed = Vec::new();
    let mut moved_from: BTreeSet<LayoutPath> = BTreeSet::new();
    let mut moved_to: BTreeSet<LayoutPath> = BTreeSet::new();

    // First pass: a lost path whose digest is the one an untagged file holds has moved unchanged.
    let mut by_digest: BTreeMap<[u8; 32], Vec<LayoutPath>> = BTreeMap::new();
    for path in &lost {
        if let Some(was) = previous.get(path) {
            by_digest
                .entry(was.digest())
                .or_default()
                .push(path.clone());
        }
    }
    for path in &untagged {
        let Some(entry) = current.get(path) else {
            continue;
        };
        if let Some(list) = by_digest.get_mut(&entry.digest())
            && let Some(from) = list.pop()
        {
            renamed.push(PathRename {
                from: from.clone(),
                to: path.clone(),
            });
            moved_from.insert(from);
            moved_to.insert(path.clone());
        }
    }

    // Second pass: text a lost path was last seen holding, against text the tree holds now.
    let sources: Vec<&LayoutPath> = lost
        .iter()
        .filter(|path| !moved_from.contains(*path))
        .collect();
    let destinations: Vec<&LayoutPath> = untagged
        .iter()
        .filter(|path| !moved_to.contains(*path))
        .collect();
    for (from, to, score) in likeness(&sources, &destinations, &previous, &current, alike) {
        renamed.push(PathRename {
            from: from.clone(),
            to: to.clone(),
        });
        moved_from.insert(from);
        moved_to.insert(to.clone());

        if score < 1.0 {
            modified.push(to);
        }
    }

    renamed.sort_by(|left, right| left.from.cmp(&right.from));
    modified.sort_unstable();
    modified.dedup();
    failed.sort_by(|left, right| left.path.cmp(&right.path));

    // What the next reading starts from: what each file holds now, and — for a path the Layout
    // still names but the tree no longer holds — what it held when it was last seen, which is all
    // that is left of it to recognise a move by.
    let mut next = Cache::empty();
    for (path, entry) in &current {
        next.insert(path.clone(), entry.clone());
    }
    for path in &lost {
        if let Some(was) = previous.get(path) {
            next.insert(path.clone(), was.clone());
        }
    }
    next.write(&cache_path)?;

    // A path that moved is one move, not also a loss and a gain: what was paired comes out of the
    // two lists it would otherwise sit in. The cache above is written first, since what a moved-away
    // path held is what tells the next reading that it moved.
    lost.retain(|path| !moved_from.contains(path));
    untagged.retain(|path| !moved_to.contains(path));

    Ok(TreeDiff {
        lost,
        untagged,
        renamed,
        modified,
        failed,
    })
}

/// Reads and fingerprints `files` at once, answering in the order they were given.
///
/// The work is cut into as many pieces as there are cores to run them on, and each piece is read
/// on a thread of its own. What it costs is what reading costs, which is the whole of what a
/// reading of an unchanged tree does not do; a tree where nothing changed reads nothing here.
// The join handles are collected so that every thread is running before any is waited for;
// joining each as it is spawned would run the reads one after another, which is the whole of what
// this is here to avoid.
#[allow(clippy::needless_collect)]
fn read_together<'a>(
    files: &[(&'a LayoutPath, &'a scan::Found)],
) -> Vec<(&'a LayoutPath, Result<Entry, std::io::Error>)> {
    if files.is_empty() {
        return Vec::new();
    }

    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let pieces = cores.clamp(1, files.len());
    let size = files.len().div_ceil(pieces);

    let read: Vec<Vec<(&LayoutPath, Result<Entry, std::io::Error>)>> =
        std::thread::scope(|scope| {
            let readers: Vec<_> = files
                .chunks(size)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|(path, file)| (*path, read_entry(file)))
                            .collect()
                    })
                })
                .collect();

            readers
                .into_iter()
                .map(|reader| reader.join().expect("reading a file never panics"))
                .collect()
        });

    read.into_iter().flatten().collect()
}

/// What a file holds now, read from disk.
fn read_entry(file: &scan::Found) -> Result<Entry, std::io::Error> {
    let bytes = fs::read(&file.disk)?;
    let digest = fingerprint::digest(&bytes);
    let binary = fingerprint::is_binary(&bytes);
    let spans = (!binary).then(|| fingerprint::spans(&bytes, false));

    Ok(Entry::new(
        file.seconds,
        file.nanos,
        file.size,
        digest,
        spans,
    ))
}

/// The moves among text files: each a lost path, the path it moved to, and how alike the two are.
///
/// Only text takes part: a binary file's likeness is not something this estimates, so it is the
/// same file moved only when its digest says so, which the first pass already found. A pair is
/// taken only when the two are alike enough and neither has been taken already, and the likeliest
/// pairs are taken first.
#[allow(clippy::cast_precision_loss)]
fn likeness(
    sources: &[&LayoutPath],
    destinations: &[&LayoutPath],
    previous: &Cache,
    current: &BTreeMap<LayoutPath, Entry>,
    alike: f32,
) -> Vec<(LayoutPath, LayoutPath, f32)> {
    /// A pair that may be the same file moved.
    struct Candidate {
        /// How alike the two are.
        score: f32,
        /// The path the Layout named.
        from: LayoutPath,
        /// The path the tree holds.
        to: LayoutPath,
    }

    // Clamped, since a likeness outside 0..1 says nothing this can use.
    let alike = alike.clamp(0.0, 1.0);

    let mut candidates: Vec<Candidate> = Vec::new();
    for from in sources {
        let Some(source) = previous.get(from) else {
            continue;
        };
        let Some(source_spans) = source.spans() else {
            continue;
        };

        for to in destinations {
            let Some(destination) = current.get(to) else {
                continue;
            };
            let Some(destination_spans) = destination.spans() else {
                continue;
            };

            let max_size = source.size().max(destination.size());
            if max_size == 0 {
                continue;
            }

            // git's own reading of when a size has moved too far to be the same file: what was
            // copied is at most the whole of the smaller file, so a size that differs by more than
            // the likeness allows cannot come out alike enough anyway.
            let delta = source.size().abs_diff(destination.size());
            if (max_size as f32) * (1.0 - alike) < delta as f32 {
                continue;
            }

            let copied = fingerprint::copied(source_spans, destination_spans);
            let score = copied as f32 / max_size as f32;
            if score >= alike {
                candidates.push(Candidate {
                    score,
                    from: (*from).clone(),
                    to: (*to).clone(),
                });
            }
        }
    }

    // Likeliest first, and in path order between the equally likely, so what is taken does not
    // depend on the order the files were walked in.
    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.from.cmp(&right.from))
            .then_with(|| left.to.cmp(&right.to))
    });

    let mut taken_from: BTreeSet<LayoutPath> = BTreeSet::new();
    let mut taken_to: BTreeSet<LayoutPath> = BTreeSet::new();
    let mut pairs = Vec::new();
    for candidate in candidates {
        if taken_from.contains(&candidate.from) || taken_to.contains(&candidate.to) {
            continue;
        }

        taken_from.insert(candidate.from.clone());
        taken_to.insert(candidate.to.clone());
        pairs.push((candidate.from, candidate.to, candidate.score));
    }

    pairs
}

/// Where the reading of `layout` keeps what it found, under the Workspace it is read from.
fn cache_path(root: &Path, layout: &Layout) -> PathBuf {
    let name = layout
        .dir()
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("layout");

    root.join(WORKSPACE_CACHE_DIR)
        .components()
        .collect::<PathBuf>()
        .join(format!("{name}-analyze"))
        .join("entries")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::sleep;
    use std::time::Duration;

    use rorolala_layout::{LayoutPath, MutableData};
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{PathRename, tree_diff};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-tree-analyze-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A path, from text a test can read.
    fn path(text: &str) -> LayoutPath {
        LayoutPath::new(text).unwrap()
    }

    /// A Workspace in a directory of its own, and the Layout it starts with.
    fn workspace(label: &str) -> (PathBuf, Workspace) {
        let dir = scratch(label);
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        (dir, workspace)
    }

    /// Makes the Layout a Workspace starts with name `text`, held by no account.
    fn track(workspace: &Workspace, id: u128, text: &str) {
        let layout = workspace.layouts().get("main").unwrap().unwrap();
        layout
            .create_entry(
                Uuid::from_u128(id),
                MutableData::new(None, [1; 32], "first".to_owned()),
            )
            .unwrap();
        layout
            .create_path(&path(text), Uuid::from_u128(id))
            .unwrap();
    }

    #[test]
    fn what_the_layout_names_and_the_tree_does_not_is_lost() {
        let (dir, workspace) = workspace("lost");
        track(&workspace, 1, "kept.txt");
        track(&workspace, 2, "gone.txt");

        fs::write(dir.join("kept.txt"), b"kept").unwrap();
        fs::write(dir.join("new.txt"), b"new").unwrap();

        let layout = workspace.layouts().get("main").unwrap().unwrap();
        let diff = tree_diff(&layout, &workspace, 0.5).unwrap();

        assert_eq!(diff.lost, vec![path("gone.txt")], "{diff:?}");
        assert_eq!(diff.untagged, vec![path("new.txt")], "{diff:?}");
        assert!(diff.renamed.is_empty(), "{diff:?}");
        assert!(diff.modified.is_empty(), "{diff:?}");
        assert!(diff.failed.is_empty(), "{diff:?}");
    }

    #[test]
    fn a_file_moved_without_changing_is_seen_as_moved() {
        let (dir, workspace) = workspace("moved");
        track(&workspace, 1, "a.txt");
        fs::write(dir.join("a.txt"), b"the same content\n").unwrap();

        let layout = workspace.layouts().get("main").unwrap().unwrap();
        // A first reading writes down what the file holds, so the move can be told next time.
        tree_diff(&layout, &workspace, 0.5).unwrap();

        fs::rename(dir.join("a.txt"), dir.join("b.txt")).unwrap();

        let diff = tree_diff(&layout, &workspace, 0.5).unwrap();

        assert_eq!(
            diff.renamed,
            vec![PathRename {
                from: path("a.txt"),
                to: path("b.txt")
            }],
            "{diff:?}"
        );
        assert!(diff.lost.is_empty(), "{diff:?}");
        assert!(diff.untagged.is_empty(), "{diff:?}");
        assert!(diff.modified.is_empty(), "{diff:?}");
    }

    #[test]
    fn a_file_that_stayed_and_changed_is_modified() {
        let (dir, workspace) = workspace("modified");
        track(&workspace, 1, "a.txt");
        fs::write(dir.join("a.txt"), b"before\n").unwrap();

        let layout = workspace.layouts().get("main").unwrap().unwrap();
        tree_diff(&layout, &workspace, 0.5).unwrap();

        // Long enough that the change is a change in time as well as in content.
        sleep(Duration::from_millis(20));
        fs::write(dir.join("a.txt"), b"after\n").unwrap();

        let diff = tree_diff(&layout, &workspace, 0.5).unwrap();

        assert_eq!(diff.modified, vec![path("a.txt")], "{diff:?}");
        assert!(diff.renamed.is_empty(), "{diff:?}");
    }

    #[test]
    fn a_text_file_moved_and_edited_is_moved_and_modified() {
        let (dir, workspace) = workspace("edited");
        track(&workspace, 1, "a.txt");
        fs::write(dir.join("a.txt"), b"one\ntwo\nthree\nfour\n").unwrap();

        let layout = workspace.layouts().get("main").unwrap().unwrap();
        tree_diff(&layout, &workspace, 0.5).unwrap();

        // The same prose, moved, with a line added: alike enough to be the same file.
        fs::remove_file(dir.join("a.txt")).unwrap();
        fs::write(dir.join("b.txt"), b"one\ntwo\nthree\nfour\nfive\n").unwrap();

        let diff = tree_diff(&layout, &workspace, 0.5).unwrap();

        assert_eq!(
            diff.renamed,
            vec![PathRename {
                from: path("a.txt"),
                to: path("b.txt")
            }],
            "{diff:?}"
        );
        assert_eq!(diff.modified, vec![path("b.txt")], "{diff:?}");
    }

    #[test]
    fn what_a_workspace_keeps_for_itself_is_not_part_of_the_work() {
        let (dir, workspace) = workspace("data-dir");

        // A nested Workspace's work is not this Workspace's to read either.
        let nested = dir.join("nested");
        fs::create_dir_all(nested.join(".rola")).unwrap();
        fs::write(nested.join("hidden.txt"), b"not here").unwrap();

        fs::write(dir.join("seen.txt"), b"here").unwrap();

        let layout = workspace.layouts().get("main").unwrap().unwrap();
        let diff = tree_diff(&layout, &workspace, 0.5).unwrap();

        assert_eq!(diff.untagged, vec![path("seen.txt")], "{diff:?}");
    }

    #[test]
    fn many_files_are_read_back_in_path_order_however_they_are_read() {
        let (dir, workspace) = workspace("many");
        let mut expected = Vec::new();

        // More files than there are cores, so the reading is cut into more than one piece.
        for at in 0..64 {
            let name = format!("file-{at}.txt");
            fs::write(dir.join(&name), format!("file number {at}\n")).unwrap();
            expected.push(path(&name));
        }
        expected.sort_unstable();

        let layout = workspace.layouts().get("main").unwrap().unwrap();
        let diff = tree_diff(&layout, &workspace, 0.5).unwrap();

        assert_eq!(diff.untagged, expected, "{diff:?}");
        assert!(diff.failed.is_empty(), "{diff:?}");
    }
}

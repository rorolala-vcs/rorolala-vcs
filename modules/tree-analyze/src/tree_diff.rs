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

use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::{Layout, LayoutPath};
use rorolala_utils_constants::WORKSPACE_CACHE_DIR;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::{Merging, Workspace};

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

    /// Whether the move is one the two being the same bytes makes — a strong move — rather than one
    /// only their likeness suggests, which is a weak move and may be taken back.
    ///
    /// It is the whole of what a caller can tell about a move: a strong one is a fact to be
    /// confirmed, and a weak one is a guess to be confirmed or taken back.
    pub strong: bool,
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
/// It writes down what it read, so a move can be told from a loss and a gain next time — and so that
/// a file that is moved after it changed is recognised as the same file. What a change is told by is
/// not that reading, though, but the digest the Layout was last known to agree with: a file that
/// changed and stayed changed is reported as changed until something records it.
///
/// `rules` is the Workspace's ignore rule set, and it governs exactly one list: a path the tree
/// holds and the Layout does not name is left out of [`TreeDiff::untagged`] when the rules say so.
/// A path the Layout names is the work's already — however it was named — and a move the reading
/// found is a fact about it, so neither `lost`, `modified` nor `renamed` is the rules' to touch.
/// The filtering happens after the moves are paired, which is what keeps a file moved into an
/// ignored place a move rather than a loss beside a hidden gain.
///
/// # Errors
///
/// Returns [`TreeDiffError::Layout`] if the Layout cannot be read back, and [`TreeDiffError::Io`]
/// if the tree cannot be walked or what was found cannot be written down. A single file that
/// cannot be read is not one of these: it is recorded in [`TreeDiff::failed`].
// The whole scan is one pass: a split would have to re-establish the order its borrows are taken
// in, and the early returns the scan answers with would become a step's own.
#[allow(clippy::too_many_lines)]
pub fn tree_diff(
    layout: &Layout,
    workspace: &Workspace,
    alike: f32,
    rules: &IgnoreRuleSet,
) -> Result<TreeDiff, TreeDiffError> {
    layout.refresh()?;

    let root = workspace.get_root();
    let (found, mut failed) = scan::walk(root);

    // A variant file is content waiting to be joined into a file the Layout names rather than work
    // of its own, so it is not read at all: what is not read is neither a change nor a gain, and it
    // is not written down for the next reading to mistake for one either.
    let ignored = Merging::read(root).ignored(layout);
    let found: Vec<scan::Found> = found
        .into_iter()
        .filter(|file| !ignored.contains(&file.path))
        .collect();

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
        let read = match result {
            Ok(entry) => entry,
            Err(error) => {
                failed.push(FailedPath::new(path.as_str().to_owned(), error.to_string()));
                continue;
            }
        };

        // Reading a file again does not change what the Layout was last known to agree with: a file
        // never read before is one the Layout has not disagreed with, and one read before keeps the
        // agreement — or the disagreement — it had.
        let agree = previous
            .get(path)
            .map_or_else(|| Some(read.digest()), Entry::agreed);

        current.insert(path.clone(), read.with_agreed(agree));
    }

    // What changed is what no longer holds what the Layout agrees with — not what the last reading
    // wrote down, which the reading itself would overwrite. So a file that changed and stayed
    // changed is reported as changed until something records it.
    let mut modified: Vec<LayoutPath> = current
        .iter()
        .filter(|(path, entry)| tracked.contains(*path) && entry.agreed() != Some(entry.digest()))
        .map(|(path, _)| path.clone())
        .collect();

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
                strong: true,
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
    for (from, to, _score) in likeness(&sources, &destinations, &previous, &current, alike) {
        renamed.push(PathRename {
            from: from.clone(),
            to: to.clone(),
            // The two are not the same bytes — that is the first pass's business — so the move is
            // one the likeness suggests rather than one the content makes.
            strong: false,
        });
        moved_from.insert(from);
        moved_to.insert(to.clone());
    }

    // A move is a change as well when what the new path holds is not what the Layout agreed with at
    // the old one — which a file that was edited before it moved is, however alike the two readings
    // of it are.
    for pair in &renamed {
        let agreed = previous.get(&pair.from).and_then(Entry::agreed);
        if agreed != current.get(&pair.to).map(Entry::digest) {
            modified.push(pair.to.clone());
        }
    }

    renamed.sort_by(|left, right| left.from.cmp(&right.from));
    modified.sort_unstable();
    modified.dedup();
    failed.sort_by(|left, right| left.path.cmp(&right.path));

    // What the next reading starts from: what each file holds now, and — for a path the Layout
    // still names but the tree no longer holds — what it held when it was last seen, which is all
    // that is left of it to recognise a move by. The digest the Layout agrees with is carried along,
    // since it is not the reading that changes it.
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
    // two lists it would otherwise sit in.
    lost.retain(|path| !moved_from.contains(path));
    untagged.retain(|path| !moved_to.contains(path));

    // The ignore rules govern only what the tree gained, and only once the moves are known: a gain
    // that turned out to be where a tracked file went is a move, and a move is not the rules' to
    // hide. What is left is an addition nobody is answerable for, which is exactly what a rule is
    // written against.
    untagged.retain(|path| !rules.is_ignore(Path::new(path.as_str()), false));

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

            // UNWRAP: the thread body only builds `Entry` values from the `fs::read` results it
            // collects, so its only `Err` is a panic propagated from the OS layer, which the
            // `Result` from `join` reports here.
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
// A likeness is a ratio of byte counts: the counts themselves are exact integers, while the ratio
// is deliberately `f32`, so the loss the lint warns about is what the comparison is made of rather
// than a hazard of it.
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
/// Where the analysis of `layout`, under the Workspace at `root`, is kept.
///
/// It is named for the Layout rather than the Workspace, since each Layout describes a tree of its
/// own; a caller that reads the tree beside a Layout and a caller that has to write down what one
/// file looks like reach the same file through this.
#[must_use]
pub fn cache_path(root: &Path, layout: &Layout) -> PathBuf {
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

/// Writes down that a path the Layout names holds what it held where it was, a move having put it there.
///
/// A move is not a change. The entry the Layout agrees with is the *entry*, wherever it is named, so what a
/// reading wrote down about it where it was is what it has where it is: what the file holds is read afresh —
/// it is at another path, so the reading's own skip of a file whose time and length are what they were does
/// not apply — and the digest the Layout was known to agree with is carried over with it.
///
/// Written down as agreement with what the file holds now, a file that was changed before it moved would be a
/// file that changed nothing: the path it moved to is one the Layout names, so there is no move left to be
/// seen either, and the change is one no reading reports. What is written here is what a reading cannot work
/// out for itself, since by the time it looks, the path that would tell it is gone.
///
/// # Errors
///
/// Returns [`TreeDiffError::Io`] when the file at the path, or the cache, could not be read or written.
pub fn remember_moved(
    root: &Path,
    layout: &Layout,
    from: &LayoutPath,
    to: &LayoutPath,
) -> Result<(), TreeDiffError> {
    let disk = root.join(to.to_path_buf());

    // A path that is not a file — a move that landed nowhere — is nothing to write down.
    if !disk.is_file() {
        return Ok(());
    }

    let file = cache_path(root, layout);
    let mut cache = Cache::read(&file);
    let read = entry_of(&disk)?;

    // A path the cache has never read is one nothing has disagreed with yet, which is what a file read for the
    // first time is; a path it has read keeps the agreement — or the disagreement — that entry had.
    cache.insert(
        to.clone(),
        match cache.get(from) {
            Some(was) => read.with_agreed(was.agreed()),
            None => read,
        },
    );

    // Where it came from is left as it is: whether the Layout still names anything there is the Layout's to
    // say, and a reading that finds nothing is one that can still say the file moved.
    cache.write(&file)
}

/// What the file at `disk` holds now, as a reading of it writes it down.
///
/// It is one file read on its own, for a caller that needs what a file looks like to the analysis
/// without walking the tree around it — aligning a Layout asks for exactly this, to say what a path
/// now holds when the tree and the Layout disagree about where it went.
///
/// # Errors
///
/// Returns what the file could not be read or looked at with.
pub fn entry_of(disk: &Path) -> Result<Entry, std::io::Error> {
    let meta = fs::metadata(disk)?;
    let (seconds, nanos) = crate::scan::stamp(&meta);
    let bytes = fs::read(disk)?;
    let digest = fingerprint::digest(&bytes);
    let binary = fingerprint::is_binary(&bytes);
    let spans = (!binary).then(|| fingerprint::spans(&bytes, false));

    Ok(Entry::new(
        seconds,
        nanos,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        digest,
        spans,
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::sleep;
    use std::time::Duration;

    use rorolala_ignore::IgnoreRuleSet;
    use rorolala_layout::{LayoutPath, MutableData};
    use rorolala_utils_constants::{DEFAULT_LAYOUT_NAME, WORKSPACE_IGNORE_RULES_DIR};
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

    /// A Workspace in a directory of its own, and the Layout it works in.
    ///
    /// A Workspace is made with no Layout, since the name of the first one is the caller's to
    /// choose; every test here works in one, so the fixture makes it and works in it.
    fn workspace(label: &str) -> (PathBuf, Workspace) {
        let dir = scratch(label);
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();
        let layouts = workspace.layouts();
        layouts.create(DEFAULT_LAYOUT_NAME).unwrap();
        layouts.set_current(DEFAULT_LAYOUT_NAME).unwrap();

        (dir, workspace)
    }

    /// The Layout a Workspace works in, which the fixture has made and chosen.
    fn working(workspace: &Workspace) -> rorolala_layout::Layout {
        workspace
            .layouts()
            .get(DEFAULT_LAYOUT_NAME)
            .unwrap()
            .unwrap()
    }

    /// Makes the Layout a Workspace works in name `text`, held by no account.
    fn track(workspace: &Workspace, id: u128, text: &str) {
        let layout = working(workspace);
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

        let layout = working(&workspace);
        let diff = tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

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

        let layout = working(&workspace);
        // A first reading writes down what the file holds, so the move can be told next time.
        tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        fs::rename(dir.join("a.txt"), dir.join("b.txt")).unwrap();

        let diff = tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        assert_eq!(
            diff.renamed,
            vec![PathRename {
                from: path("a.txt"),
                to: path("b.txt"),
                strong: true,
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

        let layout = working(&workspace);
        // A first reading writes down what the file holds, so the change can be told next time.
        tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        // Long enough that the change is a change in time as well as in content.
        sleep(Duration::from_millis(20));
        fs::write(dir.join("a.txt"), b"after\n").unwrap();

        let diff = tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        assert_eq!(diff.modified, vec![path("a.txt")], "{diff:?}");
        assert!(diff.renamed.is_empty(), "{diff:?}");
    }

    #[test]
    fn a_text_file_moved_and_edited_is_moved_and_modified() {
        let (dir, workspace) = workspace("edited");
        track(&workspace, 1, "a.txt");
        fs::write(dir.join("a.txt"), b"one\ntwo\nthree\nfour\n").unwrap();

        let layout = working(&workspace);
        // A first reading writes down what the file holds, so the move can be told next time.
        tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        // The same prose, moved, with a line added: alike enough to be the same file.
        fs::remove_file(dir.join("a.txt")).unwrap();
        fs::write(dir.join("b.txt"), b"one\ntwo\nthree\nfour\nfive\n").unwrap();

        let diff = tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        assert_eq!(
            diff.renamed,
            vec![PathRename {
                from: path("a.txt"),
                to: path("b.txt"),
                strong: false,
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

        let layout = working(&workspace);
        let diff = tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        assert_eq!(diff.untagged, vec![path("seen.txt")], "{diff:?}");
    }

    /// The Workspace's own ignore rules, with `lines` written as the one rule file.
    fn rules(dir: &Path, lines: &str) -> IgnoreRuleSet {
        let held = dir.join(WORKSPACE_IGNORE_RULES_DIR);
        fs::create_dir_all(&held).unwrap();
        fs::write(held.join("default"), lines).unwrap();

        IgnoreRuleSet::load(&held, dir).unwrap()
    }

    #[test]
    fn an_ignored_gain_is_no_gain_but_a_move_found_is_still_a_move() {
        let (dir, workspace) = workspace("ignored");
        track(&workspace, 1, "kept.txt");
        fs::write(dir.join("kept.txt"), b"the same content\n").unwrap();

        let layout = working(&workspace);
        // A first reading writes the cache the move is told by.
        tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        // A file the rules ignore, and a tracked file moved into a place the rules ignore.
        fs::write(dir.join("scratch.mb"), b"scratch").unwrap();
        fs::rename(dir.join("kept.txt"), dir.join("kept.mb")).unwrap();

        let ignoring = rules(&dir, "*.mb\n");
        let diff = tree_diff(&layout, &workspace, 0.5, &ignoring).unwrap();

        assert!(diff.untagged.is_empty(), "{diff:?}");
        assert_eq!(
            diff.renamed,
            vec![PathRename {
                from: path("kept.txt"),
                to: path("kept.mb"),
                strong: true,
            }],
            "{diff:?}"
        );

        // The same reading without the rules sees the gain, since that is what the tree holds.
        let diff = tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();
        assert_eq!(diff.untagged, vec![path("scratch.mb")], "{diff:?}");
    }

    #[test]
    fn an_addition_the_rules_ignore_is_hidden_even_when_its_twin_is_lost() {
        let (dir, workspace) = workspace("ignored-lost");
        track(&workspace, 1, "gone.txt");

        // `gone.txt` is lost, and what turned up beside it matches a rule: the move nobody found
        // reads as a loss alone, which is the ordinary shape of a file that went somewhere the
        // rules govern.
        fs::write(dir.join("scratch.mb"), b"scratch").unwrap();

        let layout = working(&workspace);
        let ignoring = rules(&dir, "*.mb\n");
        let diff = tree_diff(&layout, &workspace, 0.5, &ignoring).unwrap();

        assert_eq!(diff.lost, vec![path("gone.txt")], "{diff:?}");
        assert!(diff.untagged.is_empty(), "{diff:?}");
        assert!(diff.renamed.is_empty(), "{diff:?}");
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

        let layout = working(&workspace);
        let diff = tree_diff(&layout, &workspace, 0.5, &IgnoreRuleSet::empty()).unwrap();

        assert_eq!(diff.untagged, expected, "{diff:?}");
        assert!(diff.failed.is_empty(), "{diff:?}");
    }
}

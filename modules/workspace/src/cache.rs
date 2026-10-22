//! The read-only copy of a Vault's Layout, as a Workspace keeps it.
//!
//! A copy is published in **generations**, and the generation a reader is to open is named by a
//! pointer file beside them. It is kept this way because of what a directory costs on Windows: a
//! directory whose files another process holds open cannot be renamed, so a copy that is replaced by
//! renaming the copy itself fails with "Access is denied" whenever anything is reading it at the
//! same moment — a status reading, a plan, another fetch. A generation is therefore written whole
//! under a name of its own and never renamed again, and what moves instead is the pointer: a file,
//! which may be replaced while it is open.
//!
//! The pointer is the whole of what says where the copy is. A container whose pointer cannot be read
//! is a container nothing is known about, so nothing there is removed.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::Workspace;

/// The file, inside a copy's container, that names the generation a reader is to open.
const POINTER: &str = "CURRENT";

/// What the directory of a generation is written with before the part that makes it unique.
///
/// It is also what tells a generation apart from everything else a container may hold, so a pointer
/// naming anything else is refused rather than followed.
const GENERATION: &str = "g-";

/// What the directory a generation is being written into carries after its name, before it is published.
const STAGING: &str = ".staging";

/// How old what a container holds has to be before a fetch treats it as left behind.
///
/// A fetch of another run, or a reading of the copy, is seconds long: ten minutes is comfortably
/// longer than any of them, and short enough that what a crash left behind does not stay for long.
/// Nothing younger is touched, which is what keeps one run from removing a generation another is
/// part of the way through writing.
const LEFTOVER_AGE: Duration = Duration::from_secs(600);

/// The one file a Layout's own directory always holds, which says a directory is a copy of one.
///
/// [`Layout::open_existing`](rorolala_layout::Layout::open_existing) is what a reader opens a copy
/// with, and it refuses a directory that holds none of the Layout's files; this is the name it
/// looks for first, so asking about it says whether a directory is a copy without opening one.
const FIRST_LOG: &str = "log-0.wal";

/// The read-only copy of one Vault's Layout, as one Workspace keeps it.
///
/// A copy is one container holding a pointer and a directory per generation, and the pointer says
/// which generation is the copy. A run that fetches writes a generation and then points the copy at
/// it; a run that reads follows the pointer and opens what it names. Neither ever renames a
/// directory a reader may be holding, which is the whole reason for the shape.
pub struct LayoutCopy {
    /// The directory the pointer and the generations are kept in.
    container: PathBuf,
}

impl LayoutCopy {
    /// The copy of `layout` of `vault`, as `workspace` keeps it.
    #[must_use]
    pub fn of(workspace: &Workspace, vault: &str, layout: &str) -> Self {
        Self {
            container: workspace.readonly_layouts_dir(vault, layout),
        }
    }

    /// The directory the copy is kept in: the pointer, and a directory per generation.
    #[must_use]
    pub fn container(&self) -> &Path {
        self.container.as_path()
    }

    /// The generation a reader is to open, or nothing when no copy is here.
    ///
    /// A copy fetched before this Workspace kept generations is the container itself, and is
    /// answered as it stands: a directory carrying the Layout's own files is a copy whichever shape
    /// it was written in, so a Workspace does not have to fetch again for a change in how a copy is
    /// kept. The next fetch publishes a generation and clears the old shape.
    #[must_use]
    pub fn current(&self) -> Option<PathBuf> {
        if let Some(generation) = self.pointed() {
            let path = self.container.join(generation);

            return path.is_dir().then_some(path);
        }

        self.container
            .join(FIRST_LOG)
            .is_file()
            .then(|| self.container.clone())
    }

    /// A directory a new generation is to be written into, named so no other run uses it.
    ///
    /// What is made here is only a name: the directory is the caller's to create, and the fetch
    /// that writes into it is the one that knows what a generation holds.
    #[must_use]
    pub fn stage(&self) -> PathBuf {
        self.container
            .join(format!("{GENERATION}{}{STAGING}", unique()))
    }

    /// Publishes the generation written at `stage`, so that readers open it from now on.
    ///
    /// The generation is moved out of staging under a name no other directory has, and only then is
    /// the pointer replaced. A run that stops between the two leaves a generation no pointer names,
    /// which is cleared by a later fetch rather than read by a reader.
    ///
    /// # Errors
    ///
    /// Returns why the generation could not be taken out of staging, or why the pointer could not be
    /// written.
    pub fn publish(&self, stage: &Path) -> io::Result<()> {
        let Some(name) = stage.file_name().and_then(OsStr::to_str) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a staged generation is written without a name",
            ));
        };
        let Some(generation) = name.strip_suffix(STAGING) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a staged generation is not marked as one",
            ));
        };

        // The name this lands under is one no other run has used, so this is a move and never a
        // replacement: nothing a reader has open is being renamed over.
        fs::rename(stage, self.container.join(generation))?;

        let temporary = self
            .container
            .join(format!("{POINTER}.{}{STAGING}", unique()));
        fs::write(&temporary, format!("{generation}\n"))?;

        // The pointer is replaced rather than written over, so a reader sees the generation before it
        // or the one after it, and never a file that is half of each.
        if let Err(error) = fs::rename(&temporary, self.container.join(POINTER)) {
            let _ = fs::remove_file(&temporary);

            return Err(error);
        }

        Ok(())
    }

    /// Removes what earlier runs left in the container.
    ///
    /// What is cleared is everything the copy is not: a generation no pointer names, the shape a
    /// copy was kept in before generations, and a staging directory a run that died left. Nothing
    /// the pointer names is touched, and nothing younger than the leftover age before `started` is
    /// either — a run of another fetch is a few seconds long, and removing what it is writing would
    /// fail it for no reason.
    ///
    /// A container whose pointer cannot be read is left as it is: nothing there is known to be
    /// garbage, and the one generation that may be the copy is not something to find out by trying.
    pub fn drain(&self, started: SystemTime) {
        let Some(current) = self.pointed() else {
            return;
        };
        let cutoff = started.checked_sub(LEFTOVER_AGE).unwrap_or(started);

        let Ok(entries) = fs::read_dir(&self.container) else {
            return;
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };

            if name == POINTER || name == current.as_str() {
                continue;
            }

            let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
                continue;
            };
            if modified > cutoff {
                continue;
            }

            let path = entry.path();
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
        }
    }

    /// The generation the pointer names, when it names one a container of this shape could hold.
    fn pointed(&self) -> Option<String> {
        let text = fs::read_to_string(self.container.join(POINTER)).ok()?;
        let name = text.trim();

        (name.starts_with(GENERATION) && is_plain_name(name)).then(|| name.to_owned())
    }
}

/// A name no other run is using, for a generation of its own.
///
/// The process and the moment say which run made it, and a counter says which generation of that run
/// it is: two runs are two processes, and one run writes one generation at a time.
fn unique() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();

    format!(
        "{}-{}-{nanos}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// Whether `name` is one path component and nothing else.
///
/// A pointer is read as a directory name and joined onto the container, so a name that names
/// somewhere else — a path, a climbing name, a drive — would be read outside the one directory the
/// copy is for.
fn is_plain_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;

    use rorolala_utils_location::Locate as _;

    use crate::Workspace;

    /// A Workspace of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> (PathBuf, Workspace) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let root = std::env::temp_dir().join(format!(
            "rorolala-cache-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(crate::DATA_DIR)).unwrap();

        let workspace = Workspace::locate(&root).unwrap();

        (root, workspace)
    }

    /// A copy's container under a Workspace, which is where a fetch writes.
    fn container(workspace: &Workspace) -> PathBuf {
        workspace.readonly_layouts_dir("vault", "truth")
    }

    /// A generation written by hand, so that publishing is the only thing under test.
    fn staged(copy: &super::LayoutCopy, name: &str) -> PathBuf {
        let stage = copy.stage();
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join(name), b"a log").unwrap();

        stage
    }

    #[test]
    fn a_workspace_that_fetched_nothing_has_no_copy() {
        let (root, workspace) = scratch("none");

        assert!(workspace.readonly_layout_copy("vault", "truth").is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_published_generation_is_what_a_reader_opens() {
        let (root, workspace) = scratch("published");
        let copy = super::LayoutCopy::of(&workspace, "vault", "truth");

        let stage = staged(&copy, "log-0.wal");
        copy.publish(&stage).unwrap();

        let current = workspace.readonly_layout_copy("vault", "truth").unwrap();

        assert!(current.starts_with(container(&workspace)));
        assert!(current.join("log-0.wal").is_file(), "{current:?}");
        assert!(
            !stage.exists(),
            "the staging directory outlived its publication"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_copy_kept_before_generations_is_read_as_it_stands() {
        // The shape an older build wrote: the copy is the container, and no pointer names anything.
        let (root, workspace) = scratch("legacy");
        let legacy = container(&workspace);
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("log-0.wal"), b"a log").unwrap();

        assert_eq!(
            workspace.readonly_layout_copy("vault", "truth"),
            Some(legacy.clone())
        );

        // A container with no copy in it is not one, so a reader is told nothing was fetched rather
        // than opening a directory and making a Layout out of it.
        fs::remove_file(legacy.join("log-0.wal")).unwrap();

        assert!(workspace.readonly_layout_copy("vault", "truth").is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pointer_that_names_somewhere_else_is_not_followed() {
        let (root, workspace) = scratch("climb");
        let legacy = container(&workspace);
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(super::POINTER), "../g-elsewhere\n").unwrap();

        assert!(workspace.readonly_layout_copy("vault", "truth").is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn draining_clears_what_the_pointer_does_not_name_and_keeps_what_it_does() {
        let (root, workspace) = scratch("drained");
        let copy = super::LayoutCopy::of(&workspace, "vault", "truth");

        let first = staged(&copy, "log-0.wal");
        copy.publish(&first).unwrap();
        let kept = workspace.readonly_layout_copy("vault", "truth").unwrap();

        // A generation the pointer does not name, and the shape an older copy was kept in.
        let old = container(&workspace).join("g-1-0-1");
        fs::create_dir_all(&old).unwrap();
        fs::write(container(&workspace).join("log-7.wal"), b"old").unwrap();

        // Drained by a fetch that starts later than they were made, which is the only moment a run may
        // take them for what an earlier run left: a run that starts now leaves what is younger than the
        // grace alone, which the next test says.
        copy.drain(SystemTime::now() + super::LEFTOVER_AGE + std::time::Duration::from_secs(1));

        assert!(kept.is_dir(), "the pointer's own generation was removed");
        assert!(!old.exists(), "a generation no pointer names was kept");
        assert!(!container(&workspace).join("log-7.wal").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn draining_leaves_what_another_run_is_still_writing() {
        let (root, workspace) = scratch("young");
        let copy = super::LayoutCopy::of(&workspace, "vault", "truth");

        let first = staged(&copy, "log-0.wal");
        copy.publish(&first).unwrap();

        let running = copy.stage();
        fs::create_dir_all(&running).unwrap();

        copy.drain(SystemTime::now());

        assert!(
            running.is_dir(),
            "a staging directory written now was removed"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn draining_a_container_with_no_pointer_removes_nothing() {
        let (root, workspace) = scratch("unpointed");
        let copy = super::LayoutCopy::of(&workspace, "vault", "truth");

        let kept = container(&workspace).join("g-9-0-9");
        fs::create_dir_all(&kept).unwrap();
        fs::write(container(&workspace).join(Path::new("note")), b"kept").unwrap();

        copy.drain(SystemTime::now());

        assert!(
            kept.is_dir(),
            "a generation was removed without a pointer to say what is garbage"
        );
        assert!(container(&workspace).join("note").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_last_published_generation_is_the_one_a_reader_opens() {
        let (root, workspace) = scratch("newest");
        let copy = super::LayoutCopy::of(&workspace, "vault", "truth");

        let first = staged(&copy, "log-0.wal");
        copy.publish(&first).unwrap();
        let first = workspace.readonly_layout_copy("vault", "truth").unwrap();

        let second = staged(&copy, "log-0.wal");
        copy.publish(&second).unwrap();
        let second = workspace.readonly_layout_copy("vault", "truth").unwrap();

        assert_ne!(first, second);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn two_runs_that_publish_at_once_leave_a_pointer_that_names_a_whole_generation() {
        // Two runs of a fetch, as two processes are: neither is told about the other, and both name
        // the copy. What has to hold is that whichever pointer write lands last, what it names is a
        // generation that was written whole — never a directory that was still being filled.
        let (root, workspace) = scratch("raced");

        let one = super::LayoutCopy::of(&workspace, "vault", "truth");
        let other = super::LayoutCopy::of(&workspace, "vault", "truth");

        let first = std::thread::spawn(move || {
            let stage = staged(&one, "log-0.wal");
            one.publish(&stage).unwrap();
        });
        let second = std::thread::spawn(move || {
            let stage = staged(&other, "log-0.wal");
            other.publish(&stage).unwrap();
        });

        first.join().unwrap();
        second.join().unwrap();

        let current = workspace
            .readonly_layout_copy("vault", "truth")
            .expect("a copy after two publications");

        assert!(current.join("log-0.wal").is_file(), "{current:?}");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_copy_kept_both_ways_is_read_from_the_pointer_and_drained_of_the_old_shape() {
        let (root, workspace) = scratch("both");
        let copy = super::LayoutCopy::of(&workspace, "vault", "truth");

        // A copy an older build kept: the container itself holds the Layout, and nothing points.
        let legacy = container(&workspace);
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("log-0.wal"), b"old").unwrap();

        assert_eq!(
            workspace.readonly_layout_copy("vault", "truth"),
            Some(legacy.clone())
        );

        // A fetch publishes a generation beside it, and from then on the pointer is what is read.
        let stage = staged(&copy, "log-0.wal");
        copy.publish(&stage).unwrap();
        let published = workspace.readonly_layout_copy("vault", "truth").unwrap();

        assert_ne!(published, legacy);
        assert!(published.starts_with(&legacy));

        // Drained by a run that starts later than either was made: the old shape goes, and what the
        // pointer names stays.
        copy.drain(SystemTime::now() + super::LEFTOVER_AGE + std::time::Duration::from_secs(1));

        assert!(
            !legacy.join("log-0.wal").exists(),
            "the copy kept the old way was left behind"
        );
        assert!(
            published.is_dir(),
            "the pointer's own generation was removed"
        );

        let _ = fs::remove_dir_all(&root);
    }
}

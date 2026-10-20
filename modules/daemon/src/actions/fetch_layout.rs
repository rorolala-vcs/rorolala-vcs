//! `fetch-layout`: the Layout a Vault holds, brought here whole.
//!
//! A Workspace works in Layouts of its own and keeps one it did not make: a copy of what a Vault
//! holds. It is fetched rather than synced, and nothing moves the other way — what a Workspace
//! finds in the copy is who holds what upstream, so the copy is read and not worked in.
//!
//! What crosses is the Vault's Layout directory as it stands, every file of it, and the Workspace
//! writes that under its cache named by the Vault. The whole directory is one blob: a Layout is a
//! fixed set of small files beside snapshots, and carrying them one at a time would say the same
//! thing in more exchanges.
//!
//! What a fetch writes is a **generation** of the copy, published by moving a pointer — see
//! [`LayoutCopy`] for why. A copy is therefore never renamed over, and a run that is reading one
//! cannot make a fetch fail.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::time::SystemTime;

use rorolala_errors::IoError;
use rorolala_layout::Layout;
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_workspace::LayoutCopy;

use super::layout_remote::local_layout;
use super::sync_storage::{Side, carry_blob};

/// An Action that brings the Vault's Layout to the Workspace's read-only cache.
///
/// # Input
///
/// The name the Workspace keeps the fetched Layout under — the name it bound the Vault by. It
/// belongs to the Workspace and is read there; the Vault is handed it and has no use for it, since
/// which Vault is reached is the address's to say. The Vault keeps one Layout, so what lands under
/// that name is the one known as [`VAULT_LAYOUT_NAME`].
///
/// # Output
///
/// Nothing: what a fetch comes to is a directory the Workspace now holds, and a caller that wants
/// to say where it landed says it in its own words.
pub struct ActionFetchLayout;

impl Action for ActionFetchLayout {
    /// The fetch is id 10: the first thing asked of a Vault that is not a store or an index.
    const ID: u32 = 10;

    type Input = String;
    type Output = ();

    /// Asks the Vault for its Layout, and writes what it answers into the Workspace's cache.
    ///
    /// # Errors
    ///
    /// Returns [`ActionError::MissingValue`] if this side was handed neither a Workspace nor a
    /// Vault, [`ActionError::Store`] if the Vault sent something that is not a Layout, and
    /// [`ActionError::LocalFile`] if the Layout could not be read where it is or the copy could not
    /// be written here — besides whatever the exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // Which directory the copy lands in is the Workspace's own business, so the name is read
        // here and never looked at on the Vault.
        let cache = input.into_inner().unwrap_or_default();

        // What the Vault holds is read there: it is the side with a Layout of its own, and the
        // Workspace side has none to read.
        let held = if ctx.is_vault() {
            Some(read_layout(&local_layout(&ctx)?)?)
        } else {
            None
        };
        let mine = held
            .as_ref()
            .map(rorolala_protocol::Encodable::encode)
            .transpose()
            .map_err(codec_failed)?;

        // What the Vault holds crosses back, and the Workspace is where it is written: the same
        // call reads both ways, so the side that sent it keeps its own.
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        if ctx.is_workspace() {
            let files = Vec::<(String, Vec<u8>)>::decode(bytes).map_err(codec_failed)?;
            write_cache(&ctx, &cache, &files)?;
        }

        Ok(())
    }
}

/// Every file the Layout keeps, by the name it is kept under, in name order.
///
/// The directory is read as it stands rather than by the names a Layout is known to keep: what a
/// fetch is for is the Layout as the Vault has it, and a file this build does not know is still
/// part of what a later build may read. Directories are passed over, since a Layout keeps none.
///
/// # Errors
///
/// Returns [`ActionError::LocalFile`] naming the directory or the file when one cannot be read.
fn read_layout(layout: &Layout) -> Result<Vec<(String, Vec<u8>)>, ActionError> {
    let dir = layout.dir();
    let mut files = Vec::new();

    for entry in fs::read_dir(dir).map_err(|error| local(dir, &error))? {
        let entry = entry.map_err(|error| local(dir, &error))?;
        let path = entry.path();

        if !entry
            .file_type()
            .map_err(|error| local(&path, &error))?
            .is_file()
        {
            continue;
        }

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };

        files.push((name, fs::read(&path).map_err(|error| local(&path, &error))?));
    }

    files.sort();

    Ok(files)
}

/// Writes the fetched files as a new generation of the Workspace's copy.
///
/// What replaces the copy is written whole before the pointer is moved, so a fetch that fails part
/// of the way through leaves the copy that is here untouched: a reader is never handed a directory
/// that is half of what the Vault holds. The old generation is not removed here — a reader may be
/// part of the way through it — and is cleared by a later fetch.
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if this side has no Workspace, [`ActionError::Store`] if the
/// Vault sent something that is not a Layout this side can keep, and [`ActionError::LocalFile`] if
/// the copy cannot be written.
fn write_cache(
    ctx: &ActionContext<'_>,
    name: &str,
    files: &[(String, Vec<u8>)],
) -> Result<(), ActionError> {
    let workspace = ctx
        .current_workspace()
        .into_inner()
        .ok_or(ActionError::MissingValue)?;

    if !is_plain_name(name) {
        return Err(ActionError::Store(format!(
            "`{name}` is not a name a fetched Layout can be kept under"
        )));
    }

    check(files)?;

    let copy = LayoutCopy::of(workspace, name, VAULT_LAYOUT_NAME);

    publish(&copy, files)
}

/// Writes `files` as a generation of `copy`, points the copy at it, and clears what earlier runs left.
///
/// The clearing is done last, and only after the copy is pointed at the new generation: a fetch that
/// fails part of the way through costs nothing at all — the copy that was here is still the copy, and
/// still the one named. What is cleared is everything the pointer does not name and nothing young
/// enough to be another run's work, which is what keeps a crash from accumulating a copy's worth of
/// directories under the cache without removing what another run is writing.
///
/// # Errors
///
/// Returns [`ActionError::LocalFile`] naming the directory or the file when the generation cannot
/// be written, and the publication's own failure when it cannot be pointed at.
fn publish(copy: &LayoutCopy, files: &[(String, Vec<u8>)]) -> Result<(), ActionError> {
    let started = SystemTime::now();
    let staging = copy.stage();
    fs::create_dir_all(&staging).map_err(|error| local(&staging, &error))?;

    for (file, bytes) in files {
        let path = staging.join(file);

        if let Err(error) = write_whole(&path, bytes) {
            // The copy that is here was never touched, and what was written of the new one is no use
            // to anyone.
            let _ = fs::remove_dir_all(&staging);

            return Err(local(&path, &error));
        }
    }

    copy.publish(&staging).map_err(|error| {
        let _ = fs::remove_dir_all(&staging);

        local(copy.container(), &error)
    })?;

    copy.drain(started);

    Ok(())
}

/// Refuses what a Vault sends that is not a Layout this side can keep.
///
/// It runs before anything is written, so a name this refuses costs nothing: validation inside the
/// write loop was what lost the old copy when the last name the Vault sent was not one.
///
/// A Layout is at least the sixteen logs `Layout::open` makes, because the Vault opens its own
/// Layout before reading it: nothing at all is not a Layout, and keeping it would replace a good
/// copy with an empty one.
///
/// # Errors
///
/// Returns [`ActionError::Store`] for a name that is not a file name, or for nothing sent at all.
fn check(files: &[(String, Vec<u8>)]) -> Result<(), ActionError> {
    if files.is_empty() {
        return Err(ActionError::Store(
            "the Vault sent no Layout to keep, so the copy that is here is left as it is"
                .to_owned(),
        ));
    }

    for (file, _) in files {
        if !is_plain_name(file) {
            return Err(ActionError::Store(format!(
                "the Vault sent `{file}`, which is not a file name"
            )));
        }
    }

    Ok(())
}

/// Writes `bytes` to `path`, and does not return until they are on disk.
///
/// `fs::write` leaves the bytes with the operating system rather than on the disk, and what a copy
/// is read from after a crash should be what the Vault sent.
///
/// # Errors
///
/// Returns why the file could not be made, written or flushed.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// What a file that could not be read or written means to an action.
///
/// The path is part of what is said: a failure that answers "access is denied" and nothing else is
/// one a reader cannot go and look at, and the two sides of a fetch fail on files of their own — the
/// Vault's Layout at one end, the copy at the other.
fn local(path: &Path, error: &std::io::Error) -> ActionError {
    ActionError::LocalFile(IoError::new(
        error.kind().into(),
        format!("`{}`: {error}", path.display()),
    ))
}

/// Whether `name` is one path component and nothing else.
///
/// What a Vault sends is written under the cache by the name it was sent under, so a name that
/// names somewhere else — a path, a climbing name, a drive — would write outside the one directory
/// the fetch is for. A name this refuses is a Layout directory that is not one.
fn is_plain_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(rorolala_errors::BincodeError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_constants::VAULT_LAYOUT_NAME;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::{LayoutCopy, Workspace};

    use super::{check, publish};

    /// A Workspace of its own under the temp directory, emptied first so a rerun starts clean.
    ///
    /// It is under the project rather than the system's temporary directory so that a run on a
    /// machine whose temporary directory is sandboxed still writes where it says it does.
    fn scratch(label: &str) -> (PathBuf, Workspace) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let root = std::env::temp_dir().join(format!(
            "rorolala-fetch-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".rola")).unwrap();

        let workspace = Workspace::locate(&root).unwrap();

        (root, workspace)
    }

    /// The copy a fetch writes into, for a Vault the Workspace keeps under `vault`.
    fn copy(workspace: &Workspace, vault: &str) -> LayoutCopy {
        LayoutCopy::of(workspace, vault, VAULT_LAYOUT_NAME)
    }

    /// A Layout's worth of files, as the Vault would send them.
    fn sent(files: &[(&str, &str)]) -> Vec<(String, Vec<u8>)> {
        files
            .iter()
            .map(|(name, bytes)| ((*name).to_owned(), bytes.as_bytes().to_vec()))
            .collect()
    }

    /// The names in a directory, in name order.
    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        found
    }

    #[test]
    fn a_copy_is_published_whole() {
        let (root, workspace) = scratch("whole");
        let copy = copy(&workspace, "vault");

        publish(&copy, &sent(&[("log-0", "old"), ("an-old-file", "gone")])).unwrap();
        let first = workspace
            .readonly_layout_copy("vault", VAULT_LAYOUT_NAME)
            .unwrap();

        publish(&copy, &sent(&[("log-0", "new"), ("log-1", "new")])).unwrap();
        let second = workspace
            .readonly_layout_copy("vault", VAULT_LAYOUT_NAME)
            .unwrap();

        // What the Vault no longer keeps does not survive into the next generation...
        assert_eq!(names(&second), ["log-0", "log-1"]);
        assert_eq!(fs::read(second.join("log-0")).unwrap(), b"new");

        // ...and the generation before it is still whole, so a reader that resolved it before the
        // fetch still has what it opened.
        assert_eq!(names(&first), ["an-old-file", "log-0"]);
        assert_eq!(fs::read(first.join("log-0")).unwrap(), b"old");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_write_that_fails_leaves_the_copy_that_is_here() {
        let (root, workspace) = scratch("failed");
        let copy = copy(&workspace, "vault");

        publish(&copy, &sent(&[("log-0", "old")])).unwrap();
        let before = workspace
            .readonly_layout_copy("vault", VAULT_LAYOUT_NAME)
            .unwrap();

        // A name a Layout may be keyed by is not a name a file may be made with: the write fails
        // after the copy would have been replaced by the old order, which is the bug this is
        // written for.
        let refused = publish(&copy, &sent(&[("log-0", "new"), ("a\u{0}b", "new")]));

        assert!(refused.is_err());

        let after = workspace
            .readonly_layout_copy("vault", VAULT_LAYOUT_NAME)
            .expect("the copy that was here is still the copy");

        assert_eq!(after, before);
        assert_eq!(fs::read(after.join("log-0")).unwrap(), b"old");

        // What was written of the generation that failed is no use to anyone, and is not left where
        // a reader could open it.
        assert_eq!(
            names(copy.container())
                .into_iter()
                .filter(|name| name.ends_with(".staging"))
                .count(),
            0,
            "a staging directory was left behind"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_sent_at_all_is_refused_and_the_copy_is_untouched() {
        let (root, workspace) = scratch("empty");
        let copy = copy(&workspace, "vault");

        publish(&copy, &sent(&[("log-0", "old")])).unwrap();
        let before = workspace
            .readonly_layout_copy("vault", VAULT_LAYOUT_NAME)
            .unwrap();

        assert!(check(&[]).is_err());

        // `check` runs before anything is written, so what is here is what was here.
        assert_eq!(
            workspace.readonly_layout_copy("vault", VAULT_LAYOUT_NAME),
            Some(before)
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_name_that_is_not_a_file_name_is_refused() {
        for name in ["a/b", "../a", "", "."] {
            assert!(
                check(&[(name.to_owned(), Vec::new())]).is_err(),
                "`{name}` was taken as a file name"
            );
        }

        assert!(check(&sent(&[("log-0", "x")])).is_ok());
    }
}

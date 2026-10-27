//! `fetch-layout`: the Layout a Vault holds, brought here whole.
//!
//! A Workspace works in Layouts of its own and keeps one it did not make: a copy of what a Vault
//! holds. It is fetched rather than synced, and nothing moves the other way — what a Workspace
//! finds in the copy is who holds what upstream, so the copy is read and not worked in.
//!
//! What crosses is the Vault's Layout **written out whole**: every shard is a snapshot holding all
//! of what it holds, with an empty log after it. That is a layout a reader opens without replaying
//! anything, and it is the layout the Vault holds at the moment it answers rather than the files it
//! keeps it in — a snapshot and a log on disk are a change apart whenever another action is writing
//! or compacting, so reading the directory as it stands would carry away the two halves of two
//! states.
//!
//! What a fetch writes is a **generation** of the copy, published by moving a pointer — see
//! [`LayoutCopy`] for why. A copy is therefore never renamed over, and a run that is reading one
//! cannot make a fetch fail.
//!
//! A fetch asks with what the copy here **is**, in the terms the Vault can compare with its own: the
//! shape of every file — the name, the length and the hash — as the copy's own state writes out. A
//! Vault whose layout is that very state answers "unchanged" instead of carrying it again. Because
//! what a layout is written out as depends on what it holds and not on how it is kept, a compaction
//! changes nothing about that answer, and a copy that was written to after it was fetched — an
//! ownership change followed into it — answers for the state it was brought to rather than for the
//! files it arrived in.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::time::SystemTime;

use rorolala_errors::IoError;
use rorolala_layout::Layout;
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_workspace::LayoutCopy;
use serde::{Deserialize, Serialize};

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
    /// Vault, [`ActionError::Store`] if the Vault's Layout could not be written out,
    /// [`ActionError::LocalFile`] if the copy could not be written here — besides whatever the
    /// exchange fails with.
    async fn process(
        input: OnlyWorkspace<Self::Input>,
        mut ctx: ActionContext<'_>,
    ) -> Result<Self::Output, ActionError> {
        // Which directory the copy lands in is the Workspace's own business, so the name is read
        // here and never looked at on the Vault.
        let cache = input.into_inner().unwrap_or_default();

        // What the copy here is, in the terms the Vault can compare with its own. The Vault side has
        // no Workspace to look at, so its wrapper is empty and it is told this one value.
        let here = ctx.only_workspace(|| current_shapes(&ctx, &cache));
        let held = ctx.transfer(here).await?.into_inner().flatten();

        // What the Vault holds is written out there: it is the side with a Layout, and the Workspace
        // side has none to read.
        let answer = if ctx.is_vault() {
            let layout = local_layout(&ctx)?;
            let files = layout
                .as_files()
                .map_err(|error| ActionError::Store(error.to_string()))?;

            Some(if Some(FileShape::of(&files)) == held {
                Fetched::Unchanged
            } else {
                Fetched::Files(files)
            })
        } else {
            None
        };
        let mine = answer
            .as_ref()
            .map(rorolala_protocol::Encodable::encode)
            .transpose()
            .map_err(codec_failed)?;

        // What the Vault holds crosses back, and the Workspace is where it is written: the same
        // call reads both ways, so the side that sent it keeps its own.
        let arrived = carry_blob(&mut ctx, Side::Workspace, mine.clone()).await?;
        let bytes = arrived.or(mine).unwrap_or_default();

        if ctx.is_workspace() {
            match Fetched::decode(bytes).map_err(codec_failed)? {
                // The copy here is already the layout the Vault holds, so this fetch wrote nothing.
                Fetched::Unchanged => {}
                Fetched::Files(files) => write_cache(&ctx, &cache, &files)?,
            }
        }

        Ok(())
    }
}

/// What one file of a layout is, without the bytes themselves.
///
/// It is how a copy says what it holds: the Vault writes its layout out the same way, so a copy
/// whose every file is the length and hash its shapes name is the copy the Vault's layout already is,
/// and nothing has to cross to find that out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FileShape {
    /// The name the file is kept under.
    name: String,
    /// How many bytes it holds.
    len: u64,
    /// What those bytes hash to.
    digest: [u8; 32],
}

impl FileShape {
    /// The shape of each of `files`, in the order they are given.
    fn of(files: &[(String, Vec<u8>)]) -> Vec<Self> {
        files
            .iter()
            .map(|(name, bytes)| Self {
                name: name.clone(),
                len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                digest: *blake3::hash(bytes).as_bytes(),
            })
            .collect()
    }
}

/// What a fetch answers with.
#[derive(Debug, Serialize, Deserialize)]
enum Fetched {
    /// The Vault's Layout is what the copy the fetch was asked with already holds.
    Unchanged,
    /// The files of the Vault's Layout, by the name each is kept under.
    Files(Vec<(String, Vec<u8>)>),
}

/// The shapes the copy this side keeps of `name` would be written out as, when it holds a copy.
///
/// What is asked is the copy's own state rather than the files it arrived as: a copy that an
/// ownership change was followed into holds a change its files' shapes no longer describe, and one
/// read back as a layout says what it holds however it is kept. A directory that is not a copy — a
/// fetch cut short, or one this build did not write — is no shapes at all, which is a copy asked for
/// whole rather than taken as it stands.
fn current_shapes(ctx: &ActionContext<'_>, name: &str) -> Option<Vec<FileShape>> {
    let workspace = ctx.current_workspace().into_inner()?;
    let dir = LayoutCopy::of(workspace, name, VAULT_LAYOUT_NAME).current()?;
    let layout = Layout::open_existing(&dir).ok()?;
    let files = layout.as_files().ok()?;

    Some(FileShape::of(&files))
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

    if let Err(error) = sync_dir(&staging) {
        let _ = fs::remove_dir_all(&staging);

        return Err(local(&staging, &error));
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
/// A Layout is at least the sixteen logs and snapshots a fetch writes out, so nothing at all is not
/// a Layout, and keeping it would replace a good copy with an empty one.
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

/// Writes `bytes` to `path`.
///
/// What a fetch writes is a copy rather than the truth: the Vault still holds the Layout this was
/// taken from, so a machine that lost power before the copy reached the disk has lost this fetch
/// and not what it was a copy of. The bytes are therefore left with the operating system rather
/// than flushed one file at a time, which on a directory of small files is most of what a fetch
/// costs — and one flush of the generation as a whole is asked for instead, where there is one.
///
/// # Errors
///
/// Returns why the file could not be made or written.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)
}

/// Pushes the generation directory to the disk as a whole, where an operating system can be asked.
///
/// It is the one flush a fetch spends: a directory is a file a Unix system will flush, and one on
/// Windows is not — so what a machine cut off between the writes and the pointer costs there is this
/// fetch, which is the cost the copy was chosen to take rather than a cost on every fetch.
///
/// # Errors
///
/// Returns why the directory could not be flushed.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    fs::File::open(dir)?.sync_all()
}

/// Pushes the generation directory to the disk as a whole, where an operating system can be asked.
///
/// A Windows system keeps no handle on a directory to flush, so this is nothing: see the reason the
/// copy takes a whole fetch from a machine cut off rather than a flush of every file.
///
/// # Errors
///
/// Nothing, so this never fails.
#[cfg(not(unix))]
#[allow(
    clippy::missing_const_for_fn,
    clippy::unnecessary_wraps // one call for both systems: the Unix one can fail and this one cannot
)]
fn sync_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

/// What a file that could not be read or written means to an action.
///
/// The path is part of what is said: a failure that answers "access is denied" and nothing else is
/// one a reader cannot go and look at, and the two sides of a fetch fail on files of their own — the
/// copy at one end, the generation it is written as at the other.
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

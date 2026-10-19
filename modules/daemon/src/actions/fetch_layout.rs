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

use std::fs;
use std::io::Write as _;
use std::path::{Component, Path};

use rorolala_layout::Layout;
use rorolala_protocol::{Action, ActionContext, ActionError, Encodable as _, OnlyWorkspace};
use rorolala_utils_constants::{VAULT_LAYOUT_NAME, WORKSPACE_READONLY_LAYOUTS_DIR};
use rorolala_utils_location::Locate as _;

use super::layout_remote::local_layout;
use super::sync_storage::{Side, carry_blob};

/// The mark a fetch gives the directory it writes the new copy into, before that copy takes the place of the old one.
const FETCHING: &str = "fetching-";

/// The mark the copy being replaced carries while the new one is moved into its place.
const KEEPING: &str = "keeping-";

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
    /// Vault, [`ActionError::Store`] if the Vault's Layout could not be read or the cache written,
    /// and whatever the exchange fails with.
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
/// Returns [`ActionError::Io`] if the directory or a file in it cannot be read.
fn read_layout(layout: &Layout) -> Result<Vec<(String, Vec<u8>)>, ActionError> {
    let mut files = Vec::new();

    for entry in fs::read_dir(layout.dir())? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };

        files.push((name, fs::read(entry.path())?));
    }

    files.sort();

    Ok(files)
}

/// Writes the fetched files under the Workspace's cache, replacing whatever was there.
///
/// The old copy is taken away rather than written over, so a file the Vault no longer keeps does
/// not survive in the copy: what is read afterwards is the Vault's Layout and not that Layout with
/// older files beside it.
///
/// What replaces it is written whole before the old copy is touched — see [`replace`] — because a
/// fetch that fails part of the way through would otherwise leave an empty or half-written
/// directory where a good copy was. A directory is not a copy: a reader that met one would answer
/// "the Vault's Layout holds nothing" rather than "nothing was fetched".
///
/// # Errors
///
/// Returns [`ActionError::MissingValue`] if this side has no Workspace, [`ActionError::Store`] if the
/// Vault sent something that is not a Layout, and [`ActionError::Io`] if the copy cannot be made.
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

    let root = workspace
        .get_root()
        .join(WORKSPACE_READONLY_LAYOUTS_DIR)
        .join(name);
    let dir = root.join(VAULT_LAYOUT_NAME);

    fs::create_dir_all(&root)?;

    replace(&root, &dir, files)
}

/// Refuses what a Vault sends that is not a Layout this side can keep.
///
/// It runs before anything is written, so a name this refuses costs nothing: validation inside the
/// write loop was what lost the old copy when the last name the Vault sent was not one.
///
/// A Layout is at least the sixteen logs `Layout::open` makes, because the Vault opens its own
/// Layout before reading it: nothing at all is not a Layout, and keeping it would replace a good
/// copy with an empty directory.
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

/// Writes every file into a directory of its own, then moves it into the copy's place.
///
/// The order is what keeps a failed fetch from costing the copy that is here:
///
/// 1. what an interrupted fetch left beside the copy is cleared, and a copy an interrupted swap
///    moved aside is put back ([`recover`]);
/// 2. every file is written into `<copy>.fetching-<unique>` and flushed;
/// 3. only then is the copy moved aside, the staging directory moved in, and the old copy dropped.
///
/// The property that matters is that the copy is never removed before a complete new one exists: a
/// write that fails, or a process that is killed, leaves either the old copy or a leftover staging
/// directory — and the next fetch clears the latter.
///
/// # Errors
///
/// Returns [`ActionError::Io`] if a file cannot be written or a directory renamed.
fn replace(root: &Path, dir: &Path, files: &[(String, Vec<u8>)]) -> Result<(), ActionError> {
    recover(root, dir)?;

    let staging = root.join(format!("{VAULT_LAYOUT_NAME}.{FETCHING}{}", unique()));
    fs::create_dir_all(&staging)?;

    for (file, bytes) in files {
        if let Err(error) = write_whole(&staging.join(file), bytes) {
            // The copy that is here was never touched, and what was written of the new one is no use to anyone.
            let _ = fs::remove_dir_all(&staging);

            return Err(error.into());
        }
    }

    let backup = root.join(format!("{VAULT_LAYOUT_NAME}.{KEEPING}{}", unique()));
    let kept = dir.is_dir();

    if kept {
        fs::rename(dir, &backup)?;
    }

    if let Err(error) = fs::rename(&staging, dir) {
        // A swap that could not be finished costs nothing: the copy is put back where it was.
        if kept {
            let _ = fs::rename(&backup, dir);
        }

        let _ = fs::remove_dir_all(&staging);

        return Err(error.into());
    }

    if kept {
        // A copy that will not be dropped is not a failed fetch: the new one is in place, and what is left is cleared by
        // the next fetch ([`recover`]).
        let _ = fs::remove_dir_all(&backup);
    }

    Ok(())
}

/// Clears what an interrupted fetch left beside the copy, and puts back a copy it moved aside.
///
/// A staging directory is what a fetch that did not finish writing leaves; a backup is a copy that
/// an interrupted swap moved out of the way, and one of those is put back only when the copy itself
/// is not there — a copy still in place is the newer of the two.
///
/// Only one fetch of one copy is expected at a time: the desktop serialises its own, and two runs
/// racing over one cache are two runs writing over each other whatever this does.
///
/// # Errors
///
/// Returns [`ActionError::Io`] if the directory cannot be listed or the copy put back.
fn recover(root: &Path, dir: &Path) -> Result<(), ActionError> {
    let mut aside: Option<std::path::PathBuf> = None;

    for entry in fs::read_dir(root)? {
        let entry = entry?;

        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(rest) = name
            .strip_prefix(VAULT_LAYOUT_NAME)
            .and_then(|rest| rest.strip_prefix('.'))
        else {
            continue;
        };

        if rest.starts_with(FETCHING) {
            let _ = fs::remove_dir_all(entry.path());
        } else if rest.starts_with(KEEPING) {
            if dir.is_dir() || aside.is_some() {
                let _ = fs::remove_dir_all(entry.path());
            } else {
                aside = Some(entry.path());
            }
        }
    }

    if let Some(aside) = aside {
        fs::rename(aside, dir)?;
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

/// A value no other fetch of this copy is using, for a directory of its own beside it.
fn unique() -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

/// Whether `name` is one path component and nothing else.
///
/// What a Vault sends is written under the cache by the name it was sent under, so a name that
/// names somewhere else — a path, a climbing name, a drive — would write outside the one directory
/// the fetch is for. A name this refuses is a Layout directory that is not one.
fn is_plain_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// What an encodable payload that will not cross means to an action.
fn codec_failed(error: impl std::fmt::Display) -> ActionError {
    ActionError::Codec(rorolala_errors::BincodeError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_utils_constants::VAULT_LAYOUT_NAME;

    use super::{FETCHING, KEEPING, check, recover, replace};

    /// A directory of its own under the workspace's own scratch space, emptied first so a rerun starts clean.
    ///
    /// It is under the project rather than the system's temporary directory so that a run on a machine whose temporary
    /// directory is sandboxed still writes where it says it does.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-fetch-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The copy's own directory under a scratch root, and that root.
    fn copy_at(root: &Path) -> PathBuf {
        root.join(VAULT_LAYOUT_NAME)
    }

    /// A file's bytes, or nothing when it is not there.
    fn read(path: &Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    /// The names in a directory, in name order.
    fn names(dir: &Path) -> Vec<String> {
        let mut found: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        found
    }

    /// A Layout's worth of files, as the Vault would send them.
    fn sent(files: &[(&str, &str)]) -> Vec<(String, Vec<u8>)> {
        files
            .iter()
            .map(|(name, bytes)| ((*name).to_owned(), bytes.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn a_copy_is_replaced_whole() {
        let root = scratch("whole");
        let dir = copy_at(&root);

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("log-0"), b"old").unwrap();
        std::fs::write(dir.join("an-old-file"), b"gone").unwrap();

        replace(&root, &dir, &sent(&[("log-0", "new"), ("log-1", "new")])).unwrap();

        // What the Vault no longer keeps does not survive the copy, and neither does anything the fetch staged.
        assert_eq!(names(&dir), ["log-0", "log-1"]);
        assert_eq!(read(&dir.join("log-0")).unwrap(), b"new");
        assert_eq!(
            names(&root),
            [VAULT_LAYOUT_NAME.to_owned()],
            "the fetch left something beside the copy"
        );
    }

    #[test]
    fn a_write_that_fails_leaves_the_copy_that_is_here() {
        let root = scratch("failed");
        let dir = copy_at(&root);

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("log-0"), b"old").unwrap();

        // A name a Layout may be keyed by is not a name a file may be made with: the write fails after the copy would
        // have been removed by the old order, which is the bug this is written for.
        let refused = replace(&root, &dir, &sent(&[("log-0", "new"), ("a\u{0}b", "new")]));

        assert!(refused.is_err());
        assert_eq!(
            names(&dir),
            ["log-0"],
            "the copy was written into or emptied"
        );
        assert_eq!(read(&dir.join("log-0")).unwrap(), b"old");
        assert_eq!(
            names(&root),
            [VAULT_LAYOUT_NAME.to_owned()],
            "the fetch left its staging directory behind"
        );
    }

    #[test]
    fn nothing_sent_at_all_is_refused_and_the_copy_is_untouched() {
        let root = scratch("empty");
        let dir = copy_at(&root);

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("log-0"), b"old").unwrap();

        assert!(check(&[]).is_err());

        // The copy is only touched by `replace`, which `write_cache` reaches after the check: what is here is kept.
        assert_eq!(read(&dir.join("log-0")).unwrap(), b"old");
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

    #[test]
    fn a_copy_an_interrupted_swap_moved_aside_is_put_back() {
        let root = scratch("recovered");
        let dir = copy_at(&root);
        let aside = root.join(format!("{VAULT_LAYOUT_NAME}.{KEEPING}1-0"));

        std::fs::create_dir_all(&aside).unwrap();
        std::fs::write(aside.join("log-0"), b"kept").unwrap();

        recover(&root, &dir).unwrap();

        assert_eq!(names(&dir), ["log-0"]);
        assert_eq!(read(&dir.join("log-0")).unwrap(), b"kept");
        assert_eq!(names(&root), [VAULT_LAYOUT_NAME.to_owned()]);
    }

    #[test]
    fn what_a_cut_short_fetch_staged_is_cleared_and_the_copy_kept() {
        let root = scratch("cleared");
        let dir = copy_at(&root);
        let staging = root.join(format!("{VAULT_LAYOUT_NAME}.{FETCHING}1-0"));

        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("log-0"), b"half").unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("log-0"), b"here").unwrap();

        recover(&root, &dir).unwrap();

        assert_eq!(names(&root), [VAULT_LAYOUT_NAME.to_owned()]);
        assert_eq!(read(&dir.join("log-0")).unwrap(), b"here");
    }
}

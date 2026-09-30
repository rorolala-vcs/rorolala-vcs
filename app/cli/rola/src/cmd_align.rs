//! The `rola align` command: bringing a tree and the Layout it works from back into agreement.
//!
//! A reading of the tree ([`rola layout tree-diff`](crate::layout::cmd_layout_tree_diff)) says how
//! the two stand apart, and this is what acts on that: a path the Layout names and the tree no
//! longer holds is confirmed gone, and a path that moved is confirmed moved. Between the two, a
//! move the reading only guessed at can be asserted, and one it guessed wrong can be taken back.
//!
//! `rola track` refuses to run while the Layout names a path the tree does not hold, since recording
//! works on a tree that is settled; this is what settles it. A move, on the other hand, `track`
//! confirms on its own — naming a path that moved records it where it is now.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates, so a
// lint allowed on the handler below is reported as defined twice. The allow lives here, where it
// covers the one signature that needs it.
//
// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_lines, clippy::too_many_arguments)]

use std::fs;
use std::path::{Component, Path, PathBuf};

use librorolala::layout::{Layout, LayoutPath};
use librorolala::storage::{Key, StorageBackend as _};
use librorolala::tree_analyze::{Cache, PathRename, TreeDiff, cache_path, entry_of, tree_diff};
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResCurrentRemoteVault, ResOffline, ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate as _;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::{EC_ERR_ALIGN, EC_ERR_ALIGN_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::fetch::{self, Sources};
use crate::keys::account_named;
use crate::layout::{ErrorLayoutShouldInWorkspace, chosen, failed};

/// Where content a restore writes comes from: the store it is read out of, the index that names it,
/// and the Vaults it may be asked of when the store does not hold it.
///
/// The three travel together because a restore never wants one without the others, and passing them
/// as one value is what keeps the signatures below about the paths they act on.
#[derive(Clone, Copy)]
struct Content<'a, 'b> {
    /// The store the content is read out of.
    storage: &'a ResRorolalaStorage,
    /// The index that says which version names which object.
    index: &'a ResVCSIndex,
    /// Where an object the store does not hold may be brought from, when the run has anywhere to
    /// ask at all.
    sources: Option<&'a Sources<'b>>,
}

/// How alike two text files have to be to count as the same file moved, when nothing is said.
///
/// It is the same default `rola layout tree-diff` reads the tree with, since a move this acts on is
/// one that reading found.
const DEFAULT_ALIKE: f32 = 0.6;

/// The flags `rola align` takes: exactly one of what may be done with one path.
#[derive(Pickable)]
struct AlignFlags {
    /// Confirm that a path the Layout names but the tree does not hold is gone.
    #[arg(long)]
    delete: Flag,
    /// Confirm that a path that moved has moved.
    #[arg(long)]
    rename: Flag,
    /// Take back a move the reading guessed at, leaving a loss and a gain.
    #[arg(long = "break")]
    break_: Flag,
    /// Assert that the named path moved to `FILE`, so the next reading sees it as a move.
    #[arg(long = "move")]
    move_to: Option<String>,
    /// Put a moved file back where the Layout names it.
    #[arg(long = "restore-move")]
    restore_move: Flag,
    /// Put a changed file back to the version the Layout names.
    #[arg(long = "restore-modify")]
    restore_modify: Flag,
    /// Put a deleted file back, taking the version the Layout names out of the store.
    #[arg(long = "restore-delete")]
    restore_delete: Flag,
    /// Put a path back to what the Layout names, whichever of the three it needs.
    #[arg(long = "restore")]
    restore: Flag,
    /// Put a move's file back where the Layout names it and as the Layout names it, undoing the
    /// content as well as the place.
    #[arg(long)]
    completely: Flag,
}

/// What is to be done with the named path.
enum Mode {
    /// It is confirmed gone.
    Delete,
    /// The move it is part of is confirmed.
    Rename,
    /// The move it is part of is taken back.
    Break,
    /// It is asserted to have moved to the named path.
    Move(String),
    /// A moved file is put back where the Layout names it.
    RestoreMove,
    /// A changed file is put back to the version the Layout names.
    RestoreModify,
    /// A deleted file is put back, taken out of the store.
    RestoreDelete,
    /// Whichever of the three the path needs is done.
    Restore,
}

#[help(buffer)]
pub fn help_align(_: EntryAlign, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("align.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryAlign)]
pub fn desc_align() -> Description {
    t!("align.description").to_string().into()
}

/// Brings the tree and the Layout back into agreement about a path
///
/// `PATH` is read where the run was made, as a shell would read it: `.` is the directory the run is
/// in and everything under it, and a name that lands on a directory selects everything under it.
/// A single path need not be there — a path the Layout names and the tree does not is exactly what
/// `--delete` acts on.
///
/// Exactly one of the modes is given. `--delete` confirms that a lost path is gone. `--rename`
/// confirms a move the reading found, whether `PATH` is where it was or where it went. `--break`
/// takes a move back, so the two paths become a loss and a gain again; a move of two identical files
/// is a fact and is refused. `--move FILE` asserts that `PATH` moved to `FILE`, which is for a move
/// the reading could not guess at; the next reading then sees it as one to confirm.
///
/// `--restore-move` puts a moved file back where the Layout names it, `--restore-modify` writes the
/// version the Layout names back over a changed file, and `--restore-delete` takes a deleted file
/// back out of the store and puts it where the Layout names it; `--restore` does whichever of the
/// three the path needs. A move that was edited as well is put back first, so it is left changed at
/// its old path for `--restore-modify` to settle.
///
/// A name that lands on a directory — `.` always does — and `--restore` are what act on many paths
/// at once: every path under it is put back by what it needs, one line each, and nothing under it
/// is a failure. A move is settled first, so content is only put back once the file is where the
/// Layout names it; a path the tree holds and no Layout names is left where it lies. Other modes
/// act on one path only. `--completely` says to put a move back as well as where it was, and means
/// something only for a move being put back.
///
/// # Errors
///
/// Renders [`ErrorAlignArgument`] when no path or not exactly one mode was given,
/// [`ErrorAlignCompletely`] when `--completely` was given for something it cannot mean,
/// [`ErrorAlignNotLost`] or [`ErrorAlignNoRename`] when the path is not what the mode acts on,
/// [`ErrorAlignNotModified`] or [`ErrorAlignNotRestorable`] when there is nothing to put back,
/// [`ErrorAlignRestoreExists`] when putting a file back would land on one already there,
/// [`ErrorAlignRealMove`] when `--break` names a move of identical files, [`ErrorAlignMove`] when
/// `--move` names somewhere that is not a fresh file, and [`ErrorAlignFailed`] otherwise.
#[command(node = "align", entry = EntryAlign)]
pub fn align(args: EntryAlign) -> Next {
    let picked = args
        .pick(&arg![AlignFlags])
        .pick(&arg![Option<String>])
        .to_result();
    let (flags, path) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    let Some(path) = path else {
        return ErrorAlignArgument.into();
    };

    let completely = matches!(flags.completely, Flag::Active);

    let mut modes = Vec::new();
    if matches!(flags.delete, Flag::Active) {
        modes.push(Mode::Delete);
    }
    if matches!(flags.rename, Flag::Active) {
        modes.push(Mode::Rename);
    }
    if matches!(flags.break_, Flag::Active) {
        modes.push(Mode::Break);
    }
    if let Some(target) = flags.move_to {
        modes.push(Mode::Move(target));
    }
    if matches!(flags.restore_move, Flag::Active) {
        modes.push(Mode::RestoreMove);
    }
    if matches!(flags.restore_modify, Flag::Active) {
        modes.push(Mode::RestoreModify);
    }
    if matches!(flags.restore_delete, Flag::Active) {
        modes.push(Mode::RestoreDelete);
    }
    if matches!(flags.restore, Flag::Active) {
        modes.push(Mode::Restore);
    }

    if modes.len() != 1 {
        return ErrorAlignArgument.into();
    }

    // UNWRAP: the list was just checked to hold exactly one mode.
    let mode = modes.pop().unwrap();

    // A move is the one thing with both a place and content of its own to put back, so it is the
    // one thing `--completely` can mean.
    if completely && !matches!(mode, Mode::Restore | Mode::RestoreMove) {
        return ErrorAlignCompletely.into();
    }

    StateAlign {
        path,
        mode,
        completely,
    }
    .into()
}

/// The state an alignment starts in.
#[derive(Grouped)]
pub struct StateAlign {
    /// The path to act on, as it was written.
    path: String,
    /// What is to be done with it.
    mode: Mode,
    /// Whether a move is to be put back as well as returned to where it was.
    completely: bool,
}

#[chain]
pub fn handle_align(
    state: StateAlign,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: &ResOffline,
) -> Next {
    let StateAlign {
        path,
        mode,
        completely,
    } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    // Read after the Layout, since asking the Workspace for its own Layout is a borrow of it: what
    // is held from here on is the Workspace itself, and the resource is not asked again.
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };
    let root = normalize(held.get_root());

    // Content a restore writes comes out of the store, and the store may not hold it: what is
    // missing is asked of the Vaults the Workspace has bound. A run that cannot say which account it
    // acts as has none to ask, and then restores only what is here.
    let account = current
        .get_ref()
        .must_bind()
        .ok()
        .and_then(|name| account_named(&name, Some(held), None).ok());
    let sources = match &account {
        Some(account) => {
            let tracked = crate::ownership::tracked_vault(held, &layout);
            let primary = fetch::primary(remote.get_ref(), tracked.as_deref());
            let sources = match Sources::new(held, account, remote.get_ref(), primary, **offline) {
                Ok(sources) => sources,
                Err(cause) => return align_failed(cause),
            };

            Some(sources)
        }
        None => None,
    };
    let content = Content {
        storage: storage.get_ref(),
        index: index.get_ref(),
        sources: sources.as_ref(),
    };

    let selected = match select(&root, &path) {
        Ok(selected) => selected,
        Err(next) => return next,
    };

    let diff = match tree_diff(&layout, held, DEFAULT_ALIKE) {
        Ok(diff) => diff,
        Err(error) => {
            return ErrorAlignFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let next = match (mode, selected) {
        (Mode::Delete, Selector::One(path)) => delete(&layout, &diff, &path),
        (Mode::Rename, Selector::One(path)) => rename(&layout, &diff, &path, &root),
        (Mode::Break, Selector::One(path)) => take_back(&layout, &diff, &path, &root),
        (Mode::Move(target), Selector::One(path)) => {
            assert_move(&layout, &diff, &path, &root, &target)
        }
        (Mode::RestoreMove, Selector::One(path)) => single(restore_move(
            &layout, &diff, &path, &root, content, completely,
        )),
        (Mode::RestoreModify, Selector::One(path)) => {
            single(restore_modify(&layout, &diff, &path, &root, content))
        }
        (Mode::RestoreDelete, Selector::One(path)) => {
            single(restore_delete(&layout, &diff, &path, &root, content))
        }
        (Mode::Restore, Selector::One(path)) => single(restore_whichever(
            &layout, &diff, &path, &root, content, completely,
        )),
        (Mode::Restore, Selector::Under(prefix)) => {
            restore_all(&layout, &diff, &prefix, &root, content, completely)
        }
        // A directory or `.` names many files, and only putting them back acts on many at once.
        (_, Selector::Under(_)) => ErrorAlignArgument.into(),
    };

    if let Some(sources) = &sources {
        sources.report();
    }

    next
}

/// What a mode acts on: the one path named, or every path under a directory the run was made in.
enum Selector {
    /// One path, named by what was written.
    One(LayoutPath),
    /// Every path under the directory whose prefix this is, `""` naming the whole tree.
    Under(String),
}

/// Reads what was written into what the run is to act on.
///
/// What is written is a path where the run was made, as a shell would read it: `.` is the directory
/// the run is in and everything under it, and `..` climbs from there. A name that lands on a
/// directory — `.` always does — selects everything under it; anything else is the one path named.
///
/// # Errors
///
/// Answers with [`ErrorAlignArgument`] when the path reaches out of the Workspace, or names nothing
/// a Layout could.
fn select(root: &Path, given: &str) -> Result<Selector, Next> {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return Err(ErrorAlignFailed {
                cause: error.to_string(),
            }
            .into());
        }
    };

    let absolute = normalize(&resolve(&cwd, given));

    let Ok(relative) = absolute.strip_prefix(root) else {
        return Err(ErrorAlignArgument.into());
    };

    if absolute.is_dir() {
        return Ok(Selector::Under(prefix(relative)));
    }

    LayoutPath::from_relative(relative)
        .map(Selector::One)
        .map_err(|_| ErrorAlignArgument.into())
}

/// Whether `path` is `prefix`'s directory or under it, with `""` naming the whole tree.
fn under(prefix: &str, path: &LayoutPath) -> bool {
    if prefix.is_empty() {
        return true;
    }

    path.as_str()
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// The prefix the paths under `relative` share, or `""` for the Workspace root itself, which is the
/// one directory with no name of its own.
fn prefix(relative: &Path) -> String {
    LayoutPath::from_relative(relative)
        .map_or_else(|_| String::new(), |path| path.as_str().to_owned())
}

/// The path `given` names, made absolute against the directory the run was made in.
///
/// It mirrors `rola track`'s reading of a name, since a name is written the same way wherever it is
/// written, and the two commands act on the same paths.
fn resolve(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// `path` with its `.` dropped and its `..` climbed, worked out lexically rather than on disk.
///
/// Working it out here, before the disk is asked anything, is what lets `.` name the directory the
/// run was made in rather than a path with no components in it.
fn normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if matches!(components.last(), Some(Component::Normal(_))) => {
                components.pop();
            }
            other => components.push(other),
        }
    }

    components.into_iter().collect()
}

/// Answers one path put back as the only item of a result.
fn single(outcome: Result<(AlignDid, String), Next>) -> Next {
    match outcome {
        Ok((did, what)) => ResultAlign::one(did, what).into(),
        Err(next) => next,
    }
}

/// Confirms that a path the Layout names and the tree does not hold is gone.
fn delete(layout: &Layout, diff: &TreeDiff, path: &LayoutPath) -> Next {
    if !diff.lost.contains(path) {
        return ErrorAlignNotLost {
            path: path.as_str().to_owned(),
        }
        .into();
    }

    let Some(id) = layout.id_of(path) else {
        return ErrorAlignNotLost {
            path: path.as_str().to_owned(),
        }
        .into();
    };

    if let Err(error) = layout.remove_entry(id) {
        return failed(&error);
    }

    ResultAlign::one(AlignDid::Deleted, path.as_str().to_owned()).into()
}

/// Confirms a move the reading found.
///
/// The path the file is now at is one the Layout names, and what the Layout names there is the
/// version the file held before it moved. A move that was edited is therefore left changed — and
/// what the reading wrote down says the two agree, so the disagreement is written down here rather
/// than left for a reading that would never find it.
fn rename(layout: &Layout, diff: &TreeDiff, path: &LayoutPath, root: &Path) -> Next {
    let Some(pair) = rename_of(diff, path) else {
        return ErrorAlignNoRename {
            path: path.as_str().to_owned(),
        }
        .into();
    };

    if let Err(error) = layout.move_path(&pair.from, &pair.to) {
        return failed(&error);
    }

    if let Err(next) = remember_rename(layout, root, &pair.to, diff.modified.contains(&pair.to)) {
        return next;
    }

    ResultAlign::one(
        AlignDid::Moved,
        format!("{} -> {}", pair.from.as_str(), pair.to.as_str()),
    )
    .into()
}

/// Writes down where a confirmed move left the file: what the Layout now names, and whether the
/// tree still holds it.
fn remember_rename(
    layout: &Layout,
    root: &Path,
    to: &LayoutPath,
    modified: bool,
) -> Result<(), Next> {
    let disk = root.join(to.to_path_buf());
    if !disk.is_file() {
        return Ok(());
    }

    let path = cache_path(root, layout);
    let mut cache = Cache::read(&path);

    let entry = match cache.get(to).cloned() {
        Some(entry) => entry,
        None => entry_of(&disk).map_err(|error| align_failed(error.to_string()))?,
    };

    cache.insert(to.clone(), if modified { entry.disagreed() } else { entry });
    cache
        .write(&path)
        .map_err(|error| align_failed(error.to_string()))
}

/// Takes back a move the reading guessed at, leaving the two paths a loss and a gain.
///
/// The reading pairs two text files by how alike they are, and keeps that likeness in what it
/// wrote down about the lost path; forgetting it leaves the digest — which is what the path is
/// remembered by — and stops the two being paired. A move of two files that hold the same bytes is
/// one the reading found by digest, not by likeness, and is a fact rather than a guess.
fn take_back(layout: &Layout, diff: &TreeDiff, path: &LayoutPath, root: &Path) -> Next {
    let Some(pair) = rename_of(diff, path) else {
        return ErrorAlignNoRename {
            path: path.as_str().to_owned(),
        }
        .into();
    };

    // A strong move is one the two being the same bytes makes: taking it back would be denying what
    // is there, so it is refused. Only a move the likeness only suggests is a guess to be undone.
    if pair.strong {
        return ErrorAlignRealMove {
            from: pair.from.as_str().to_owned(),
            to: pair.to.as_str().to_owned(),
        }
        .into();
    }

    let cache_file = cache_path(root, layout);
    let mut cache = Cache::read(&cache_file);

    let Some(was) = cache.get(&pair.from).cloned() else {
        return ErrorAlignNoRename {
            path: path.as_str().to_owned(),
        }
        .into();
    };

    cache.insert(pair.from.clone(), was.without_spans());
    if let Err(error) = cache.write(&cache_file) {
        return ErrorAlignFailed {
            cause: error.to_string(),
        }
        .into();
    }

    ResultAlign::one(
        AlignDid::Broke,
        format!("{} / {}", pair.from.as_str(), pair.to.as_str()),
    )
    .into()
}

/// Asserts that a lost path moved to `target`, so the next reading pairs them.
///
/// The reading knows a path moved by what it wrote down about the path that is gone; what is
/// written down here is what `target` holds now, so the two are the same to the next reading. What
/// changes is the memory, not the Layout: `--rename` is still what moves the path.
fn assert_move(
    layout: &Layout,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    target: &str,
) -> Next {
    if !diff.lost.contains(path) {
        return ErrorAlignNotLost {
            path: path.as_str().to_owned(),
        }
        .into();
    }

    let Ok(target) = LayoutPath::new(target) else {
        return ErrorAlignMove {
            path: target.to_owned(),
            kind: MoveError::Missing,
        }
        .into();
    };

    if layout.id_of(&target).is_some() {
        return ErrorAlignMove {
            path: target.as_str().to_owned(),
            kind: MoveError::Tracked,
        }
        .into();
    }

    let disk = root.join(target.to_path_buf());
    if !disk.is_file() {
        return ErrorAlignMove {
            path: target.as_str().to_owned(),
            kind: MoveError::Missing,
        }
        .into();
    }

    let entry = match entry_of(&disk) {
        Ok(entry) => entry,
        Err(error) => {
            return ErrorAlignFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let cache_file = cache_path(root, layout);
    let mut cache = Cache::read(&cache_file);
    cache.insert(path.clone(), entry);
    if let Err(error) = cache.write(&cache_file) {
        return ErrorAlignFailed {
            cause: error.to_string(),
        }
        .into();
    }

    ResultAlign::one(
        AlignDid::Asserted,
        format!("{} -> {}", path.as_str(), target.as_str()),
    )
    .into()
}

/// Puts a moved file back where the Layout names it.
///
/// The Layout still names the path the file was at, since the move was never confirmed, so putting
/// the file back there undoes the move on disk. What the file holds is not touched — one that was
/// edited as well as moved is left changed at its old path, which is then `--restore-modify`'s to
/// settle — unless `completely`, which puts the version the Layout names back over it too, so the
/// move is undone in content as well as in place.
fn restore_move(
    layout: &Layout,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: Content<'_, '_>,
    completely: bool,
) -> Result<(AlignDid, String), Next> {
    let Some(pair) = rename_of(diff, path) else {
        return Err(ErrorAlignNoRename {
            path: path.as_str().to_owned(),
        }
        .into());
    };

    let from_disk = root.join(pair.from.to_path_buf());
    let to_disk = root.join(pair.to.to_path_buf());

    // What the Layout names is where the file goes; a file already there is one the restore would
    // have to take away, and a restore does not take anything away.
    if from_disk.exists() {
        return Err(ErrorAlignRestoreExists {
            path: pair.from.as_str().to_owned(),
        }
        .into());
    }

    if let Some(parent) = from_disk.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return Err(ErrorAlignFailed {
            cause: error.to_string(),
        }
        .into());
    }

    if let Err(error) = fs::rename(&to_disk, &from_disk) {
        return Err(ErrorAlignFailed {
            cause: error.to_string(),
        }
        .into());
    }

    let what = format!("{} <- {}", pair.from.as_str(), pair.to.as_str());

    if completely {
        put_version(layout, &pair.from, root, content)?;
        return Ok((AlignDid::RestoredCompletely, what));
    }

    Ok((AlignDid::RestoredMove, what))
}

/// Puts a changed file back to the version the Layout names, writing over what it holds.
fn restore_modify(
    layout: &Layout,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: Content<'_, '_>,
) -> Result<(AlignDid, String), Next> {
    // A path the Layout does not name has no version to go back to: a move's destination is one of
    // these, which is why a move is put back first and only then has its content settled.
    if !diff.modified.contains(path) || layout.id_of(path).is_none() {
        return Err(ErrorAlignNotModified {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    put_version(layout, path, root, content)?;
    Ok((AlignDid::RestoredModify, path.as_str().to_owned()))
}

/// Puts a deleted file back, taking the version the Layout names out of the store.
fn restore_delete(
    layout: &Layout,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: Content<'_, '_>,
) -> Result<(AlignDid, String), Next> {
    if !diff.lost.contains(path) || layout.id_of(path).is_none() {
        return Err(ErrorAlignNotLost {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    // A lost path is one the tree does not hold, so putting it back has nothing to land on: a file
    // already there is something to be told about rather than written over.
    if root.join(path.to_path_buf()).exists() {
        return Err(ErrorAlignRestoreExists {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    put_version(layout, path, root, content)?;
    Ok((AlignDid::RestoredDelete, path.as_str().to_owned()))
}

/// Writes the version stored under `version` out to `disk`, making the way to it.
///
/// It is what putting a file back is made of, whether the file is one that changed or one that is
/// gone: the Layout names a version, the store holds the content it was made of, and the two are put
/// back together at the path. Content the store does not hold is brought from a Vault first, when
/// the run has one to ask.
fn write_recorded(content: Content<'_, '_>, version: [u8; 32], disk: &Path) -> Result<(), Next> {
    let Some(store) = content.storage.as_ref() else {
        return Err(align_failed(t!("align.err_no_store").trim()));
    };
    let Some(vcs) = content.index.as_ref() else {
        return Err(align_failed(t!("align.err_no_index").trim()));
    };

    let runtime =
        tokio::runtime::Runtime::new().map_err(|error| align_failed(error.to_string()))?;
    let recorded = recorded_hash(vcs, &runtime, version).map_err(align_failed)?;

    if let Some(parent) = disk.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return Err(align_failed(error.to_string()));
    }

    if let Some(sources) = content.sources {
        sources.bring(store, &[Key::new(recorded)]);
    }

    runtime
        .block_on(store.extract_file(&Key::new(recorded), disk))
        .map_err(|error| align_failed(error.to_string()))
}

/// Writes down that a path put back holds what the Layout names again.
///
/// Putting a file back is what makes the two agree, so what the reading remembers of the path has to
/// say so: a path left marked as disagreed — by a move that was edited and confirmed — would go on
/// being reported as changed after its content was put back.
fn remember_restored(layout: &Layout, root: &Path, path: &LayoutPath) -> Result<(), Next> {
    let disk = root.join(path.to_path_buf());
    if !disk.is_file() {
        return Ok(());
    }

    let cache_path = cache_path(root, layout);
    let mut cache = Cache::read(&cache_path);
    let entry = entry_of(&disk).map_err(|error| align_failed(error.to_string()))?;

    cache.insert(path.clone(), entry);
    cache
        .write(&cache_path)
        .map_err(|error| align_failed(error.to_string()))
}

/// Writes the version the Layout names for `path` over what is there, and remembers that it does.
///
/// A file's content is put back by naming what the Layout holds for it and letting the store hand
/// that version out at the path — whether the file is one that changed or one that is gone, since
/// both want the same thing written — and remembering it is what stops the path being reported as
/// changed once it agrees again.
fn put_version(
    layout: &Layout,
    path: &LayoutPath,
    root: &Path,
    content: Content<'_, '_>,
) -> Result<(), Next> {
    let not_restorable = || ErrorAlignNotRestorable {
        path: path.as_str().to_owned(),
    };

    let Some(id) = layout.id_of(path) else {
        return Err(not_restorable().into());
    };
    let Some(data) = layout.entry(id) else {
        return Err(not_restorable().into());
    };

    write_recorded(content, data.version(), &root.join(path.to_path_buf()))?;

    remember_restored(layout, root, path)
}

/// The failure an alignment that could not be made is answered with.
fn align_failed(cause: impl Into<String>) -> Next {
    ErrorAlignFailed {
        cause: cause.into(),
    }
    .into()
}

/// Puts the path back to what the Layout names, whichever of the three it needs.
fn restore_whichever(
    layout: &Layout,
    diff: &TreeDiff,
    path: &LayoutPath,
    root: &Path,
    content: Content<'_, '_>,
    completely: bool,
) -> Result<(AlignDid, String), Next> {
    if rename_of(diff, path).is_some() {
        return restore_move(layout, diff, path, root, content, completely);
    }

    if diff.lost.contains(path) && layout.id_of(path).is_some() {
        return restore_delete(layout, diff, path, root, content);
    }

    if diff.modified.contains(path) && layout.id_of(path).is_some() {
        return restore_modify(layout, diff, path, root, content);
    }

    Err(ErrorAlignNotRestorable {
        path: path.as_str().to_owned(),
    }
    .into())
}

/// Puts every path under a directory back to what the Layout names, each by what it needs.
///
/// A move is settled first: its destination is not a path the Layout names, so content can only be
/// put back once the file is where the Layout names it. What is left — a path the tree no longer
/// holds, and one that changed where it is — is put back after. A path the tree holds and no Layout
/// names (`untagged`) is left where it lies, since there is nothing it is to be put back to.
fn restore_all(
    layout: &Layout,
    diff: &TreeDiff,
    prefix: &str,
    root: &Path,
    content: Content<'_, '_>,
    completely: bool,
) -> Next {
    let mut items = Vec::new();

    for pair in &diff.renamed {
        if !(under(prefix, &pair.from) || under(prefix, &pair.to)) {
            continue;
        }

        match restore_move(layout, diff, &pair.from, root, content, completely) {
            Ok((did, what)) => items.push(AlignItem { did, what }),
            Err(next) => return next,
        }
    }

    for path in &diff.lost {
        if !under(prefix, path) || layout.id_of(path).is_none() {
            continue;
        }

        match restore_delete(layout, diff, path, root, content) {
            Ok((did, what)) => items.push(AlignItem { did, what }),
            Err(next) => return next,
        }
    }

    for path in &diff.modified {
        if !under(prefix, path) || layout.id_of(path).is_none() {
            continue;
        }

        match restore_modify(layout, diff, path, root, content) {
            Ok((did, what)) => items.push(AlignItem { did, what }),
            Err(next) => return next,
        }
    }

    ResultAlign { items }.into()
}

/// The stored hash the version `hash` names, as the Layout records it.
fn recorded_hash(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: [u8; 32],
) -> Result<[u8; 32], String> {
    let version = runtime
        .block_on(vcs.read(Key::new(hash)))
        .map_err(|error| error.reason())?
        .expect_version()
        .map_err(|error| error.to_string())?;

    let variant = runtime
        .block_on(vcs.read(Key::new(*version.variant())))
        .map_err(|error| error.reason())?
        .expect_variant()
        .map_err(|error| error.to_string())?;

    Ok(*variant.storage_hash())
}

/// The move `path` is a side of, when the reading found one.
fn rename_of<'a>(diff: &'a TreeDiff, path: &LayoutPath) -> Option<&'a PathRename> {
    diff.renamed
        .iter()
        .find(|pair| &pair.from == path || &pair.to == path)
}

/// What was done with the path.
#[derive(Clone, Copy)]
enum AlignDid {
    /// A lost path was confirmed gone.
    Deleted,
    /// A move was confirmed.
    Moved,
    /// A move was taken back.
    Broke,
    /// A move was asserted for the next reading.
    Asserted,
    /// A moved file was put back where the Layout names it.
    RestoredMove,
    /// A moved file was put back where and as the Layout names it.
    RestoredCompletely,
    /// A changed file was put back to the version the Layout names.
    RestoredModify,
    /// A deleted file was put back, taken out of the store.
    RestoredDelete,
}

/// One path, as `align` reports it.
struct AlignItem {
    /// What was done with it.
    did: AlignDid,
    /// What it was done to.
    what: String,
}

/// Result: the tree and the Layout were brought back into agreement about paths.
#[derive(Grouped)]
pub struct ResultAlign {
    /// What was done, in the order it was done.
    items: Vec<AlignItem>,
}

impl ResultAlign {
    /// The result of one path brought back into agreement.
    fn one(did: AlignDid, what: String) -> Self {
        Self {
            items: vec![AlignItem { did, what }],
        }
    }
}

#[renderer(buffer)]
pub fn render_result_align(result: ResultAlign) {
    for item in &result.items {
        let said = match item.did {
            AlignDid::Deleted => t!("align.result_deleted", what = item.what),
            AlignDid::Moved => t!("align.result_moved", what = item.what),
            AlignDid::Broke => t!("align.result_broke", what = item.what),
            AlignDid::Asserted => t!("align.result_asserted", what = item.what),
            AlignDid::RestoredMove => t!("align.result_restored_move", what = item.what),
            AlignDid::RestoredCompletely => {
                t!("align.result_restored_completely", what = item.what)
            }
            AlignDid::RestoredModify => t!("align.result_restored_modify", what = item.what),
            AlignDid::RestoredDelete => t!("align.result_restored_delete", what = item.what),
        };

        r_println!("{}", said.trim());
    }
}

/// Error: `rola align` was given no path, or not exactly one mode.
#[derive(Grouped)]
pub struct ErrorAlignArgument;

impl Failure for ErrorAlignArgument {
    fn name(&self) -> &'static str {
        "error_align_argument"
    }

    fn reason(&self) -> String {
        t!("align.err_argument").trim().to_string()
    }
}

failure!(ErrorAlignArgument);

#[renderer(buffer)]
pub fn render_error_align_argument(_: ErrorAlignArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("align.err_argument").trim()));
    r_eprintln!("{}", help_line!(t!("align.err_argument_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: `--completely` was given for something it cannot mean.
#[derive(Grouped)]
pub struct ErrorAlignCompletely;

impl Failure for ErrorAlignCompletely {
    fn name(&self) -> &'static str {
        "error_align_completely"
    }

    fn reason(&self) -> String {
        t!("align.err_completely").trim().to_string()
    }
}

failure!(ErrorAlignCompletely);

#[renderer(buffer)]
pub fn render_error_align_completely(_: ErrorAlignCompletely, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("align.err_completely").trim()));
    r_eprintln!("{}", help_line!(t!("align.err_completely_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path named is not one the Layout names and the tree does not hold.
#[derive(Grouped)]
pub struct ErrorAlignNotLost {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNotLost {
    fn name(&self) -> &'static str {
        "error_align_not_lost"
    }

    fn reason(&self) -> String {
        t!("align.err_not_lost", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNotLost);

#[renderer(buffer)]
pub fn render_error_align_not_lost(error: ErrorAlignNotLost, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_not_lost_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path named is not a side of a move the reading found.
#[derive(Grouped)]
pub struct ErrorAlignNoRename {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNoRename {
    fn name(&self) -> &'static str {
        "error_align_no_rename"
    }

    fn reason(&self) -> String {
        t!("align.err_no_rename", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNoRename);

#[renderer(buffer)]
pub fn render_error_align_no_rename(error: ErrorAlignNoRename, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_no_rename_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path is not a change the Layout names, so there is no version to go back to.
#[derive(Grouped)]
pub struct ErrorAlignNotModified {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNotModified {
    fn name(&self) -> &'static str {
        "error_align_not_modified"
    }

    fn reason(&self) -> String {
        t!("align.err_not_modified", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNotModified);

#[renderer(buffer)]
pub fn render_error_align_not_modified(error: ErrorAlignNotModified, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_not_modified_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: putting a moved file back would land on one already there.
#[derive(Grouped)]
pub struct ErrorAlignRestoreExists {
    /// Where it would land.
    path: String,
}

impl Failure for ErrorAlignRestoreExists {
    fn name(&self) -> &'static str {
        "error_align_restore_exists"
    }

    fn reason(&self) -> String {
        t!("align.err_restore_exists", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignRestoreExists);

#[renderer(buffer)]
pub fn render_error_align_restore_exists(error: ErrorAlignRestoreExists, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_restore_exists_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the path is neither a move nor a change, so there is nothing to put back.
#[derive(Grouped)]
pub struct ErrorAlignNotRestorable {
    /// What was named.
    path: String,
}

impl Failure for ErrorAlignNotRestorable {
    fn name(&self) -> &'static str {
        "error_align_not_restorable"
    }

    fn reason(&self) -> String {
        t!("align.err_not_restorable", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignNotRestorable);

#[renderer(buffer)]
pub fn render_error_align_not_restorable(error: ErrorAlignNotRestorable, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_not_restorable_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: `--break` named a move of two files that hold the same bytes.
#[derive(Grouped)]
pub struct ErrorAlignRealMove {
    /// Where the Layout named it.
    from: String,
    /// Where the tree holds it.
    to: String,
}

impl Failure for ErrorAlignRealMove {
    fn name(&self) -> &'static str {
        "error_align_real_move"
    }

    fn reason(&self) -> String {
        t!("align.err_real_move", from = self.from, to = self.to)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignRealMove);

#[renderer(buffer)]
pub fn render_error_align_real_move(error: ErrorAlignRealMove, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_real_move_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: what `--move` named cannot be moved onto.
#[derive(Grouped)]
pub struct ErrorAlignMove {
    /// What was named.
    path: String,
    /// What is wrong with it.
    kind: MoveError,
}

/// The ways what `--move` named cannot be moved onto.
pub enum MoveError {
    /// It is not a file inside the Workspace.
    Missing,
    /// The Layout already names it.
    Tracked,
}

impl Failure for ErrorAlignMove {
    fn name(&self) -> &'static str {
        "error_align_move"
    }

    fn reason(&self) -> String {
        let said = match self.kind {
            MoveError::Missing => t!("align.err_move_missing", path = self.path),
            MoveError::Tracked => t!("align.err_move_tracked", path = self.path),
        };

        said.trim().to_string()
    }
}

failure!(ErrorAlignMove);

#[renderer(buffer)]
pub fn render_error_align_move(error: ErrorAlignMove, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_move_help").trim()));
    ec.exit_code = EC_ERR_ALIGN_ARGUMENT;
}

/// Error: the alignment could not be made.
#[derive(Grouped)]
pub struct ErrorAlignFailed {
    /// Why it could not.
    cause: String,
}

impl Failure for ErrorAlignFailed {
    fn name(&self) -> &'static str {
        "error_align_failed"
    }

    fn reason(&self) -> String {
        t!("align.err_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorAlignFailed);

#[renderer(buffer)]
pub fn render_error_align_failed(error: ErrorAlignFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("align.err_failed_help").trim()));
    ec.exit_code = EC_ERR_ALIGN;
}

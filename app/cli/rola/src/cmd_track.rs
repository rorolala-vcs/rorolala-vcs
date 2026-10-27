//! The `rola track` command: put a file's content into the store and record it in the Layout.
//!
//! It is the whole of a piece of work rather than a step of it: a file is named, its content goes
//! into the store, a version is made of it, and the Layout being worked in is made to name that
//! version — so what a person says is a file, and what a Layout gains is everything a checkout of
//! it would need. A file already recorded is left alone while its content has not changed, and is
//! given a new version when it has.
//!
//! What is recorded about a change is a message, and a message covers a group: one for the whole run
//! and, when a file wants its own words, one per file. A run is not always able to say them on the
//! command line, so what it does not say is asked for in an editor. See [`crate::editor`].
//!
//! The order is read, ask, then write: every file that is to be recorded is read first, to find which
//! of them changed, and the message is asked for those; only then is the content written and the
//! versions made. So a run whose editor is left empty has written nothing, and the editor lists the
//! files that will actually be recorded rather than a guess at them. What the run is doing while it
//! does it is said through [`crate::progress`], on a bar or as one record per line.

// The handler that records a file is one pass over everything the run is handed: the source, the
// store, the index and the report about them are moved through together, and the borrows `#[chain]`
// hands the handler would be scattered across helpers if that pass were cut up.
#![allow(clippy::too_many_lines)]
// A recording of one file needs everything about it — where it is, what it holds, the version it is
// at, the merge waiting for it and the move it confirms — and gathering them into one value would
// only move the count somewhere else.
#![allow(clippy::too_many_arguments)]
// The progress setting is the resource the injection hands the handler and not something a recording
// owns: taking a copy would read a state of its own, so it is taken by reference, which is what the
// injection gives it and what every other handler that watches a run does.
#![allow(clippy::trivially_copy_pass_by_ref)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use librorolala::layout::{Batch, Layout, LayoutPath, MutableData};
use librorolala::status::{tracked_vault, unowned};
use librorolala::storage::{
    Chunking, Key, Lockable as _, RorolalaStorage, key_of_file, store_file,
};
use librorolala::tree_analyze::{Cache, PathRename, cache_path, entry_of, tree_diff, walk};
use librorolala::vcs::{Creator, Message, VCSIndex, VCSIndexBatch, Variant, Version};
use librorolala::workspace::{DATA_DIR, Merging, Pending};
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResForce, ResProgressSetting, ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::WORKSPACE_EDITING_PATH;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{filling_flag, is_flag, offer, strip_written, typing_flag};
use crate::editor::{ResEditor, open};
use crate::exit_codes::{
    EC_ABORT, EC_ERR_TRACK, EC_ERR_TRACK_ARGUMENT, EC_ERR_TRACK_OWNERSHIP, EC_HELP,
};
use crate::failure::failure;
use crate::layout::{ErrorLayoutShouldInWorkspace, chosen, failed};
use crate::progress::Reporting;

/// The longest a message may be, in bytes.
///
/// It is the limit a [`Message`] object carries, kept here so that what a run is told before
/// anything is written is the same limit the object enforces.
const LONGEST_MESSAGE: usize = 256;

/// How alike two text files have to be to count as the same file moved.
///
/// It is the same default the reading uses, since what `track` acts on is what the reading found.
const DEFAULT_ALIKE: f32 = 0.6;

/// How many times the editor is opened again while what it holds is still too long.
///
/// The editor is the caller's, and there is no telling what one will do with a message that was
/// refused; a few attempts tell apart a caller fixing it from one that cannot, which is a run that
/// has to be stopped rather than reopened forever.
const EDIT_ATTEMPTS: usize = 10;

/// How long a recording goes unwatched before its bars are let out.
///
/// Shorter than the ordinary wait — see [`crate::progress`] — because a recording opens with the part a
/// reader most wants to watch: the whole of every file it is to record is read before anything is
/// written, and a bar that appeared only once that reading was over would be a bar for nothing.
const WATCH_AFTER: Duration = Duration::from_secs(1);

/// How many loose objects a recording leaves before it packs the index.
///
/// What a pack costs is a rewrite of every object the index holds, so packing after every run is a run
/// whose cost grows with the index for a saving that does not. A few hundred loose objects is when one
/// pack is worth the rewrite; until then they are valid loose objects a later run settles.
const LOOSE_BEFORE_PACK: usize = 256;

/// How many packs the index may hold before a recording gathers them into one.
///
/// A run writes what it makes as one pack, so a run that never gathered would leave a pack per run
/// behind — and every lookup walks every pack. A handful is what a run of them is worth; past that the
/// walk is the reading rather than the lookup, and one rewrite is cheaper than it.
const PACKS_BEFORE_REPACK: usize = 8;

/// What the whole group did; the message every file carries unless it is given its own.
const ARG_MESSAGE: PickerArg<'static, Option<String>> = arg![message: Option<String>];

/// What one file did, one for each file named, in the order they are named.
const ARG_FILE_MESSAGE: PickerArg<'static, Vec<String>> = arg![file_message: Vec<String>];

/// Record what is already known, and never open an editor.
const ARG_NO_EDITOR: PickerArg<'static, Flag> = arg![no_editor: Flag];

/// Replace the version the Layout names for each file, the way `git commit --amend` replaces one.
const ARG_REPLACE: PickerArg<'static, Flag> = arg![replace: Flag];

/// Cut these files with the named chunking policy, and prefer it for them from now on.
const ARG_POLICY: PickerArg<'static, Option<String>> = arg![policy: Option<String>];

#[help(buffer)]
pub fn help_track(_: EntryTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("track.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTrack)]
pub fn desc_track() -> Description {
    t!("track.description").to_string().into()
}

/// Completes what `rola track` can be given next.
///
/// Every `FILE` names where something sits in the tree, so the filesystem answers it. A message is
/// the caller's own words and `--no-editor` is a flag, so neither has anything to list but the
/// flags themselves.
#[completion(EntryTrack)]
pub fn complete_track(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_MESSAGE: t!("track.complete.message"),
                ARG_FILE_MESSAGE: t!("track.complete.file_message"),
                ARG_NO_EDITOR: t!("track.complete.no_editor"),
                ARG_REPLACE: t!("track.complete.replace"),
                ARG_POLICY: t!("track.complete.policy"),
            },
        );
    }

    if filling_flag(&ctx, &ARG_MESSAGE) || filling_flag(&ctx, &ARG_FILE_MESSAGE) {
        return suggest!();
    }

    // What a policy may be is this build's to say rather than the run's, so the names are offered
    // from the registry: a shell that could only complete file names would leave a caller typing a
    // name nothing answers to.
    if filling_flag(&ctx, &ARG_POLICY) {
        return offer(&ctx, librorolala::storage::names());
    }

    Suggest::file_comp()
}

/// Records one or more files in the Layout being worked in
///
/// Each `FILE` is read first, to find whether what it holds differs from what the Layout names — the
/// files that changed and no others then go on to a message, a version and a Layout entry. A file
/// already recorded and unchanged is skipped, and one that did change is given a new version. A
/// `FILE` that is a directory is taken recursively: every file
/// under it is recorded, read the way the tree reading reads them, so what the Workspace keeps for
/// itself and what another Workspace holds are left out. A name is read against where the run was
/// made, so `.` is the directory it is in — and, at the Workspace root, the whole of the work.
///
/// `--message` says what the whole group did and is what each file carries; `--file-message` gives
/// one file its own words, one for each file named, paired with the files by position. A file whose
/// own message is empty carries the group's alone; one that begins with `!` carries its own alone,
/// without the group's. When neither says enough, an editor is opened.
///
/// `--replace` redoes the version each file is at rather than adding one after it, the way
/// `git commit --amend` redoes the last commit: the new version is built on what the version being
/// replaced was built on, so that version stands as if it had never been made. It is what a run
/// asks for when the version it made was wrong — the wrong content, or the wrong words — and the
/// file's content is what the tree holds now. A file the Layout records nothing for, and a version
/// that is a merge, are refused: neither is a version a new one can take the place of. The old
/// version and its variant stay in the index; only the Layout stops naming them. With `--replace`
/// and no `--message` or `--file-message`, the words the replaced version had are what the editor
/// opens on — and what `--no-editor` records as they stand.
///
/// A file the tree moved is recorded where it is now, and the move is confirmed along with it: what
/// a run asks for is a version. A move whose file was not edited is nothing to record, though, so
/// it is left alone — `rola align` is what confirms it.
///
/// A file the fetched copy of the Vault's Layout says another account holds is refused: the version
/// made here would be one this side cannot push, so recording it makes the conflict rather than
/// avoiding it. `--force` records it anyway, for a run that knows this and means it.
///
/// `--policy` cuts the files this run writes with the named strategy, and records that as what they
/// are to be cut with from then on. A file whose preference names a policy this build does not have
/// is refused rather than written by a default: cutting it another way is not a smaller thing than
/// stopping, it is a write that does not deduplicate against everything already stored for it.
///
/// # Errors
///
/// Renders [`ErrorTrackNoFiles`] when no file is named, [`ErrorTrackFile`] when a path is not a file
/// or is not inside the Workspace, [`ErrorTrackPolicy`] when `--policy` names a strategy this build
/// does not carry, [`ErrorTrackPreference`] when a file's recorded preference names one it does not
/// carry, [`ErrorTrackReplace`] when `--replace` names a file the Layout
/// records no version for or one whose version is a merge, [`ErrorTrackMessage`] when the messages
/// do not line up or are too long, [`ErrorTrackUnowned`] when a file named is one another account
/// holds, [`ErrorNoEditor`](crate::editor::ErrorNoEditor) when an editor is needed and none is
/// named, [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorTrackFailed`] when the store, the index or the Layout refuses.
#[command(node = "track", entry = EntryTrack)]
pub fn track(args: EntryTrack, force: &ResForce) -> Next {
    let picked = args
        .pick(ARG_MESSAGE)
        .pick(ARG_FILE_MESSAGE)
        .pick(ARG_NO_EDITOR)
        .pick(ARG_REPLACE)
        .pick(ARG_POLICY)
        .pick_or_route(arg![Vec<String>], || ErrorTrackNoFiles.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (message, file_message, no_editor, replace, policy, files, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "track") {
        return next;
    }

    if files.is_empty() {
        return ErrorTrackNoFiles.into();
    }

    // A name this build does not carry is refused before any file is read, which is where a typo
    // belongs: the run has nothing to undo and the list of what it could have said is right there.
    let policy = match policy {
        None => None,
        Some(name) => match librorolala::storage::policy(&name) {
            Some(found) => Some(found.name),
            None => return ErrorTrackPolicy { name }.into(),
        },
    };

    StateTrack {
        files,
        message,
        file_messages: file_message
            .into_iter()
            // The framework hands a repeated flag its own name back as one of the values; it is not
            // a message a caller wrote.
            .filter(|message| !is_flag(message, &ARG_FILE_MESSAGE))
            .collect(),
        no_editor: matches!(no_editor, Flag::Active),
        replace: matches!(replace, Flag::Active),
        policy,
        force: **force,
    }
    .into()
}

/// The state a recording starts in.
#[derive(Grouped)]
pub struct StateTrack {
    /// The files named, as they were written.
    files: Vec<String>,
    /// What the whole group did, when it was said.
    message: Option<String>,
    /// What one file did, when any were given.
    file_messages: Vec<String>,
    /// Whether an editor is never to be opened.
    no_editor: bool,
    /// Whether each file's version is replaced rather than added after.
    replace: bool,
    /// The cut every file this run writes is to be made with, by its registered name.
    policy: Option<&'static str>,
    /// Whether a file another account holds is recorded anyway.
    force: bool,
}

#[chain]
pub fn handle_track(
    state: StateTrack,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    account: &mut LazyRes<ResCurrentAccount>,
    editor: &ResEditor,
    progress: &ResProgressSetting,
) -> Next {
    let reporting = Reporting::after(*progress, WATCH_AFTER);

    // The handle is let go of before the reader is waited for: the last handle dropping is what tells
    // the reader there is nothing more to come, and waiting first would wait forever.
    let next = {
        let progress = reporting.progress();
        record(
            state, workspace, vault, storage, index, account, editor, &progress,
        )
    };

    reporting.finish();

    next
}

/// The recording itself, with a running reader to say what it is doing.
///
/// Split from [`handle_track`] so that the reader is started and waited for around it exactly once,
/// however many ways the recording has of returning.
fn record(
    state: StateTrack,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    account: &mut LazyRes<ResCurrentAccount>,
    editor: &ResEditor,
    progress: &Progress,
) -> Next {
    let StateTrack {
        files,
        message,
        file_messages,
        no_editor,
        replace,
        policy,
        force,
    } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };

    let root = held.get_root().to_path_buf();

    // A variant file is content waiting to be joined rather than work to record, so where it lies is
    // left out of every reading here — and a file waiting for one is recorded as the merge it is.
    let mut merging = Merging::read(&root);
    let ignored = merging.ignored(&layout);

    // Recording works on a tree that is settled: a path the Layout names and the tree does not hold
    // is something to settle first, with `rola align`, rather than something to record around. A
    // move is not one of these — naming a path where it is now is how a move is confirmed.
    let diff = match tree_diff(&layout, held, DEFAULT_ALIKE) {
        Ok(diff) => diff,
        Err(error) => return fail(error.to_string()),
    };

    if !diff.lost.is_empty() {
        return ErrorTrackLost {
            paths: diff
                .lost
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
        }
        .into();
    }

    let Some(store) = storage.get_ref().as_ref() else {
        return fail(t!("track.err_no_store").trim());
    };

    let Some(vcs) = index.get_ref().as_ref() else {
        return fail(t!("track.err_no_index").trim());
    };

    let creator_name = match account.get_ref().must_bind() {
        Ok(name) => name,
        Err(error) => return error.into(),
    };

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return fail(error.to_string()),
    };

    let prepared = match prepare(
        &layout,
        &root,
        &cwd,
        &files,
        &diff.renamed,
        &ignored,
        policy,
    ) {
        Ok(prepared) => prepared,
        Err(next) => return next,
    };

    // A file the Vault's fetched copy says another account holds is not one to record: the version
    // made here is one this side cannot push, so recording it makes the conflict rather than
    // avoiding it. Only what the tree says changed is asked about — a file that did not change
    // records nothing — and a Vault that tracks nothing, a copy that is not there, and a file the
    // copy does not name are all answered with nothing to refuse.
    if !force {
        let tracked = tracked_vault(held, &layout);
        let unowned = match unowned(
            held,
            &layout,
            tracked.as_deref(),
            Some(&creator_name),
            &diff,
        ) {
            Ok(unowned) => unowned,
            Err(error) => return fail(error.to_string()),
        };
        let named: BTreeSet<&str> = prepared.iter().map(|file| file.path.as_str()).collect();
        let blocked: Vec<String> = unowned
            .into_iter()
            .filter(|path| named.contains(path.as_str()))
            .collect();

        if !blocked.is_empty() {
            return ErrorTrackUnowned { paths: blocked }.into();
        }
    }

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return fail(error.to_string()),
    };

    // What is worth reading is what the tree says changed, plus what `--replace` names and what the
    // reading could not look at: a file the Layout agrees with holds content the store already has, so
    // it is not read at all. The reading happens before the message is asked and before anything is
    // written — see `enumerate` — so a file the reading agrees with is neither asked about nor
    // written, and a move that changed nothing under it is left to `rola align` as it always was.
    let recordable: BTreeSet<LayoutPath> = diff
        .modified
        .iter()
        .chain(diff.untagged.iter())
        .cloned()
        .collect();

    // A file the reading could not look at is not one it can say did not change, so it is read
    // again here rather than skipped: what it then finds is either its content recorded or the
    // failure the reading met.
    let unreadable: BTreeSet<&str> = diff
        .failed
        .iter()
        .map(|failed| failed.path.as_str())
        .collect();

    // The files worth reading are the ones the reading found changed, the ones `--replace` names
    // whatever the reading found, and the ones the reading could not look at. Everything else holds
    // content the store already has, so it is neither read nor written.
    let candidates: Vec<Prepared> = prepared
        .into_iter()
        .filter(|file| {
            replace || recordable.contains(&file.path) || unreadable.contains(file.path.as_str())
        })
        .collect();

    // What each file now holds, and whether that is a change, is worked out before anything is asked
    // or written: the content is read here so that the message is asked for the files that will be
    // recorded and no others, and a run whose editor is left empty has then written nothing at all.
    let mut enumerated = match enumerate(vcs, &runtime, progress, candidates, replace) {
        Ok(enumerated) => enumerated,
        Err(next) => return next,
    };

    // The files in the list are the ones that changed; one that did not is not recorded, so it is
    // not something a message stands for either — including when it was named on the command line.
    enumerated.retain(|file| file.changed);
    let changing = enumerated;
    let files: Vec<&Prepared> = changing.iter().map(|file| &file.file).collect();

    if !file_messages.is_empty() && file_messages.len() != files.len() {
        return message_error(MessageError::Mismatch);
    }

    // An amend with no new words keeps the ones the replaced version said, so they are what the
    // editor opens on and what `--no-editor` records as they stand. They are read from the index
    // only in that case, so a run that says what it wants never pays for them.
    let keeps_words = replace && message.is_none() && file_messages.is_empty();

    let own: Vec<String> = if !file_messages.is_empty() {
        file_messages
    } else if keeps_words {
        match replaced_messages(vcs, &runtime, &changing) {
            Ok(messages) => messages,
            Err(next) => return next,
        }
    } else {
        vec![String::new(); files.len()]
    };

    // Nothing changing is nothing to say: a run that found every file already recorded records
    // nothing, and is not asked to explain a change it did not make.
    let messages = if files.is_empty() {
        Vec::new()
    } else {
        match decide(&files, &own, message, no_editor, editor, &root) {
            Ok(messages) => messages,
            Err(next) => return next,
        }
    };

    // The content of every file that is to be recorded goes into the store, read again as it goes — the
    // price of having asked for the message first (see `enumerate`).
    let staged = match write(store, &runtime, progress, changing) {
        Ok(staged) => staged,
        Err(next) => return next,
    };

    let mut items = Vec::with_capacity(staged.len());
    let mut committed = Vec::with_capacity(staged.len());

    // A version is one thing made per file, so the bar for making them counts files rather than the
    // bytes the content above was measured in.
    let mut placing = (!staged.is_empty())
        .then(|| progress.begin(t!("track.recording").to_string(), Some(staged.len() as u64)));

    // What the run does to the Layout is gathered up and written when it is done, so a run that
    // records many files pays one read of what others wrote and one flushed write per shard rather
    // than the same for each file.
    let mut batch = layout.batch();

    // What the run makes in the index is gathered up the same way: a run writes its objects as one
    // pack rather than a file per object, which is what a file creation each was.
    let mut writes = vcs.batch();

    for (at, file) in staged.iter().enumerate() {
        let text = &messages[at];

        // A file a variant is waiting for is recorded as the merge it is, and the waiting is over
        // once it is: what the merge was is read here, where the version is made.
        let pending = merging.at(file.file.id);

        if let Err(next) = commit(
            &layout,
            vcs,
            &runtime,
            &creator_name,
            &mut writes,
            &mut batch,
            file,
            text,
            replace,
            pending,
            &merging,
            &root,
        ) {
            return next;
        }

        if let Some(pending) = pending {
            if let Some(path) = merging.variant_path(&layout, pending) {
                let disk = root.join(path.to_path_buf());

                // A variant file that is already gone is one the merge can still be recorded from:
                // the content that was checked in is what the version names, and the file was only
                // how it was shown.
                if disk.is_file()
                    && let Err(error) = fs::remove_file(&disk)
                {
                    return fail(error.to_string());
                }
            }

            merging.remove(file.file.id);
        }

        committed.push((file.file.path.clone(), file.file.disk.clone()));
        items.push(TrackItem {
            path: file.file.path.as_str().to_owned(),
            action: if replace {
                TrackAction::Replaced
            } else {
                TrackAction::if_updated(&file.file)
            },
        });

        if let Some(task) = &mut placing {
            task.advance_by(1);
        }
    }

    // What the run made in the index is written before the Layout that names it: a Layout naming a
    // version the index does not hold is a version nothing reads, so the objects are made first and
    // the Layout follows them. One pack for the whole run, and nothing the run wrote is left loose.
    if let Err(error) = runtime.block_on(writes.publish()) {
        return fail(error.to_string());
    }

    // What was recorded is what the tree and the Layout now agree on, so the reading remembers it —
    // only for the paths recorded, since remembering the whole tree would take every other change
    // with it and a later `rola status` would have nothing to say about them.
    if let Err(error) = batch.commit() {
        return failed(&error);
    }

    drop(placing);

    if let Err(next) = remember(&layout, &root, &committed) {
        return next;
    }

    if let Err(error) = merging.write(&root) {
        return fail(error.to_string());
    }

    // What the run wrote into the index is laid out with it, so a Workspace does not gather a loose
    // object per version it records. A run that recorded nothing has nothing to lay out.
    if !committed.is_empty() {
        pack_index(vcs, &runtime);
    }

    ResultTrack { items }.into()
}

/// Lays the index's objects out as one pack, when this run has left enough of them to gather.
///
/// A run writes what it makes as a pack already, so what gathers between packings is a pack per run
/// rather than a loose file per object. What a gathering costs is a rewrite of every object the index
/// holds, though, which grows with the index while what it saves does not — so a run gathers only when
/// the loose objects it left number [`LOOSE_BEFORE_PACK`], or when the packs it left number more than
/// [`PACKS_BEFORE_REPACK`], and until then they are valid where they are: a later run or `rola pack`
/// settles them. It is best effort — a pack that cannot be made because another run holds the lock
/// leaves an index that is correct and merely ungathered, and a failure here does not fail a recording
/// that is already durable.
fn pack_index(index: &VCSIndex, runtime: &tokio::runtime::Runtime) {
    let Ok(guard) = runtime.block_on(index.lock()) else {
        return;
    };

    let _ = runtime.block_on(guard.repack_when_due(LOOSE_BEFORE_PACK, PACKS_BEFORE_REPACK));
}

/// Writes down what a recording left the tree and the Layout agreeing on.
fn remember(layout: &Layout, root: &Path, committed: &[(LayoutPath, PathBuf)]) -> Result<(), Next> {
    if committed.is_empty() {
        return Ok(());
    }

    let path = cache_path(root, layout);
    let mut cache = Cache::read(&path);

    for (layout_path, disk) in committed {
        // The reading of the tree already wrote down what the file holds, and nothing in this run
        // has touched the file since — so what it found is taken rather than the file read a third
        // time, and the Layout is held to agree with the digest it wrote down.
        let entry = match cache.get(layout_path) {
            Some(read) => read.clone().with_agreed(Some(read.digest())),
            None => entry_of(disk).map_err(|error| fail(error.to_string()))?,
        };
        cache.insert(layout_path.clone(), entry);
    }

    cache.write(&path).map_err(|error| fail(error.to_string()))
}

/// One file, resolved against the Layout and the tree.
struct Prepared {
    /// Where it sits in the Layout.
    path: LayoutPath,
    /// Where it sits on disk.
    disk: PathBuf,
    /// The `Uuid` it is known by: the one it holds, or the one it is about to be given.
    id: Uuid,
    /// What the Layout holds for it, when it holds anything.
    data: Option<MutableData>,
    /// The cut it is to be written with, by its registered name, when one was settled.
    ///
    /// It is both what the write is made with and what the version goes on to prefer: a run that
    /// says how a file is to be cut is saying it for next time as much as for this one.
    policy: Option<&'static str>,
    /// Where the reading found it, when naming it confirmed a move.
    ///
    /// A path that moved is recorded only when what it holds changed: a move by itself is not
    /// something a recording is about, so a file that was moved and not edited is left for
    /// `rola align` to confirm. One that was edited as well is recorded — the change is the
    /// recording — and the move is carried in with it, since the Layout has to name where the file
    /// now is for the change to be recorded at all.
    moved_from: Option<LayoutPath>,
}

/// One file whose content has been put into the store, and what was found about it.
struct Staged {
    /// The file itself.
    file: Prepared,
    /// The key its content is stored under.
    storage: Key,
    /// The version it is at now, when it was recorded before.
    recorded: Option<Recorded>,
}

/// One candidate file, read and weighed before anything is asked or written.
struct Enumerated {
    /// The file itself.
    file: Prepared,
    /// Whether what it holds is a change from what the Layout names: what decides whether a message is
    /// asked for it and whether a version is made of it.
    changed: bool,
    /// The version it is at now, when it was recorded before.
    recorded: Option<Recorded>,
}

/// A file's recorded version, and what its variant holds.
///
/// The variant is kept whole rather than only the base hash because amending reads the base from it
/// and writes a variant of its own without needing the version object it was made from.
struct Recorded {
    /// The version the Layout names now.
    version: Version,
    /// The variant that version points at.
    variant: Variant,
    /// The key the version's content is stored under.
    storage: [u8; 32],
}

/// Resolves each name against the Layout and the tree, or refuses.
///
/// A name is written the way a shell writes one — relative to where the run was made, or absolute —
/// and what a Layout names is a path relative to the Workspace root, so one becomes the other here.
/// A directory is taken recursively: every file under it is recorded, read the way the tree reading
/// reads them, so what a Workspace keeps for itself and what another Workspace holds are left out.
/// What is neither a file nor a directory, is not inside the Workspace, or is part of the
/// Workspace's own data is refused rather than recorded.
///
/// Each file's cut is settled here as well: `policy` if the run named one, else the preference the
/// entry already has, else nothing. Settling it before anything is asked or written is what lets a
/// preference this build cannot honour stop the run with the workspace untouched — and it settles
/// every file, not only the ones that turn out to have changed, because a file's preference is not
/// something a change is needed to discover.
fn prepare(
    layout: &Layout,
    root: &Path,
    cwd: &Path,
    names: &[String],
    renames: &[PathRename],
    ignored: &BTreeSet<LayoutPath>,
    policy: Option<&'static str>,
) -> Result<Vec<Prepared>, Next> {
    let root = normalize(root);

    // What each name stands for: a file, or a directory whose files are taken in its place.
    let mut standing = Vec::with_capacity(names.len());

    for given in names {
        let disk = normalize(&resolve(cwd, given));

        if !disk.exists() {
            return Err(ErrorTrackFile::new(given, TrackFileError::NotAFile).into());
        }

        let Ok(relative) = disk.strip_prefix(&root) else {
            return Err(ErrorTrackFile::new(given, TrackFileError::OutsideWorkspace).into());
        };

        if disk.is_dir() {
            // A directory is read where it is rather than by walking the whole Workspace and
            // filtering, so naming `src/` reads `src/` alone. What the walk leaves out is left out
            // just the same, since it is the same walk — see `reachable` for the one thing a walk
            // started inside the tree does not see for itself.
            check_directory(relative, given)?;
            standing.push(Named::Directory(disk));
        } else if disk.is_file() {
            // A variant file is content waiting to be joined rather than work to record, so a run
            // that names one is asking for nothing and is answered with nothing.
            let path = file_path(relative, given)?;
            if ignored.contains(&path) {
                continue;
            }

            standing.push(Named::File(path, disk));
        } else {
            return Err(ErrorTrackFile::new(given, TrackFileError::NotAFile).into());
        }
    }

    // A directory is read through the same walk the tree reading uses, so what it leaves out — the
    // Workspace's own `.rola`, a Workspace nested inside it, a symbolic link — is left out here too,
    // and so is every variant file among what it found. The walk starts at the directory named
    // rather than at the Workspace root, so naming a directory reads that directory alone.
    let mut files: Vec<(LayoutPath, PathBuf)> = Vec::new();
    for name in standing {
        match name {
            Named::File(path, disk) => files.push((path, disk)),
            Named::Directory(disk) => {
                // A walk started inside the tree does not see the directories above it, so whether
                // the whole-tree walk would have descended into this one is told here; a directory
                // it would not have reached holds work that is not this Workspace's.
                if !reachable(&root, &disk) {
                    continue;
                }

                let mut under: Vec<(LayoutPath, PathBuf)> = walk(&disk)
                    .0
                    .into_iter()
                    .filter_map(|found| {
                        let relative = found.disk.strip_prefix(&root).ok()?;
                        let path = LayoutPath::from_relative(relative).ok()?;

                        (!ignored.contains(&path)).then_some((path, found.disk))
                    })
                    .collect();
                under.sort_by(|left, right| left.0.cmp(&right.0));
                files.extend(under);
            }
        }
    }

    // A file named twice — under a directory that was named as well, or as itself — is one file, so
    // it is recorded once.
    let mut seen = BTreeSet::new();
    files.retain(|(path, _)| seen.insert(path.clone()));

    let mut prepared = Vec::with_capacity(files.len());

    for (path, disk) in files {
        // A path the tree moved still has its entry under where it was, so the entry is read there.
        // The move itself is made only as the file is recorded — a move the file did not change
        // under is not recorded at all, and is left to `rola align` — so a run that is refused
        // before then, for a message it could not be given, has changed nothing.
        let moved_from = renames
            .iter()
            .find(|rename| rename.to == path)
            .map(|rename| rename.from.clone());
        let source = moved_from.as_ref().unwrap_or(&path);

        let (id, data) = match layout
            .id_of(source)
            .and_then(|id| layout.entry(id).map(|data| (id, data)))
        {
            Some((id, data)) => (id, Some(data)),
            None => (Uuid::new_v4(), None),
        };

        let chosen = settle(policy, data.as_ref(), path.as_str())?;

        prepared.push(Prepared {
            path,
            disk,
            id,
            data,
            policy: chosen,
            moved_from,
        });
    }

    Ok(prepared)
}

/// The cut one file is to be written with: the run's, else the entry's preference, else none.
///
/// A preference this build cannot honour is a refusal rather than a fall back. Falling back would
/// write the file cut by something other than what it asks for, which is not a smaller failure than
/// stopping: it is a write that does not deduplicate against what is already stored for the file,
/// made silently, and discovered only when someone measures the store months later.
fn settle(
    policy: Option<&'static str>,
    data: Option<&MutableData>,
    path: &str,
) -> Result<Option<&'static str>, Next> {
    if let Some(name) = policy {
        return Ok(Some(name));
    }

    let Some(name) = data.and_then(MutableData::preference) else {
        return Ok(None);
    };

    librorolala::storage::policy(name).map_or_else(
        || {
            Err(ErrorTrackPreference {
                path: path.to_owned(),
                name: name.to_owned(),
            }
            .into())
        },
        |found| Ok(Some(found.name)),
    )
}

/// What one name on the command line stands for.
enum Named {
    /// A file, and where it sits on disk.
    File(LayoutPath, PathBuf),
    /// A directory, and where it sits on disk.
    Directory(PathBuf),
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// `path` with its `.` dropped and its `..` climbed, worked out lexically rather than on disk.
///
/// A name is written against where the run was made, so `.` names the directory it was made in and
/// `..` its parent — and a path that spells one place two ways is one place. Working it out here,
/// before the disk is asked anything, is what lets `.` name the Workspace root rather than a path
/// with no components in it.
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

/// Refuses a directory that cannot be named in the Layout: one outside the Workspace, or one that is
/// part of the Workspace's own data. The Workspace root itself names nothing and is always allowed.
fn check_directory(relative: &Path, given: &str) -> Result<(), Next> {
    if relative.as_os_str().is_empty() {
        return Ok(());
    }

    file_path(relative, given)?;

    Ok(())
}

/// The path `relative` names, or the failure for why it cannot be named in the Layout.
fn file_path(relative: &Path, given: &str) -> Result<LayoutPath, Next> {
    let Ok(path) = LayoutPath::from_relative(relative) else {
        return Err(ErrorTrackFile::new(given, TrackFileError::OutsideWorkspace).into());
    };

    if path.as_str().split('/').any(|part| part == DATA_DIR) {
        return Err(ErrorTrackFile::new(given, TrackFileError::InsideData).into());
    }

    Ok(path)
}

/// Whether the whole-tree walk would have descended into `dir`.
///
/// The walk never descends into a directory that keeps a data directory of its own — that directory
/// is a Workspace of its own — and a walk started inside the tree does not see the directories above
/// it. So whether any of them is such a Workspace is asked here; a directory under one holds work
/// that is not this Workspace's to record.
fn reachable(root: &Path, dir: &Path) -> bool {
    let mut at = Some(dir);

    while let Some(current) = at {
        if current == root {
            return true;
        }
        if current.join(DATA_DIR).is_dir() {
            return false;
        }

        at = current.parent();
    }

    false
}

/// Works out what each changing file's message is, asking an editor when it was not said.
///
/// A file whose own message was given is paired with it here; one that was not carries the group's
/// alone. What none of that answers is asked of the editor, which is the only way left.
fn decide(
    files: &[&Prepared],
    own: &[String],
    message: Option<String>,
    no_editor: bool,
    editor: &ResEditor,
    root: &Path,
) -> Result<Vec<String>, Next> {
    if let Some(global) = message {
        return assemble_all(files, &global, own);
    }

    if no_editor {
        return assemble_all(files, "", own);
    }

    edit_messages(files, own, editor, root)
}

/// Splices a group message and each file's own message into the message that file carries.
fn assemble_all(files: &[&Prepared], global: &str, own: &[String]) -> Result<Vec<String>, Next> {
    let mut messages = Vec::with_capacity(files.len());

    for (file, own) in files.iter().zip(own) {
        let Some(spliced) = assemble(global, own) else {
            return Err(message_error(MessageError::Missing {
                path: file.path.as_str().to_owned(),
            }));
        };

        if spliced.len() > LONGEST_MESSAGE {
            return Err(message_error(MessageError::TooLong {
                path: file.path.as_str().to_owned(),
                length: spliced.len(),
            }));
        }

        messages.push(spliced);
    }

    Ok(messages)
}

/// The one message a file carries, from the group's and its own.
///
/// Three shapes come out of the two, which is what a message has always been able to be: the group
/// alone, the file's own alone, or the two joined by `: `. A file's own message that begins with `!`
/// is the second shape — it says the group does not apply here — which is how a run says in one
/// argument what would otherwise be a choice about the whole group. Nothing at all is `None`.
fn assemble(global: &str, own: &str) -> Option<String> {
    let global = global.trim();
    let own = own.trim();

    if own.is_empty() {
        return (!global.is_empty()).then(|| global.to_owned());
    }

    if let Some(rest) = own.strip_prefix('!') {
        let rest = rest.trim();
        return (!rest.is_empty()).then(|| rest.to_owned());
    }

    Some(if global.is_empty() {
        own.to_owned()
    } else {
        format!("{global}: {own}")
    })
}

/// Asks an editor for the messages, opening it again while what it wrote is still too long.
// The `?` operator is not used to read the editor out of the resource: the failure it would carry is
// not one the chain's error type is built from, so it is turned into the answer by hand.
#[allow(clippy::question_mark)]
fn edit_messages(
    files: &[&Prepared],
    prefill: &[String],
    editor: &ResEditor,
    root: &Path,
) -> Result<Vec<String>, Next> {
    let editor_name = match editor.must_bind() {
        Ok(name) => name,
        Err(error) => return Err(error.into()),
    };

    let path = root.join(WORKSPACE_EDITING_PATH);
    let mut prefill: Vec<String> = prefill.to_vec();
    let mut warnings: Vec<String> = Vec::new();

    for _ in 0..EDIT_ATTEMPTS {
        if let Err(error) = fs::write(&path, template(files, &prefill, &warnings)) {
            return Err(fail(error.to_string()));
        }

        if let Err(error) = open(editor_name, &path) {
            return Err(error.into());
        }

        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => return Err(fail(error.to_string())),
        };

        let (global, per) = parse_editing(&text);
        let global = global.unwrap_or_default();

        // Writing nothing at all is a run changing its mind, not one to be asked again: there is no
        // message to be had, so it stops here rather than opening the editor on the same nothing.
        let wrote_nothing = global.trim().is_empty()
            && files.iter().all(|file| {
                per.get(&file.id.to_string())
                    .is_none_or(|own| own.trim().is_empty())
            });
        if wrote_nothing {
            return Err(ErrorEditingAborted.into());
        }

        let mut messages = Vec::with_capacity(files.len());
        let mut refused = Vec::new();

        for file in files {
            let own = per.get(&file.id.to_string()).map_or("", String::as_str);

            let Some(spliced) = assemble(&global, own) else {
                refused.push(
                    t!("track.editing_missing", path = file.path.as_str())
                        .trim()
                        .to_string(),
                );
                continue;
            };

            if spliced.len() > LONGEST_MESSAGE {
                refused.push(
                    t!(
                        "track.editing_warning",
                        path = file.path.as_str(),
                        length = spliced.len()
                    )
                    .trim()
                    .to_string(),
                );
                continue;
            }

            messages.push(spliced);
        }

        if refused.is_empty() {
            return Ok(messages);
        }

        warnings = refused;
        // What the caller wrote is kept as what the editor opens with next time, so nothing is lost
        // between one attempt and the next.
        prefill = files
            .iter()
            .map(|file| per.get(&file.id.to_string()).cloned().unwrap_or_default())
            .collect();
    }

    Err(message_error(MessageError::Unresolved))
}

/// The content an editor is opened on: the group's section and one per file.
fn template(files: &[&Prepared], prefill: &[String], warnings: &[String]) -> String {
    let mut text = String::new();

    for warning in warnings {
        let _ = writeln!(text, "# == {warning}");
    }

    let _ = writeln!(text, "# {}\n", t!("track.editing_header").trim());
    let _ = writeln!(
        text,
        ":: GLOBAL ::\n# {}\n",
        t!("track.editing_global").trim()
    );

    for (at, file) in files.iter().enumerate() {
        let own = prefill.get(at).map_or("", String::as_str);
        let _ = writeln!(
            text,
            ":: {} ::\n# {}: `{}`\n{own}\n",
            file.id,
            t!("track.editing_file").trim(),
            file.path.as_str()
        );
    }

    text
}

/// What an edited file said: the group's message and each section's own, by key.
///
/// Only lines that are not comments count, and a section is named by `:: KEY ::`, with `GLOBAL` the
/// group's own. What a reader wrote is otherwise taken as it stands, so a message may be anything a
/// message may be.
fn parse_editing(text: &str) -> (Option<String>, BTreeMap<String, String>) {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut key: Option<String> = None;
    let mut body = String::new();

    for line in text.lines() {
        if let Some(name) = section_name(line) {
            if let Some(previous) = key.take() {
                sections.push((previous, body.clone()));
            }
            body.clear();
            key = Some(name);
            continue;
        }

        if line.trim_start().starts_with('#') {
            continue;
        }

        if key.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }

    if let Some(previous) = key {
        sections.push((previous, body));
    }

    let mut global = None;
    let mut per = BTreeMap::new();

    for (name, body) in sections {
        let body = body.trim().to_owned();
        if name.eq_ignore_ascii_case("GLOBAL") {
            global = Some(body);
        } else {
            per.insert(name, body);
        }
    }

    (global, per)
}

/// The answer messages that do not work out are given.
fn message_error(kind: MessageError) -> Next {
    ErrorTrackMessage { kind }.into()
}

/// The name a section line carries, when it is one: `:: KEY ::`.
fn section_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("::")?.strip_suffix("::")?;

    let name = inner.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Reads every candidate file and says which of them changed, before anything is asked or written.
///
/// The content is read here — the whole of every candidate — because whether a file is worth a message
/// and a version is whether what is on disk differs from what the Layout names, and that is only known by
/// hashing it. The bar is drawn over that reading, since it is the part a reader has to wait through.
///
/// What this is for is the order of the run: the message is asked for *after* this and *before* anything
/// is written, so the editor lists exactly the files that will be recorded and a run whose editor is left
/// empty has written nothing. Asking after the content was written would leave the store holding content
/// no version names.
///
/// # Errors
///
/// Renders [`ErrorTrackReplace`] when `--replace` names a file the Layout records no version for or one
/// whose version is a merge, and [`ErrorTrackFailed`] when a file cannot be read.
fn enumerate(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    progress: &Progress,
    candidates: Vec<Prepared>,
    replace: bool,
) -> Result<Vec<Enumerated>, Next> {
    let bytes: u64 = candidates.iter().map(|file| size_of(&file.disk)).sum();
    let mut task = (!candidates.is_empty())
        .then(|| progress.begin(t!("track.enumerating").to_string(), Some(bytes)));

    let mut enumerated = Vec::with_capacity(candidates.len());

    for file in candidates {
        let size = size_of(&file.disk);

        let recorded = match &file.data {
            Some(data) => Some(read_recorded(index, runtime, data.version())?),
            None => None,
        };

        if replace {
            let Some(recorded) = &recorded else {
                return Err(ErrorTrackReplace {
                    path: file.path.as_str().to_owned(),
                    kind: ReplaceError::Unrecorded,
                }
                .into());
            };

            // Replacing a merge would drop what was joined in: a version has one base, and a merge is
            // the record of there being two.
            if recorded.variant.join().is_some() {
                return Err(ErrorTrackReplace {
                    path: file.path.as_str().to_owned(),
                    kind: ReplaceError::Merge,
                }
                .into());
            }
        }

        let key = key_of_file(&file.disk).map_err(|error| fail(error.to_string()))?;
        let changed = replace
            || recorded
                .as_ref()
                .is_none_or(|recorded| recorded.storage != *key.digest());

        if let Some(task) = &mut task {
            task.advance_by(size);
        }

        enumerated.push(Enumerated {
            file,
            changed,
            recorded,
        });
    }

    Ok(enumerated)
}

/// Puts the content of every file that is to be recorded into the store.
///
/// These are the files [`enumerate`] found changed, so nothing else is written: a file whose content the
/// store already has has nothing to put there. Each is read again, which is what makes this a pass of its
/// own and what its bar is drawn over — the price of having asked for the message before writing.
///
/// # Errors
///
/// Renders [`ErrorTrackFailed`] when the store refuses to write a file.
fn write(
    store: &RorolalaStorage,
    runtime: &tokio::runtime::Runtime,
    progress: &Progress,
    changing: Vec<Enumerated>,
) -> Result<Vec<Staged>, Next> {
    let bytes: u64 = changing.iter().map(|file| size_of(&file.file.disk)).sum();
    let mut task = (!changing.is_empty())
        .then(|| progress.begin(t!("track.writing").to_string(), Some(bytes)));

    let mut staged = Vec::with_capacity(changing.len());

    for Enumerated { file, recorded, .. } in changing {
        let size = size_of(&file.disk);

        // What a file is cut with is settled per file, so the store handed to the write is the store
        // told which cut to make. A file nothing settled is written by the store's own choosing.
        let told = file.policy.map_or_else(
            || store.clone(),
            |name| store.clone().with_forced_cut(Chunking::Policy(name)),
        );

        let storage = runtime
            .block_on(store_file(&told, &file.disk))
            .map_err(|error| fail(error.to_string()))?;

        if let Some(task) = &mut task {
            task.advance_by(size);
        }

        staged.push(Staged {
            file,
            storage,
            recorded,
        });
    }

    Ok(staged)
}

/// How large a file is, or nothing when it cannot be looked at.
///
/// Used for the bars alone, which are measured in bytes: a size that cannot be read is nothing to draw
/// rather than a failure, since the write that follows reports the failure in its own words.
fn size_of(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |data| data.len())
}

/// Makes a version of the staged content and makes the Layout name it.
///
/// `replace` builds the new version on what the version being replaced was built on, rather than on
/// that version: the one that was there stands as if it had never been made, which is what an amend
/// is. That base is carried by the replaced variant as the hash it is, so the version object it
/// names is never read back to make the new one.
fn commit(
    layout: &Layout,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    creator_name: &str,
    writes: &mut VCSIndexBatch,
    batch: &mut Batch<'_>,
    file: &Staged,
    text: &str,
    replace: bool,
    pending: Option<&Pending>,
    merging: &Merging,
    root: &Path,
) -> Result<(), Next> {
    if replace && pending.is_some() {
        return Err(ErrorTrackReplace {
            path: file.file.path.as_str().to_owned(),
            kind: ReplaceError::Merging,
        }
        .into());
    }

    let creator =
        Creator::try_from(creator_name.to_owned()).map_err(|error| fail(error.to_string()))?;
    let message = Message::try_from(text.to_owned()).map_err(|error| fail(error.to_string()))?;

    let creator_key = writes.add(&creator);
    let message_key = writes.add(&message);

    let variant = match (&file.recorded, replace) {
        (Some(recorded), true) => Variant::new_bare_variant(
            *file.storage.digest(),
            *recorded.variant.base_version(),
            None,
            *creator_key.digest(),
            *message_key.digest(),
            recorded.variant.base_version_num(),
        ),
        (Some(recorded), false) => {
            let base = recorded.version.new_variant(
                *file.storage.digest(),
                *creator_key.digest(),
                *message_key.digest(),
            );

            match pending {
                None => base,
                Some(pending) => {
                    let joined = runtime
                        .block_on(index.read(Key::new(*pending.variant())))
                        .map_err(|error| fail(error.reason()))?
                        .expect_variant()
                        .map_err(|error| fail(error.to_string()))?;

                    verify_variant_file(layout, merging, root, pending, &joined)?;

                    // Content that is exactly the variant's own is no merge but a replacement: the
                    // file now holds what was checked in, and what is recorded is that, on the chain
                    // the file was already on. Only content of its own is joined.
                    if *joined.storage_hash() == *file.storage.digest() {
                        base
                    } else {
                        base.new_merge_variant(*file.storage.digest(), &joined)
                    }
                }
            }
        }
        (None, _) => {
            let root = Version::root();
            writes.add(&root);
            root.new_variant(
                *file.storage.digest(),
                *creator_key.digest(),
                *message_key.digest(),
            )
        }
    };
    let version = variant.new_version();

    writes.add(&variant);
    let version_key = writes.add(&version);

    // What the version goes on to prefer, whatever this write did: the preference is what the *next*
    // write is cut by, so it is recorded from what was settled rather than from what the store did.
    // A file nothing was settled for keeps saying nothing, and the next write chooses again.
    let data = MutableData::new(
        Some(creator_name.to_owned()),
        *version_key.digest(),
        String::new(),
    )
    .with_preference(file.file.policy.map(str::to_owned));

    // A moved file is brought to where it is now: the move is the last thing done, so that a
    // failure before it — writing the version the change is — leaves the Layout as it was. The
    // variant file waiting for it, if there is one, moves with it.
    if let Some(from) = &file.file.moved_from {
        if let Err(error) = batch.move_path(from, &file.file.path) {
            return Err(failed(&error));
        }

        if let Err(error) = merging.follow(root, file.file.id, from, &file.file.path) {
            return Err(fail(error.to_string()));
        }
    }

    if file.file.data.is_some() {
        if let Err(error) = batch.update_entry(file.file.id, data) {
            return Err(failed(&error));
        }

        return Ok(());
    }

    if let Err(error) = batch.create_file(file.file.id, data, &file.file.path) {
        return Err(failed(&error));
    }

    Ok(())
}

/// Reads the variant file waiting for a file, refusing when it no longer holds the variant's content.
///
/// What a recording says is that this variant was joined; a file that no longer holds what the
/// variant names would record a merge of something else, which is worse than recording nothing.
fn verify_variant_file(
    layout: &Layout,
    merging: &Merging,
    root: &Path,
    pending: &Pending,
    joined: &Variant,
) -> Result<(), Next> {
    let Some(path) = merging.variant_path(layout, pending) else {
        return Ok(());
    };
    let disk = root.join(path.to_path_buf());

    // A variant file that is already gone is one the merge can still be recorded from: what was
    // checked in is what the version names, and the file was only how it was shown.
    if !disk.is_file() {
        return Ok(());
    }

    let entry = entry_of(&disk).map_err(|error| fail(error.to_string()))?;
    if entry.digest() != *joined.storage_hash() {
        return Err(ErrorTrackMergeChanged {
            path: path.as_str().to_owned(),
        }
        .into());
    }

    Ok(())
}

/// What each changing file's version being replaced says, read from the index.
///
/// It is read only for a run that amends without writing new words, since that is the only run the
/// words are read for.
fn replaced_messages(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    files: &[Enumerated],
) -> Result<Vec<String>, Next> {
    let mut messages = Vec::with_capacity(files.len());

    for file in files {
        // UNWRAP: `enumerate` refuses a file the Layout records nothing for when `--replace` was asked
        // for, so every changing file here has a version whose words can be read.
        let recorded = file.recorded.as_ref().unwrap();

        let message = runtime
            .block_on(index.read(Key::new(*recorded.variant.message())))
            .map_err(|error| fail(error.reason()))?
            .expect_message()
            .map_err(|error| fail(error.to_string()))?;

        messages.push(message.read_to_string().to_owned());
    }

    Ok(messages)
}

/// The `Version` stored under `hash`, with the variant it points at and the content key that names.
fn read_recorded(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: [u8; 32],
) -> Result<Recorded, Next> {
    let version = runtime
        .block_on(index.read(Key::new(hash)))
        .map_err(|error| fail(error.reason()))?
        .expect_version()
        .map_err(|error| fail(error.to_string()))?;

    let variant = runtime
        .block_on(index.read(Key::new(*version.variant())))
        .map_err(|error| fail(error.reason()))?
        .expect_variant()
        .map_err(|error| fail(error.to_string()))?;

    Ok(Recorded {
        version,
        storage: *variant.storage_hash(),
        variant,
    })
}

/// The answer a store, an index or a Layout refusing is given.
fn fail(cause: impl Into<String>) -> Next {
    ErrorTrackFailed::cause(cause.into()).into()
}

/// What happened to one file.
#[derive(Clone, Copy)]
enum TrackAction {
    /// It was not recorded before, and now is.
    Added,
    /// It was recorded, and its content changed.
    Updated,
    /// The version it was at was replaced by a new one.
    Replaced,
}

impl TrackAction {
    /// Whether a committed file was new or an update, from what the Layout held for it.
    fn if_updated(file: &Prepared) -> Self {
        if file.data.is_some() {
            Self::Updated
        } else {
            Self::Added
        }
    }
}

/// One file, as `track` reports it.
struct TrackItem {
    /// Where it sits in the Layout.
    path: String,
    /// What happened to it.
    action: TrackAction,
}

/// Result: the named files were recorded.
#[derive(Grouped)]
pub struct ResultTrack {
    /// Each file, in the order it was named.
    items: Vec<TrackItem>,
}

#[renderer(buffer)]
pub fn render_result_track(result: ResultTrack) {
    for item in &result.items {
        let said = match item.action {
            TrackAction::Added => t!("track.result_added", path = item.path),
            TrackAction::Updated => t!("track.result_updated", path = item.path),
            TrackAction::Replaced => t!("track.result_replaced", path = item.path),
        };

        r_println!("{}", said.trim());
    }
}

/// Error: no file was named.
#[derive(Grouped)]
pub struct ErrorTrackNoFiles;

impl Failure for ErrorTrackNoFiles {
    fn name(&self) -> &'static str {
        "error_track_no_files"
    }

    fn reason(&self) -> String {
        t!("track.err_no_files").trim().to_string()
    }
}

failure!(ErrorTrackNoFiles);

#[renderer(buffer)]
pub fn render_error_track_no_files(_: ErrorTrackNoFiles, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("track.err_no_files").trim()));
    r_eprintln!("{}", help_line!(t!("track.err_no_files_help").trim()));
    ec.exit_code = EC_ERR_TRACK_ARGUMENT;
}

/// Error: the Layout names paths the tree does not hold, so the tree is not settled.
#[derive(Grouped)]
pub struct ErrorTrackLost {
    /// The paths the Layout names and the tree does not hold.
    paths: Vec<String>,
}

impl Failure for ErrorTrackLost {
    fn name(&self) -> &'static str {
        "error_track_lost"
    }

    fn reason(&self) -> String {
        t!(
            "track.err_lost",
            count = self.paths.len(),
            paths = self.paths.join(", ")
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorTrackLost);

#[renderer(buffer)]
pub fn render_error_track_lost(error: ErrorTrackLost, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_lost_help").trim()));
    ec.exit_code = EC_ERR_TRACK;
}

/// Error: the files named are ones the Vault's fetched copy says another account holds.
#[derive(Grouped)]
pub struct ErrorTrackUnowned {
    /// The paths that are not this account's to record.
    paths: Vec<String>,
}

impl Failure for ErrorTrackUnowned {
    fn name(&self) -> &'static str {
        "error_track_unowned"
    }

    fn reason(&self) -> String {
        t!(
            "track.err_unowned",
            count = self.paths.len(),
            paths = self.paths.join(", ")
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorTrackUnowned);

#[renderer(buffer)]
pub fn render_error_track_unowned(error: ErrorTrackUnowned, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_unowned_help").trim()));
    ec.exit_code = EC_ERR_TRACK_OWNERSHIP;
}

/// Error: a file named cannot be recorded.
#[derive(Grouped)]
pub struct ErrorTrackFile {
    /// What was named.
    path: String,
    /// What is wrong with it.
    kind: TrackFileError,
}

impl ErrorTrackFile {
    /// The failure of recording `path`, for the reason `kind` names.
    fn new(path: &str, kind: TrackFileError) -> Self {
        Self {
            path: path.to_owned(),
            kind,
        }
    }
}

/// The ways a named file cannot be recorded.
pub enum TrackFileError {
    /// It is not a file.
    NotAFile,
    /// It is not inside the Workspace.
    OutsideWorkspace,
    /// It is part of the Workspace's own data rather than its work.
    InsideData,
}

impl Failure for ErrorTrackFile {
    fn name(&self) -> &'static str {
        "error_track_file"
    }

    fn reason(&self) -> String {
        let said = match self.kind {
            TrackFileError::NotAFile => t!("track.err_not_a_file", path = self.path),
            TrackFileError::OutsideWorkspace => {
                t!("track.err_outside_workspace", path = self.path)
            }
            TrackFileError::InsideData => t!("track.err_inside_data", path = self.path),
        };

        said.trim().to_string()
    }
}

failure!(ErrorTrackFile);

#[renderer(buffer)]
pub fn render_error_track_file(error: ErrorTrackFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_file_help").trim()));
    ec.exit_code = EC_ERR_TRACK_ARGUMENT;
}

/// Error: the version `--replace` names cannot be replaced.
#[derive(Grouped)]
pub struct ErrorTrackReplace {
    /// The path named.
    path: String,
    /// Why its version cannot be replaced.
    kind: ReplaceError,
}

/// The ways a named file's version cannot be replaced.
pub enum ReplaceError {
    /// The Layout records no version for it.
    Unrecorded,
    /// The version is a merge, and replacing it would drop what was joined in.
    Merge,
    /// A variant is waiting to be joined into it, and an amend would drop that join.
    Merging,
}

impl Failure for ErrorTrackReplace {
    fn name(&self) -> &'static str {
        "error_track_replace"
    }

    fn reason(&self) -> String {
        let said = match self.kind {
            ReplaceError::Unrecorded => t!("track.err_replace_unrecorded", path = self.path),
            ReplaceError::Merge => t!("track.err_replace_merge", path = self.path),
            ReplaceError::Merging => t!("track.err_replace_merging", path = self.path),
        };

        said.trim().to_string()
    }
}

failure!(ErrorTrackReplace);

#[renderer(buffer)]
pub fn render_error_track_replace(error: ErrorTrackReplace, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_replace_help").trim()));
    ec.exit_code = EC_ERR_TRACK;
}

/// Error: a variant file no longer holds the variant's own content.
#[derive(Grouped)]
pub struct ErrorTrackMergeChanged {
    /// The variant file that changed.
    pub path: String,
}

impl Failure for ErrorTrackMergeChanged {
    fn name(&self) -> &'static str {
        "error_track_merge_changed"
    }

    fn reason(&self) -> String {
        t!("track.err_merge_changed", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorTrackMergeChanged);

#[renderer(buffer)]
pub fn render_error_track_merge_changed(error: ErrorTrackMergeChanged, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_merge_changed_help").trim()));
    ec.exit_code = EC_ERR_TRACK;
}

/// Error: the messages do not line up, or none could be worked out.
#[derive(Grouped)]
pub struct ErrorTrackMessage {
    /// What is wrong with the messages.
    kind: MessageError,
}

/// The ways the messages cannot be worked out.
pub enum MessageError {
    /// The per-file messages do not pair with the files.
    Mismatch,
    /// A file would carry no message at all.
    Missing {
        /// The file with nothing to say.
        path: String,
    },
    /// A file's message is longer than one may be.
    TooLong {
        /// The file whose message is too long.
        path: String,
        /// How long it is, in bytes.
        length: usize,
    },
    /// The editor was opened again and again without a message that may be kept.
    Unresolved,
}

impl Failure for ErrorTrackMessage {
    fn name(&self) -> &'static str {
        "error_track_message"
    }

    fn reason(&self) -> String {
        let said = match &self.kind {
            MessageError::Mismatch => t!("track.err_message_mismatch"),
            MessageError::Missing { path } => t!("track.err_message_missing", path = path),
            MessageError::TooLong { path, length } => {
                t!("track.err_message_too_long", path = path, length = length)
            }
            MessageError::Unresolved => t!("track.err_message_unresolved"),
        };

        said.trim().to_string()
    }
}

failure!(ErrorTrackMessage);

#[renderer(buffer)]
pub fn render_error_track_message(error: ErrorTrackMessage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_message_help").trim()));
    ec.exit_code = EC_ERR_TRACK_ARGUMENT;
}

/// Error: the editor was opened and nothing was written, so there is no message to record.
#[derive(Grouped)]
pub struct ErrorEditingAborted;

impl Failure for ErrorEditingAborted {
    fn name(&self) -> &'static str {
        "aborted"
    }

    fn reason(&self) -> String {
        t!("track.err_aborted").trim().to_string()
    }
}

failure!(ErrorEditingAborted);

#[renderer(buffer)]
pub fn render_error_editing_aborted(_: ErrorEditingAborted, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("track.err_aborted").trim()));
    r_eprintln!("{}", help_line!(t!("track.err_aborted_help").trim()));
    ec.exit_code = EC_ABORT;
}

/// Error: a store, an index or a Layout would not do what recording needs.
#[derive(Grouped)]
pub struct ErrorTrackFailed {
    /// Why it would not.
    cause: String,
}

impl ErrorTrackFailed {
    /// The failure naming `cause`.
    fn cause(cause: String) -> Self {
        Self { cause }
    }
}

impl Failure for ErrorTrackFailed {
    fn name(&self) -> &'static str {
        "error_track_failed"
    }

    fn reason(&self) -> String {
        t!("track.err_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTrackFailed);

#[renderer(buffer)]
pub fn render_error_track_failed(error: ErrorTrackFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TRACK;
}

/// Error: `--policy` names a chunking policy this build does not carry.
#[derive(Grouped)]
pub struct ErrorTrackPolicy {
    /// The name that was asked for.
    name: String,
}

impl Failure for ErrorTrackPolicy {
    fn name(&self) -> &'static str {
        "error_track_policy"
    }

    fn reason(&self) -> String {
        t!("track.err_policy_unknown", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorTrackPolicy);

#[renderer(buffer)]
pub fn render_error_track_policy(error: ErrorTrackPolicy, ec: &mut ResExitCode) {
    // The name is a mistake in the argument rather than a failure of the run, so the exit code is
    // the one every other argument mistake gets, and nothing was read or written before this.
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("track.err_policy_unknown_help", known = known_policies()).trim())
    );
    ec.exit_code = EC_ERR_TRACK_ARGUMENT;
}

/// Error: a file's recorded preference names a chunking policy this build does not carry.
///
/// It is a refusal rather than a fall back to the default cut: what a fall back would write is a
/// version cut another way than the file asks for, which deduplicates against nothing already stored
/// for it and says so nowhere.
#[derive(Grouped)]
pub struct ErrorTrackPreference {
    /// The path whose preference it is.
    path: String,
    /// The name the preference holds.
    name: String,
}

impl Failure for ErrorTrackPreference {
    fn name(&self) -> &'static str {
        "error_track_preference"
    }

    fn reason(&self) -> String {
        t!("track.err_preference", path = self.path, name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorTrackPreference);

#[renderer(buffer)]
pub fn render_error_track_preference(error: ErrorTrackPreference, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(
            t!(
                "track.err_preference_help",
                name = error.name,
                known = known_policies()
            )
            .trim()
        )
    );
    ec.exit_code = EC_ERR_TRACK;
}

/// The chunking policies this build carries, as one line a reader can act on.
fn known_policies() -> String {
    let names: Vec<&str> = librorolala::storage::names().collect();

    if names.is_empty() {
        return t!("track.err_policy_none").trim().to_string();
    }

    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::{assemble, parse_editing, section_name};

    #[test]
    fn a_message_is_the_group_the_file_or_the_two() {
        assert_eq!(assemble("group", "").as_deref(), Some("group"));
        assert_eq!(assemble("", "own").as_deref(), Some("own"));
        assert_eq!(assemble("group", "own").as_deref(), Some("group: own"));
    }

    #[test]
    fn a_bang_keeps_a_file_out_of_the_group() {
        assert_eq!(assemble("group", "! own").as_deref(), Some("own"));
        assert_eq!(assemble("", "! own").as_deref(), Some("own"));
    }

    #[test]
    fn nothing_at_all_is_no_message() {
        assert_eq!(assemble("", ""), None);
        assert_eq!(assemble("", "!"), None);
    }

    #[test]
    fn a_section_line_names_its_section() {
        assert_eq!(section_name(":: GLOBAL ::").as_deref(), Some("GLOBAL"));
        assert_eq!(section_name("  :: a-b ::  ").as_deref(), Some("a-b"));
        assert_eq!(section_name("# a comment"), None);
        assert_eq!(section_name("plain text"), None);
    }

    #[test]
    fn an_edited_file_reads_back_by_section_without_its_comments() {
        let edited =
            "# warning\n\n:: GLOBAL ::\n# hint\nthe group\n\n:: abc ::\n# hint\nthe file\n";
        let (global, per) = parse_editing(edited);

        assert_eq!(global.as_deref(), Some("the group"));
        assert_eq!(per.get("abc").map(String::as_str), Some("the file"));
    }

    #[test]
    fn a_section_left_empty_reads_back_empty() {
        let (global, per) = parse_editing(":: GLOBAL ::\nonly\n\n:: abc ::\n");

        assert_eq!(global.as_deref(), Some("only"));
        assert_eq!(per.get("abc").map(String::as_str), Some(""));
    }
}

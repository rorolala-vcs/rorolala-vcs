//! The `rola track` command: put files' content into the store and record it in the Layout.
//!
//! It is the whole of a piece of work rather than a step of it: a file is named, its content goes
//! into the store, a version is made of it, and the Layout being worked in is made to name that
//! version, so what a person says is a file, and what a Layout gains is everything a checkout of
//! it would need. A file already recorded is left alone while its content has not changed, and is
//! given a new version when it has.
//!
//! What is recorded about a change is a message, and a message covers a group: one for the whole run
//! and, when a file wants its own words, one per file. A run is not always able to say them on the
//! command line, so what it does not say is asked for in an editor. See [`crate::editor`].
//!
//! The order is read, ask, then write, and the library owns the reading and the writing: the plan
//! reads every file and tree that is to be recorded, the command asks for a message for the subjects
//! the plan found, and only then does the apply write the content and make the versions. So a run
//! whose editor is left empty has written nothing, and the editor lists the files that will actually
//! be recorded rather than a guess at them. What the run is doing while it does it is said through
//! [`crate::progress`], on a bar or as one record per line.

// The handler that records a file is one pass over everything the run is handed: the source, the
// store, the index and the report about them are moved through together, and the borrows `#[chain]`
// hands the handler would be scattered across helpers if that pass were cut up.
#![allow(clippy::too_many_lines)]
// A recording of one file needs everything about it, where it is, what it holds, the version it is
// at, the merge waiting for it and the move it confirms, and gathering them into one value would
// only move the count somewhere else.
#![allow(clippy::too_many_arguments)]
// The progress setting is the resource the injection hands the handler and not something a recording
// owns: taking a copy would read a state of its own, so it is taken by reference, which is what the
// injection gives it and what every other handler that watches a run does.
#![allow(clippy::trivially_copy_pass_by_ref)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use librorolala::write::WriteError;
use librorolala::write::track::batch::{
    Action as TrackedAction, Carry, FileKind, Messages, Options, Refusal, Subject,
};
use librorolala::write::track::batch::{apply, plan};
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
    ResForce, ResProgressSetting, ResStorageSet, ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::WORKSPACE_EDITING_PATH;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{filling_flag, is_flag, offer, strip_written, typing_flag};
use crate::editor::{ResEditor, open};
use crate::exit_codes::{
    EC_ABORT, EC_ERR_TRACK, EC_ERR_TRACK_ARGUMENT, EC_ERR_TRACK_OWNERSHIP, EC_HELP,
};
use crate::failure::failure;
use crate::layout::{
    ErrorLayoutExists, ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutShouldInWorkspace, chosen,
};
use crate::progress::Reporting;

/// The longest a message may be, in bytes.
///
/// It is the limit a `Message` object carries, kept here so that what a run is told before
/// anything is written is the same limit the object enforces.
const LONGEST_MESSAGE: usize = 256;

/// How many times the editor is opened again while what it holds is still too long.
///
/// The editor is the caller's, and there is no telling what one will do with a message that was
/// refused; a few attempts tell apart a caller fixing it from one that cannot, which is a run that
/// has to be stopped rather than reopened forever.
const EDIT_ATTEMPTS: usize = 10;

/// How long a recording goes unwatched before its bars are let out.
///
/// Shorter than the ordinary wait, see [`crate::progress`], because a recording opens with the part a
/// reader most wants to watch: the whole of every file it is to record is read before anything is
/// written, and a bar that appeared only once that reading was over would be a bar for nothing.
const WATCH_AFTER: std::time::Duration = std::time::Duration::from_secs(1);

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
                crate::ignore::ARG_INCLUDE_IGNORED: t!("ignore.complete.include_ignored"),
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
/// Each `FILE` is read first, to find whether what it holds differs from what the Layout names, the
/// files that changed and no others then go on to a message, a version and a Layout entry. A file
/// already recorded and unchanged is skipped, and one that did change is given a new version. A
/// `FILE` that is a directory is taken recursively: every file
/// under it is recorded, read the way the tree reading reads them, so what the Workspace keeps for
/// itself and what another Workspace holds are left out. A name is read against where the run was
/// made, so `.` is the directory it is in, and, at the Workspace root, the whole of the work.
///
/// `--message` says what the whole group did and is what each file carries; `--file-message` gives
/// one file its own words, one for each file named, paired with the files by position. A file whose
/// own message is empty carries the group's alone; one that begins with `!` carries its own alone,
/// without the group's. When neither says enough, an editor is opened.
///
/// `--replace` redoes the version each file is at rather than adding one after it, the way
/// `git commit --amend` redoes the last commit: the new version is built on what the version being
/// replaced was built on, so that version stands as if it had never been made. It is what a run
/// asks for when the version it made was wrong (the wrong content, or the wrong words) and the
/// file's content is what the tree holds now. A file the Layout records nothing for, and a version
/// that is a merge, are refused: neither is a version a new one can take the place of. The old
/// version and its variant stay in the index; only the Layout stops naming them. With `--replace`
/// and no `--message` or `--file-message`, the words the replaced version had are what the editor
/// opens on, and what `--no-editor` records as they stand.
///
/// A file the tree moved is recorded where it is now, and the move is confirmed along with it: what
/// a run asks for is a version. A move whose file was not edited is nothing to record, though, so
/// it is left alone, `rola align` is what confirms it.
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
/// holds, [`ErrorTrackIgnored`] when a file named is one the ignore rules govern,
/// [`ErrorNoEditor`](crate::editor::ErrorNoEditor) when an editor is needed and none is
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
        .pick(crate::ignore::ARG_INCLUDE_IGNORED)
        .pick_or_route(arg![Vec<String>], || ErrorTrackNoFiles.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (message, file_message, no_editor, replace, policy, include_ignored, files, unmatched) =
        match picked {
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
        include_ignored: matches!(include_ignored, Flag::Active),
    }
    .into()
}

/// The state a recording starts in.
// What a run does to the tree is a handful of independent answers the caller gave, no editor, a
// replaced version, another account's file, the ignore rules, and making each a two-variant enum
// would say no more than the bool does while writing every use of it out twice.
#[allow(clippy::struct_excessive_bools)]
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
    /// Whether a file the ignore rules govern may be named anyway.
    include_ignored: bool,
}

#[chain]
pub fn handle_track(
    state: StateTrack,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResStorageSet>,
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
    storage: &mut LazyRes<ResStorageSet>,
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
        include_ignored,
    } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };

    let Some(store) = storage.get_ref().storage() else {
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

    let root = held.get_root().to_path_buf();

    // The rules the Workspace ignores by govern what the tree gained: what a rule covers is not
    // this run's to record unless the run was told to include it.
    let rules = crate::ignore::rules(held, include_ignored);

    // An amend with no new words keeps the words the replaced version had, so the plan reads them
    // only when the run gave none of its own.
    let keeps_words = replace && message.is_none() && file_messages.is_empty();
    let options = Options {
        force,
        replace,
        include_ignored,
        keep_words: keeps_words,
        policy,
    };
    let messages = Messages {
        enumerating: t!("track.enumerating").to_string(),
        writing: t!("track.writing").to_string(),
        recording: t!("track.recording").to_string(),
    };
    let carry = Carry {
        workspace: held,
        layout: &layout,
        store,
        index: vcs,
        creator: &creator_name,
        rules: &rules,
        messages: &messages,
    };
    let cancellation = crate::cancel::on_ctrl_c();

    // Nothing is written before the plan is read: the editor is opened for the subjects the plan
    // found, and the apply records exactly those.
    let plan = match plan(&carry, &cwd, &files, options, progress, cancellation) {
        Ok(plan) => plan,
        Err(error) => return track_write_error(error),
    };

    if let Some(refusal) = plan.refused() {
        return refusal_state(refusal.clone());
    }

    if !file_messages.is_empty() && file_messages.len() != plan.subjects().len() {
        return message_error(MessageError::Mismatch);
    }

    let own = if file_messages.is_empty() {
        plan.prefill().to_vec()
    } else {
        file_messages
    };

    // Nothing changing is nothing to say: a run that found every file and tree already recorded
    // records nothing, and is not asked to explain a change it did not make.
    let texts = if plan.subjects().is_empty() {
        Vec::new()
    } else {
        match decide(plan.subjects(), &own, message, no_editor, editor, &root) {
            Ok(texts) => texts,
            Err(next) => return next,
        }
    };

    let outcome = match apply(&carry, plan, &texts, options, progress, cancellation) {
        Ok(outcome) => outcome,
        Err(error) => return track_write_error(error),
    };

    if let Some(refusal) = outcome.refused.first() {
        return refusal_state(refusal.clone());
    }

    ResultTrack {
        items: outcome
            .items
            .into_iter()
            .map(|item| TrackItem {
                path: item.path,
                action: match item.action {
                    TrackedAction::Added => TrackAction::Added,
                    TrackedAction::Updated => TrackAction::Updated,
                    TrackedAction::Replaced => TrackAction::Replaced,
                },
            })
            .collect(),
    }
    .into()
}

/// The failure a write the library refused is told as.
///
/// What a plan or an apply returns as a [`Refusal`] is mapped by [`refusal_state`]; what reaches here
/// is a write that could not be made, and a tree failure keeps the tree's own failure so a run that
/// named a tree is told about the tree.
fn track_write_error(error: WriteError) -> Next {
    match error {
        WriteError::NoStore => fail(t!("track.err_no_store").trim()),
        WriteError::NoIndex => fail(t!("track.err_no_index").trim()),
        WriteError::Tree { reason } => ErrorTrackTreeFailed::new(reason).into(),
        other => fail(other.to_string()),
    }
}

/// Turns one refusal into what the run is told.
fn refusal_state(refusal: Refusal) -> Next {
    match refusal {
        Refusal::File { path, kind } => ErrorTrackFile::new(
            &path,
            match kind {
                FileKind::NotAFile => TrackFileError::NotAFile,
                FileKind::OutsideWorkspace => TrackFileError::OutsideWorkspace,
                FileKind::InsideData => TrackFileError::InsideData,
            },
        )
        .into(),
        Refusal::Ignored { paths } => ErrorTrackIgnored { paths }.into(),
        Refusal::Preference { path, name } => ErrorTrackPreference { path, name }.into(),
        Refusal::Lost { paths } => ErrorTrackLost { paths }.into(),
        Refusal::Unowned { paths } => ErrorTrackUnowned { paths }.into(),
        Refusal::Unrecorded { path } => ErrorTrackReplace {
            path,
            kind: ReplaceError::Unrecorded,
        }
        .into(),
        Refusal::Merge { path } => ErrorTrackReplace {
            path,
            kind: ReplaceError::Merge,
        }
        .into(),
        Refusal::Merging { path } => ErrorTrackReplace {
            path,
            kind: ReplaceError::Merging,
        }
        .into(),
        Refusal::MergeChanged { path } => ErrorTrackMergeChanged { path }.into(),
        Refusal::TreeUnsupported => ErrorTrackTreeUnsupported.into(),
        Refusal::TreeConflict { path } => ErrorTrackTreeConflict { path }.into(),
        Refusal::LayoutExists => ErrorLayoutExists.into(),
        Refusal::LayoutMissing => ErrorLayoutMissing.into(),
        Refusal::LayoutFailed { cause } => ErrorLayoutFailed::new(cause).into(),
    }
}

/// Works out what each subject's message is, asking an editor when it was not said.
///
/// A subject whose own message was given is paired with it here; one that was not carries the
/// group's alone. What none of that answers is asked of the editor, which is the only way left.
fn decide(
    subjects: &[Subject],
    own: &[String],
    message: Option<String>,
    no_editor: bool,
    editor: &ResEditor,
    root: &Path,
) -> Result<Vec<String>, Next> {
    if let Some(global) = message {
        return assemble_all(subjects, &global, own);
    }

    if no_editor {
        return assemble_all(subjects, "", own);
    }

    edit_messages(subjects, own, editor, root)
}

/// Splices a group message and each subject's own message into the message that subject carries.
fn assemble_all(subjects: &[Subject], global: &str, own: &[String]) -> Result<Vec<String>, Next> {
    let mut messages = Vec::with_capacity(subjects.len());

    for (subject, own) in subjects.iter().zip(own) {
        let Some(spliced) = assemble(global, own) else {
            return Err(message_error(MessageError::Missing {
                path: subject.path.clone(),
            }));
        };

        if spliced.len() > LONGEST_MESSAGE {
            return Err(message_error(MessageError::TooLong {
                path: subject.path.clone(),
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
/// is the second shape, it says the group does not apply here, which is how a run says in one
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
    subjects: &[Subject],
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
        if let Err(error) = fs::write(&path, template(subjects, &prefill, &warnings)) {
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
            && subjects.iter().all(|subject| {
                per.get(&subject.key)
                    .is_none_or(|own| own.trim().is_empty())
            });
        if wrote_nothing {
            return Err(ErrorEditingAborted.into());
        }

        let mut messages = Vec::with_capacity(subjects.len());
        let mut refused = Vec::new();

        for subject in subjects {
            let own = per.get(&subject.key).map_or("", String::as_str);

            let Some(spliced) = assemble(&global, own) else {
                refused.push(
                    t!("track.editing_missing", path = subject.path)
                        .trim()
                        .to_string(),
                );
                continue;
            };

            if spliced.len() > LONGEST_MESSAGE {
                refused.push(
                    t!(
                        "track.editing_warning",
                        path = subject.path,
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
        prefill = subjects
            .iter()
            .map(|subject| per.get(&subject.key).cloned().unwrap_or_default())
            .collect();
    }

    Err(message_error(MessageError::Unresolved))
}

/// The content an editor is opened on: the group's section and one per subject.
fn template(subjects: &[Subject], prefill: &[String], warnings: &[String]) -> String {
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

    for (at, subject) in subjects.iter().enumerate() {
        let own = prefill.get(at).map_or("", String::as_str);
        let _ = writeln!(
            text,
            ":: {} ::\n# {}: `{}`\n{own}\n",
            subject.key,
            t!("track.editing_file").trim(),
            subject.path
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

/// Error: the files named are ones the ignore rules govern, so they cannot be selected.
#[derive(Grouped)]
pub struct ErrorTrackIgnored {
    /// What was named, as it was written.
    paths: Vec<String>,
}

impl Failure for ErrorTrackIgnored {
    fn name(&self) -> &'static str {
        "error_track_ignored"
    }

    fn reason(&self) -> String {
        t!(
            "track.err_ignored",
            count = self.paths.len(),
            paths = self.paths.join(", ")
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorTrackIgnored);

#[renderer(buffer)]
pub fn render_error_track_ignored(error: ErrorTrackIgnored, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_ignored_help").trim()));
    ec.exit_code = EC_ERR_TRACK_ARGUMENT;
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

/// Error: a tree cannot be recorded with the flags this run gave.
#[derive(Grouped)]
pub struct ErrorTrackTreeUnsupported;

impl Failure for ErrorTrackTreeUnsupported {
    fn name(&self) -> &'static str {
        "error_track_tree_unsupported"
    }

    fn reason(&self) -> String {
        t!("track.err_tree_unsupported").trim().to_string()
    }
}

failure!(ErrorTrackTreeUnsupported);

#[renderer(buffer)]
pub fn render_error_track_tree_unsupported(_: ErrorTrackTreeUnsupported, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("track.err_tree_unsupported").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("track.err_tree_unsupported_help").trim())
    );
    ec.exit_code = EC_ERR_TRACK_ARGUMENT;
}

/// Error: a tree could not be recorded.
#[derive(Grouped)]
pub struct ErrorTrackTreeFailed {
    /// What happened.
    cause: String,
}

impl ErrorTrackTreeFailed {
    /// The failure of recording a tree, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTrackTreeFailed {
    fn name(&self) -> &'static str {
        "error_track_tree_failed"
    }

    fn reason(&self) -> String {
        t!("track.err_tree_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTrackTreeFailed);

#[renderer(buffer)]
pub fn render_error_track_tree_failed(error: ErrorTrackTreeFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_tree_failed_help").trim()));
    ec.exit_code = EC_ERR_TRACK;
}

/// Error: a tree has a conflict that was never settled.
#[derive(Grouped)]
pub struct ErrorTrackTreeConflict {
    /// The tree.
    path: String,
}

impl Failure for ErrorTrackTreeConflict {
    fn name(&self) -> &'static str {
        "error_track_tree_conflict"
    }

    fn reason(&self) -> String {
        t!("track.err_tree_conflict", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorTrackTreeConflict);

#[renderer(buffer)]
pub fn render_error_track_tree_conflict(error: ErrorTrackTreeConflict, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("track.err_tree_conflict_help").trim()));
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

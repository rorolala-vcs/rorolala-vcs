//! The `rola undo` command: put back what the last recorded operations changed.
//!
//! A run pops the newest operations off the stack `rola opslog` lists and reverses them, newest
//! first. What each operation left is compared with the tree as it is now: a run whose every change
//! still stands is reversed at once, one that stands only in part asks before it goes on, and one
//! that stands nowhere stops the run and leaves the stack below it alone.
//!
//! A file the operation wrote is put back from the version the entry recorded before it, so a
//! content the store does not hold is one this cannot answer: a file that changed after the
//! operation is refused rather than written over, unless the run says `--force`, and `--soft` leaves
//! every file as it is and moves only the Layout.

// The confirmation is the resource the injection hands the handler and not something an undo owns:
// taking a copy would read a state of its own, so it is taken by reference, which is what the
// injection gives it and what every other handler that asks a question does.
#![allow(clippy::trivially_copy_pass_by_ref)]

use std::fs;
use std::path::{Path, PathBuf};

use librorolala::layout::{EntryKind, Layout, LayoutPath, MutableData};
use librorolala::storage::{Key, StorageSet, key_of_file};
use librorolala::vcs::VCSIndex;
use librorolala::write::{Effect, EntryState, Journal, LayoutOp, apply_recorded, switch_entry};
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    confirm::YesConfirm,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::{ResConfirm, ResExitCode},
};
use rorolala_cli_setups::{ResForce, ResStorageSet, ResVCSIndex, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate as _;
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_TWEAK, EC_HELP};
use crate::failure::failure;
use crate::layout::ErrorLayoutNoCurrent;

/// How many operations to undo, newest first.
const COUNT: PickerArg<'static, Option<String>> = arg![Option<String>];

/// Move only the record back, leaving the work on disk as it is.
const ARG_SOFT: PickerArg<'static, Flag> = arg![soft: Flag];

#[help(buffer)]
pub fn help_undo(_: EntryUndo, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("undo.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryUndo)]
pub fn desc_undo() -> Description {
    t!("undo.description").to_string().into()
}

/// Completes what `rola undo` can be given next.
///
/// The count is a number this program cannot know, and the only word worth offering is the flag.
#[completion(EntryUndo)]
pub fn complete_undo(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_SOFT: t!("undo.complete.soft"),
            },
        );
    }

    suggest!()
}

/// Undoes the last recorded operations, newest first
///
/// `COUNT` is how many recorded operations to undo, from the newest, and defaults to one. An
/// operation whose every change still stands is undone at once; one that stands only in part asks
/// whether to go on, and one that stands nowhere stops the run and leaves the stack below it.
///
/// `--force` continues past a file that changed after the operation and writes over it, and
/// `--soft` moves only the Layout back and leaves every file as it is.
///
/// # Errors
///
/// Renders [`ErrorUndoNoLog`] when the Workspace records nothing, [`ErrorUndoCount`] when `COUNT` is
/// not a number from one to the number of recorded operations, [`ErrorUndoFlags`] when `--force` and
/// `--soft` are given together, [`ErrorUndoChanged`] when a file changed after the operation,
/// [`ErrorUndoAbsent`] when a file the operation left is gone, [`ErrorUndoFailed`] when the Layout or
/// the store refuses, [`ErrorLayoutShouldInWorkspace`](crate::layout::ErrorLayoutShouldInWorkspace)
/// when the run is not inside a Workspace, and
/// [`ErrorLayoutNoCurrent`] when the Workspace has no Layout to move back.
#[command(node = "undo", entry = EntryUndo)]
pub fn undo(args: EntryUndo, force: &ResForce) -> Next {
    let picked = args.pick(COUNT).pick(ARG_SOFT).pick(&UNMATCHED).to_result();
    let (count, soft, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "undo") {
        return next;
    }

    StateUndo {
        count,
        soft: matches!(soft, Flag::Active),
        force: **force,
    }
    .into()
}

/// The state an undo starts in.
#[derive(Grouped)]
pub struct StateUndo {
    /// How many operations were named, as they were written.
    count: Option<String>,
    /// Whether only the record is moved back.
    soft: bool,
    /// Whether a file that changed is written over.
    force: bool,
}

#[chain(routeify)]
pub fn handle_undo(
    state: StateUndo,
    workspace: &mut LazyRes<ResWorkspace>,
    storage: &mut LazyRes<ResStorageSet>,
    index: &mut LazyRes<ResVCSIndex>,
    confirm: &ResConfirm,
) -> Next {
    let StateUndo {
        count: named,
        soft,
        force,
    } = state;

    // The two ask for different undos: one writes over a file, the other writes none at all.
    if force && soft {
        return ErrorUndoFlags.into();
    }

    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to read.
    let held = workspace.get_ref().as_ref().unwrap();

    let layout = match held.layout_if_there() {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutNoCurrent.into(),
        Err(error) => return failed(error.to_string()),
    };

    let files = match Journal::files(held) {
        Ok(files) => files,
        Err(error) => return failed(error.to_string()),
    };

    if files.is_empty() {
        return ErrorUndoNoLog.into();
    }

    // The files are named by the moment each run ended, so the stack is read backwards: the last
    // name is the newest operation.
    let stack: Vec<&PathBuf> = files.iter().rev().collect();

    // A count is read against what is recorded, so a number past the stack is a number the caller
    // read somewhere else.
    let count = match named.as_deref().map(str::trim) {
        None | Some("") => 1,
        Some(written) => match written.parse::<usize>() {
            Ok(number) if number >= 1 && number <= stack.len() => number,
            _ => {
                return ErrorUndoCount {
                    count: written.to_owned(),
                    available: stack.len(),
                }
                .into();
            }
        },
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return failed(error.to_string()),
    };
    let keeper = Keeper {
        layout: &layout,
        vcs: index.get_ref().as_ref(),
        store: storage.get_ref().storage(),
        root: held.get_root().to_path_buf(),
        runtime,
    };
    let mut undone: Vec<String> = Vec::new();

    for path in stack.into_iter().take(count) {
        let group = match Journal::read(path) {
            Ok(group) => group,
            Err(error) => return failed(error.to_string()),
        };

        let (chosen, all) = match plan(&keeper, &group.effects, soft, force) {
            Ok(plan) => plan,
            Err(next) => return next,
        };

        if chosen.is_empty() {
            break;
        }

        // A run that stands only in part says so before it moves anything: what it can put back is
        // not what the operation did.
        if !all && !confirm.ask::<YesConfirm>(&format!("{} [y/n] ", t!("undo.partial").trim())) {
            break;
        }

        if let Err(reason) = reverse(&keeper, &chosen, soft) {
            return ErrorUndoFailed {
                command: group.command.clone(),
                reason,
            }
            .into();
        }

        if let Err(error) = fs::remove_file(path) {
            return ErrorUndoFailed {
                command: group.command.clone(),
                reason: error.to_string(),
            }
            .into();
        }

        undone.push(group.command);
    }

    match undone.len() {
        0 => ResultUndo::none().into(),
        1 => ResultUndoOne {
            command: undone.remove(0),
        }
        .into(),
        count => ResultUndo {
            count,
            commands: undone.join(", "),
        }
        .into(),
    }
}

/// What an undo reads the recorded versions and files through.
struct Keeper<'a> {
    /// The Layout being moved back.
    layout: &'a Layout,
    /// The index the recorded versions are read from, when there is one.
    vcs: Option<&'a VCSIndex>,
    /// The store the content is put back from, when there is one.
    store: Option<&'a StorageSet>,
    /// The root of the Workspace, which a relative path is read against.
    root: PathBuf,
    /// The run the index is read on.
    runtime: tokio::runtime::Runtime,
}

impl Keeper<'_> {
    /// The content key the version `version` holds, when the index still names it.
    ///
    /// A version names a variant, and the variant names the content: the entry records a version,
    /// which is not a content key, so the two are told apart through the index.
    fn content_of_version(&self, version: &str) -> Option<String> {
        let vcs = self.vcs?;
        let hash = parse_hash(version)?;
        let version = self
            .runtime
            .block_on(vcs.read(Key::new(hash)))
            .ok()?
            .expect_version()
            .ok()?;
        let variant = self
            .runtime
            .block_on(vcs.read(Key::new(*version.variant())))
            .ok()?
            .expect_variant()
            .ok()?;

        Some(Key::new(*variant.storage_hash()).hex())
    }

    /// The whole of a path in the journal, which is relative to the Workspace's root.
    fn at(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);

        if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        }
    }
}

/// How one recorded change stands against the tree as it is now.
enum Standing {
    /// The tree is as the change left it.
    Matched,
    /// The tree is not as the change left it.
    Changed,
    /// Something the change left is gone.
    Absent,
}

/// The changes one operation can be reversed by, and whether all of them still stand.
///
/// A `File` or `FileMoved` change is left out under `--soft`, since only the record moves back. A
/// file that changed after the operation is refused here rather than written over: it is the one
/// thing this run cannot put back, and the caller is told how to say it is what they want.
fn plan<'a>(
    keeper: &Keeper<'_>,
    effects: &'a [Effect],
    soft: bool,
    force: bool,
) -> Result<(Vec<&'a Effect>, bool), Next> {
    let mut chosen = Vec::new();
    let mut all = true;

    for effect in effects {
        if soft && mentions_file(effect) {
            continue;
        }

        match standing(keeper, effect) {
            Standing::Matched => chosen.push(effect),
            Standing::Changed => {
                if let Some(path) = touched_file(effect) {
                    if !force {
                        return Err(ErrorUndoChanged { path }.into());
                    }

                    chosen.push(effect);
                } else {
                    all = false;
                }
            }
            Standing::Absent => {
                let Some(path) = touched_file(effect) else {
                    all = false;

                    continue;
                };

                if !force {
                    return Err(ErrorUndoAbsent { path }.into());
                }
            }
        }
    }

    Ok((chosen, all))
}

/// Whether a change is about a file on disk rather than the Layout alone.
fn mentions_file(effect: &Effect) -> bool {
    matches!(effect, Effect::File { .. } | Effect::FileMoved { .. })
        || matches!(
            effect,
            Effect::Entry {
                after: Some(state),
                ..
            } if state.kind == "file"
        )
}

/// The path a change wrote, when it wrote one.
fn touched_file(effect: &Effect) -> Option<String> {
    match effect {
        Effect::File { path, .. } => Some(path.clone()),
        Effect::FileMoved { to, .. } => Some(to.clone()),
        Effect::Entry { path, .. } => path.clone(),
        _ => None,
    }
}

/// Whether `path` names `id`.
fn names(layout: &Layout, path: Option<&str>, id: uuid::Uuid) -> bool {
    path.and_then(|path| LayoutPath::new(path).ok())
        .is_some_and(|path| layout.id_of(&path) == Some(id))
}

/// How `effect` stands against the tree.
fn standing(keeper: &Keeper<'_>, effect: &Effect) -> Standing {
    let layout = keeper.layout;

    match effect {
        Effect::Entry {
            id, path, after, ..
        } => {
            // The id may have moved since the change, as a cut moves it, so the entry is read at the
            // path the change recorded and falls back to the id when the path names nothing.
            let named = path
                .as_deref()
                .and_then(|path| LayoutPath::new(path).ok())
                .and_then(|path| layout.id_of(&path));

            if entry_state(layout, named.unwrap_or(*id)) != *after {
                return Standing::Changed;
            }

            let Some(after) = after else {
                return Standing::Matched;
            };

            // A tree is one material, and its files are not what the Layout holds, so only a file
            // entry has a file on disk to compare.
            let Some(path) = path else {
                return Standing::Matched;
            };

            if after.kind != "file" {
                return Standing::Matched;
            }

            // A version the index no longer names is one this cannot compare against, and the
            // inverse leaves the file alone rather than refusing the run over it.
            let Some(expected) = keeper.content_of_version(&after.version) else {
                return Standing::Matched;
            };

            match content_of(&keeper.at(path)) {
                Some(content) if content == expected => Standing::Matched,
                Some(_) => Standing::Changed,
                None => Standing::Absent,
            }
        }
        Effect::Path { id, from, to } => {
            let matched = to.as_ref().map_or_else(
                || !names(layout, from.as_deref(), *id),
                |to| names(layout, Some(to), *id),
            );

            if matched {
                Standing::Matched
            } else {
                Standing::Changed
            }
        }
        Effect::Uuid {
            from,
            to,
            path,
            after,
            ..
        } => {
            let at_to = path
                .as_deref()
                .and_then(|path| LayoutPath::new(path).ok())
                .is_some_and(|path| layout.id_of(&path) == Some(*to));

            if at_to
                && entry_state(layout, *to).as_ref() == Some(after)
                && layout.entry(*from).is_none()
            {
                Standing::Matched
            } else {
                Standing::Changed
            }
        }
        Effect::File { path, after, .. } => match content_of(&PathBuf::from(path)) {
            Some(content) if Some(&content) == after.as_ref() => Standing::Matched,
            Some(_) => Standing::Changed,
            None if after.is_some() => Standing::Absent,
            None => Standing::Matched,
        },
        Effect::FileMoved { from, to } => {
            let from = PathBuf::from(from);
            let to = PathBuf::from(to);

            if from.exists() {
                Standing::Changed
            } else if to.exists() {
                Standing::Matched
            } else {
                Standing::Absent
            }
        }
    }
}

/// Reverses `effects`, newest change first.
///
/// Each change is written as it is reversed, so what the next one reads is the Layout the one before
/// it left: a change that moves an entry to another `Uuid` may be reversed before the change that
/// put what the entry held, and the two would collide if they were one batch read from one state.
/// The files follow, put back from the version before them, or taken away when one was made.
fn reverse(keeper: &Keeper<'_>, effects: &[&Effect], soft: bool) -> Result<(), String> {
    let layout = keeper.layout;
    let mut files = Vec::new();

    for effect in effects.iter().rev() {
        match effect {
            Effect::Entry {
                id, path, before, ..
            } => undo_entry(
                keeper,
                *id,
                path.as_deref(),
                before.as_ref(),
                soft,
                &mut files,
            )?,
            Effect::Path { id, from, to } => match (from, to) {
                (Some(from), Some(to)) => {
                    if let (Ok(from), Ok(to)) = (LayoutPath::new(from), LayoutPath::new(to))
                        && layout.id_of(&to) == Some(*id)
                        && layout.id_of(&from).is_none()
                    {
                        write(layout, LayoutOp::MovePath { from: to, to: from })?;
                    }
                }
                (Some(from), None) => {
                    if let Ok(from) = LayoutPath::new(from)
                        && layout.id_of(&from).is_none()
                        && layout.entry(*id).is_some()
                    {
                        write(
                            layout,
                            LayoutOp::CreatePath {
                                path: from,
                                id: *id,
                            },
                        )?;
                    }
                }
                (None, Some(to)) => {
                    if let Ok(to) = LayoutPath::new(to)
                        && layout.id_of(&to) == Some(*id)
                    {
                        write(layout, LayoutOp::RemovePath { path: to })?;
                    }
                }
                (None, None) => {}
            },
            Effect::Uuid {
                from,
                to,
                path,
                before,
                ..
            } => {
                let at_to = path
                    .as_deref()
                    .and_then(|path| LayoutPath::new(path).ok())
                    .filter(|path| layout.id_of(path) == Some(*to));

                if let Some(path) = at_to
                    && layout.entry(*from).is_none()
                {
                    switch_entry(layout, &path, *from, data_of(before)?)
                        .map_err(|error| error.to_string())?;
                }
            }
            Effect::File { path, before, .. } => {
                let path = PathBuf::from(path);

                match before {
                    Some(content) => files.push(FileAction::PutBack {
                        path,
                        content: content.clone(),
                    }),
                    None => files.push(FileAction::Delete { path }),
                }
            }
            Effect::FileMoved { from, to } => files.push(FileAction::MoveBack {
                from: PathBuf::from(to),
                to: PathBuf::from(from),
            }),
        }
    }

    for action in files {
        action.make(keeper.store)?;
    }

    Ok(())
}

/// Puts an entry back to the state `before` holds, and the file to the content of its version.
fn undo_entry(
    keeper: &Keeper<'_>,
    id: uuid::Uuid,
    path: Option<&str>,
    before: Option<&EntryState>,
    soft: bool,
    files: &mut Vec<FileAction>,
) -> Result<(), String> {
    let layout = keeper.layout;

    match before {
        Some(before) => {
            let data = data_of(before)?;

            if layout.entry(id).is_some() {
                write(layout, LayoutOp::UpdateEntry { id, data })?;
            } else {
                let free = path
                    .and_then(|path| LayoutPath::new(path).ok())
                    .filter(|path| layout.id_of(path).is_none());

                write(
                    layout,
                    LayoutOp::CreateEntry {
                        id,
                        path: free,
                        data,
                    },
                )?;
            }

            // The file the entry stood for, put back from the version it held before, unless the run
            // said the disk is to be left alone.
            if !soft
                && let Some(path) = path
                && before.kind == "file"
                && let Some(content) = keeper.content_of_version(&before.version)
            {
                files.push(FileAction::PutBack {
                    path: keeper.at(path),
                    content,
                });
            }
        }
        None => {
            if layout.entry(id).is_some() {
                write(layout, LayoutOp::RemoveEntry { id })?;
            }
        }
    }

    Ok(())
}

/// Writes one change to the Layout the run is reading.
fn write(layout: &Layout, op: LayoutOp) -> Result<(), String> {
    apply_recorded(layout, &[op], None).map_err(|error| error.to_string())
}

/// What reversing one change does to a file on disk.
enum FileAction {
    /// Puts content back at a path.
    PutBack {
        /// Where the content goes.
        path: PathBuf,
        /// The key of the content, as hex.
        content: String,
    },
    /// Takes a file the operation made away.
    Delete {
        /// The file.
        path: PathBuf,
    },
    /// Puts a file back where it was before it was moved.
    MoveBack {
        /// Where the file is now.
        from: PathBuf,
        /// Where it was.
        to: PathBuf,
    },
}

impl FileAction {
    /// Carries the action out.
    fn make(self, store: Option<&StorageSet>) -> Result<(), String> {
        match self {
            Self::PutBack { path, content } => {
                let Some(store) = store else {
                    return Ok(());
                };

                let hash = parse_hash(&content).ok_or_else(|| {
                    t!("undo.err_failed", command = "undo", reason = content)
                        .trim()
                        .to_owned()
                })?;

                if let Some(parent) = path.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(error.to_string());
                }

                let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;

                runtime
                    .block_on(store.extract_file(&Key::new(hash), &path))
                    .map_err(|error| error.to_string())
            }
            Self::Delete { path } => match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            },
            Self::MoveBack { from, to } => {
                if !from.exists() {
                    return Ok(());
                }

                if to.exists() {
                    return Err(t!("undo.err_changed", path = to.display().to_string())
                        .trim()
                        .to_owned());
                }

                if let Some(parent) = to.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(error.to_string());
                }

                fs::rename(&from, &to).map_err(|error| error.to_string())
            }
        }
    }
}

/// The entry `id` holds, in the shape the journal keeps it.
fn entry_state(layout: &Layout, id: uuid::Uuid) -> Option<EntryState> {
    layout.entry(id).map(|data| EntryState::of(&data))
}

/// What a journalled entry state holds, as a Layout entry.
fn data_of(state: &EntryState) -> Result<MutableData, String> {
    let version = parse_hash(&state.version).ok_or_else(|| {
        t!(
            "undo.err_failed",
            command = "undo",
            reason = state.version.clone()
        )
        .trim()
        .to_owned()
    })?;
    let kind = if state.kind == "tree" {
        EntryKind::Tree
    } else {
        EntryKind::File
    };

    Ok(
        MutableData::new(state.owner.clone(), version, state.description.clone())
            .with_kind(kind)
            .with_preference(state.preference.clone()),
    )
}

/// The content key of `path`, or nothing when there is no file there.
fn content_of(path: &Path) -> Option<String> {
    key_of_file(path).ok().map(|key| key.hex())
}

/// The thirty-two bytes a sixty-four character hex string stands for.
fn parse_hash(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }

    let mut hash = [0_u8; 32];
    for (at, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let pair = std::str::from_utf8(pair).ok()?;
        hash[at] = u8::from_str_radix(pair, 16).ok()?;
    }

    Some(hash)
}

/// Result: one operation was undone.
#[derive(Grouped)]
pub struct ResultUndoOne {
    /// The command that made it.
    command: String,
}

#[renderer(buffer)]
pub fn render_result_undo_one(result: ResultUndoOne, ec: &mut ResExitCode) {
    r_println!("{}", t!("undo.result_one", command = result.command).trim());
    let _ = ec;
}

/// Result: several operations were undone, or none was.
#[derive(Grouped)]
pub struct ResultUndo {
    /// How many were undone.
    count: usize,
    /// The commands that made them, oldest first.
    commands: String,
}

impl ResultUndo {
    /// The result of a run that found nothing it could undo.
    fn none() -> Self {
        Self {
            count: 0,
            commands: String::new(),
        }
    }
}

#[renderer(buffer)]
pub fn render_result_undo(result: ResultUndo, ec: &mut ResExitCode) {
    // A run that found nothing it could undo leaves the stack as it was and says nothing: the
    // operations are still there for `rola opslog` to name.
    if result.count > 0 {
        r_println!(
            "{}",
            t!(
                "undo.result",
                count = result.count,
                commands = result.commands
            )
            .trim()
        );
    }

    let _ = ec;
}

/// Error: the Workspace records no operation.
#[derive(Grouped)]
pub struct ErrorUndoNoLog;

impl Failure for ErrorUndoNoLog {
    fn name(&self) -> &'static str {
        "error_undo_no_log"
    }

    fn reason(&self) -> String {
        t!("undo.err_no_log").trim().to_string()
    }
}

failure!(ErrorUndoNoLog);

#[renderer(buffer)]
pub fn render_error_undo_no_log(_: ErrorUndoNoLog, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("undo.err_no_log").trim()));
    r_eprintln!("{}", help_line!(t!("undo.err_no_log_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the count names no operation.
#[derive(Grouped)]
pub struct ErrorUndoCount {
    /// The number as it was written.
    count: String,
    /// How many operations are recorded.
    available: usize,
}

impl Failure for ErrorUndoCount {
    fn name(&self) -> &'static str {
        "error_undo_count"
    }

    fn reason(&self) -> String {
        t!(
            "undo.err_count",
            count = self.count,
            available = self.available
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorUndoCount);

#[renderer(buffer)]
pub fn render_error_undo_count(error: ErrorUndoCount, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_count_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a file changed after the operation was recorded.
#[derive(Grouped)]
pub struct ErrorUndoChanged {
    /// The file.
    path: String,
}

impl Failure for ErrorUndoChanged {
    fn name(&self) -> &'static str {
        "error_undo_changed"
    }

    fn reason(&self) -> String {
        t!("undo.err_changed", path = self.path).trim().to_string()
    }
}

failure!(ErrorUndoChanged);

#[renderer(buffer)]
pub fn render_error_undo_changed(error: ErrorUndoChanged, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_changed_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: a file the operation left is gone.
#[derive(Grouped)]
pub struct ErrorUndoAbsent {
    /// The file.
    path: String,
}

impl Failure for ErrorUndoAbsent {
    fn name(&self) -> &'static str {
        "error_undo_absent"
    }

    fn reason(&self) -> String {
        t!("undo.err_absent", path = self.path).trim().to_string()
    }
}

failure!(ErrorUndoAbsent);

#[renderer(buffer)]
pub fn render_error_undo_absent(error: ErrorUndoAbsent, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_absent_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: `--force` and `--soft` were given together.
#[derive(Grouped)]
pub struct ErrorUndoFlags;

impl Failure for ErrorUndoFlags {
    fn name(&self) -> &'static str {
        "error_undo_flags"
    }

    fn reason(&self) -> String {
        t!("undo.err_flags").trim().to_string()
    }
}

failure!(ErrorUndoFlags);

#[renderer(buffer)]
pub fn render_error_undo_flags(_: ErrorUndoFlags, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("undo.err_flags").trim()));
    r_eprintln!("{}", help_line!(t!("undo.err_flags_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// Error: the undo could not be carried out.
#[derive(Grouped)]
pub struct ErrorUndoFailed {
    /// The command whose operation failed.
    command: String,
    /// What failed.
    reason: String,
}

impl Failure for ErrorUndoFailed {
    fn name(&self) -> &'static str {
        "error_undo_failed"
    }

    fn reason(&self) -> String {
        t!(
            "undo.err_failed",
            command = self.command,
            reason = self.reason
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorUndoFailed);

#[renderer(buffer)]
pub fn render_error_undo_failed(error: ErrorUndoFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("undo.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TWEAK;
}

/// The failure an undo that could not be read is told as.
fn failed(reason: String) -> Next {
    ErrorUndoFailed {
        command: "undo".to_owned(),
        reason,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::{EntryState, data_of, parse_hash};

    #[test]
    fn a_version_reads_back_from_its_hex() {
        let hash = parse_hash("0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20")
            .expect("a hash of sixty-four digits");

        assert_eq!(hash[0], 1);
        assert_eq!(hash[31], 0x20);
        assert!(parse_hash("0102").is_none());
        assert!(parse_hash(&"z".repeat(64)).is_none());
    }

    #[test]
    fn an_entry_state_reads_back_as_the_entry_it_was() {
        let state = EntryState {
            kind: "file".to_owned(),
            owner: Some("bob".to_owned()),
            version: "ab".repeat(32),
            description: "said".to_owned(),
            preference: Some("psd".to_owned()),
        };
        let data = data_of(&state).expect("an entry state holds a hash");

        assert_eq!(EntryState::of(&data), state);
    }
}

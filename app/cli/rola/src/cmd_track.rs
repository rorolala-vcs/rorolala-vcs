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

#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::storage::{Key, RorolalaStorage, store_file};
use librorolala::tree_analyze::{Cache, PathRename, cache_path, entry_of, tree_diff, walk};
use librorolala::vcs::{Creator, Message, VCSIndex, Version};
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::WORKSPACE_EDITING_PATH;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::editor::{ResEditor, open};
use crate::exit_codes::{EC_ABORT, EC_ERR_TRACK, EC_ERR_TRACK_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::layout::{ErrorLayoutShouldInWorkspace, chosen, failed};

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

/// The flags `rola track` takes.
#[derive(Pickable)]
struct TrackFlags {
    /// What the whole group did; the message every file carries unless it is given its own.
    #[arg(long)]
    message: Option<String>,
    /// What one file did, one for each file named, in the order they are named.
    #[arg(long)]
    file_message: Vec<String>,
    /// Record what is already known, and never open an editor.
    #[arg(long)]
    no_editor: Flag,
}

#[help(buffer)]
pub fn help_track(_: EntryTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("track.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTrack)]
pub fn desc_track() -> Description {
    t!("track.description").to_string().into()
}

/// Records one or more files in the Layout being worked in
///
/// Each `FILE` has its content put into the store and a version made of it, and the Layout is made
/// to name that version. A file already recorded is skipped while its content has not changed, and
/// given a new version when it has. A `FILE` that is a directory is taken recursively: every file
/// under it is recorded, read the way the tree reading reads them, so what the Workspace keeps for
/// itself and what another Workspace holds are left out. A name is read against where the run was
/// made, so `.` is the directory it is in — and, at the Workspace root, the whole of the work.
///
/// `--message` says what the whole group did and is what each file carries; `--file-message` gives
/// one file its own words, one for each file named, paired with the files by position. A file whose
/// own message is empty carries the group's alone; one that begins with `!` carries its own alone,
/// without the group's. When neither says enough, an editor is opened.
///
/// A file the tree moved is recorded where it is now, and the move is confirmed along with it: what
/// a run asks for is a version. A move whose file was not edited is nothing to record, though, so
/// it is left alone — `rola align` is what confirms it.
///
/// # Errors
///
/// Renders [`ErrorTrackNoFiles`] when no file is named, [`ErrorTrackFile`] when a path is not a file
/// or is not inside the Workspace, [`ErrorTrackMessage`] when the messages do not line up or are too
/// long, [`ErrorNoEditor`](crate::editor::ErrorNoEditor) when an editor is needed and none is named,
/// [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace, and [`ErrorTrackFailed`]
/// when the store, the index or the Layout refuses.
#[command(node = "track", entry = EntryTrack)]
pub fn track(args: EntryTrack) -> Next {
    let picked = args
        .pick(&arg![TrackFlags])
        .pick_or_route(&arg![Vec<String>], || ErrorTrackNoFiles.into())
        .to_result();
    let (flags, files) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if files.is_empty() {
        return ErrorTrackNoFiles.into();
    }

    StateTrack {
        files,
        message: flags.message,
        file_messages: flags
            .file_message
            .into_iter()
            // The framework hands a repeated flag its own name back as one of the values; it is not
            // a message a caller wrote.
            .filter(|message| !message.starts_with("--file-message"))
            .collect(),
        no_editor: matches!(flags.no_editor, Flag::Active),
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
) -> Next {
    let StateTrack {
        files,
        message,
        file_messages,
        no_editor,
    } = state;

    let root = match workspace.get_ref().as_ref() {
        Some(workspace_held) => workspace_held.get_root().to_path_buf(),
        None => return ErrorLayoutShouldInWorkspace.into(),
    };

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    // Recording works on a tree that is settled: a path the Layout names and the tree does not hold
    // is something to settle first, with `rola align`, rather than something to record around. A
    // move is not one of these — naming a path where it is now is how a move is confirmed.
    let diff = {
        let Some(workspace_held) = workspace.get_ref().as_ref() else {
            return ErrorLayoutShouldInWorkspace.into();
        };

        match tree_diff(&layout, workspace_held, DEFAULT_ALIKE) {
            Ok(diff) => diff,
            Err(error) => return fail(error.to_string()),
        }
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

    let prepared = match prepare(&layout, &root, &cwd, &files, &diff.renamed) {
        Ok(prepared) => prepared,
        Err(next) => return next,
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return fail(error.to_string()),
    };

    // What goes into the store is decided before anything is asked, so that a file whose content is
    // already recorded is never asked for a message: it will not be given a version to carry one.
    let mut staged = Vec::with_capacity(prepared.len());
    for file in prepared {
        let (storage, changed, base) = match stage(vcs, &runtime, store, &file) {
            Ok(staged) => staged,
            Err(next) => return next,
        };

        staged.push(Staged {
            file,
            storage,
            changed,
            base,
        });
    }

    // The files in the list are the ones that changed; one that did not is not recorded, so it is
    // not something a message stands for either — including when it was named on the command line.
    let changing: Vec<&Staged> = staged.iter().filter(|file| file.changed).collect();
    let files: Vec<&Prepared> = changing.iter().map(|file| &file.file).collect();

    if !file_messages.is_empty() && file_messages.len() != files.len() {
        return message_error(MessageError::Mismatch);
    }

    let own: Vec<String> = if file_messages.is_empty() {
        vec![String::new(); files.len()]
    } else {
        file_messages
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

    let mut items = Vec::with_capacity(changing.len());
    let mut at = 0;
    let mut committed = Vec::with_capacity(changing.len());

    for file in &staged {
        if !file.changed {
            continue;
        }

        let text = &messages[at];
        at += 1;

        if let Err(next) = commit(&layout, vcs, &runtime, &creator_name, file, text) {
            return next;
        }

        committed.push((file.file.path.clone(), file.file.disk.clone()));
        items.push(TrackItem {
            path: file.file.path.as_str().to_owned(),
            action: TrackAction::if_updated(&file.file),
        });
    }

    // What was recorded is what the tree and the Layout now agree on, so the reading remembers it —
    // only for the paths recorded, since remembering the whole tree would take every other change
    // with it and a later `rola status` would have nothing to say about them.
    if let Err(next) = remember(&layout, &root, &committed) {
        return next;
    }

    ResultTrack { items }.into()
}

/// Writes down what a recording left the tree and the Layout agreeing on.
fn remember(layout: &Layout, root: &Path, committed: &[(LayoutPath, PathBuf)]) -> Result<(), Next> {
    if committed.is_empty() {
        return Ok(());
    }

    let path = cache_path(root, layout);
    let mut cache = Cache::read(&path);

    for (layout_path, disk) in committed {
        let entry = entry_of(disk).map_err(|error| fail(error.to_string()))?;
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
    /// Whether that is a change from what the Layout already names.
    changed: bool,
    /// The version it is at now, when it was recorded before.
    base: Option<Version>,
}

/// Resolves each name against the Layout and the tree, or refuses.
///
/// A name is written the way a shell writes one — relative to where the run was made, or absolute —
/// and what a Layout names is a path relative to the Workspace root, so one becomes the other here.
/// A directory is taken recursively: every file under it is recorded, read the way the tree reading
/// reads them, so what a Workspace keeps for itself and what another Workspace holds are left out.
/// What is neither a file nor a directory, is not inside the Workspace, or is part of the
/// Workspace's own data is refused rather than recorded.
fn prepare(
    layout: &Layout,
    root: &Path,
    cwd: &Path,
    names: &[String],
    renames: &[PathRename],
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

        standing.push(if disk.is_dir() {
            Named::Directory(directory_prefix(relative, given)?)
        } else if disk.is_file() {
            Named::File(file_path(relative, given)?, disk)
        } else {
            return Err(ErrorTrackFile::new(given, TrackFileError::NotAFile).into());
        });
    }

    // A directory is read through the same walk the tree reading uses, so what it leaves out — the
    // Workspace's own `.rola`, a Workspace nested inside it, a symbolic link — is left out here too.
    let walked = if standing
        .iter()
        .any(|name| matches!(name, Named::Directory(_)))
    {
        walk(&root).0
    } else {
        Vec::new()
    };

    let mut files: Vec<(LayoutPath, PathBuf)> = Vec::new();
    for name in standing {
        match name {
            Named::File(path, disk) => files.push((path, disk)),
            Named::Directory(prefix) => {
                let mut under: Vec<(LayoutPath, PathBuf)> = walked
                    .iter()
                    .filter(|found| under(&prefix, &found.path))
                    .map(|found| (found.path.clone(), found.disk.clone()))
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

        prepared.push(Prepared {
            path,
            disk,
            id,
            data,
            moved_from,
        });
    }

    Ok(prepared)
}

/// What one name on the command line stands for.
enum Named {
    /// A file, and where it sits on disk.
    File(LayoutPath, PathBuf),
    /// A directory, by the prefix its files share — empty for the Workspace root itself, which is
    /// the one directory with no name of its own, and holds the whole of the work.
    Directory(String),
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

/// The prefix a directory's files share, or the failure for why the directory cannot be named.
fn directory_prefix(relative: &Path, given: &str) -> Result<String, Next> {
    if relative.as_os_str().is_empty() {
        return Ok(String::new());
    }

    Ok(file_path(relative, given)?.as_str().to_owned())
}

/// The path `relative` names, or the failure for why it cannot be named in the Layout.
fn file_path(relative: &Path, given: &str) -> Result<LayoutPath, Next> {
    let Ok(path) = LayoutPath::from_relative(relative) else {
        return Err(ErrorTrackFile::new(given, TrackFileError::OutsideWorkspace).into());
    };

    if path.as_str().split('/').any(|part| part == ".rola") {
        return Err(ErrorTrackFile::new(given, TrackFileError::InsideData).into());
    }

    Ok(path)
}

/// Whether `path` sits under the directory whose prefix is `prefix`.
fn under(prefix: &str, path: &LayoutPath) -> bool {
    if prefix.is_empty() {
        return true;
    }

    path.as_str()
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('/'))
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

/// Puts one file's content into the store, and says whether that is a change.
///
/// The content is written before anything else is decided because what the Layout names is a key
/// the store answers with, not something the file on disk can be compared against directly: content
/// kept as a manifest of chunks hashes to something no reading of the file alone produces.
fn stage(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    store: &RorolalaStorage,
    file: &Prepared,
) -> Result<(Key, bool, Option<Version>), Next> {
    let storage = runtime
        .block_on(store_file(store, &file.disk))
        .map_err(|error| fail(error.to_string()))?;

    match &file.data {
        Some(data) => {
            let (version, current) = load(index, runtime, data.version())?;
            Ok((storage, current != *storage.digest(), Some(version)))
        }
        None => Ok((storage, true, None)),
    }
}

/// Makes a version of the staged content and makes the Layout name it.
fn commit(
    layout: &Layout,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    creator_name: &str,
    file: &Staged,
    text: &str,
) -> Result<(), Next> {
    let base = if let Some(version) = &file.base {
        version.clone()
    } else {
        let root = Version::root();
        runtime
            .block_on(index.write(root.clone()))
            .map_err(|error| fail(error.reason()))?;
        root
    };

    let creator =
        Creator::try_from(creator_name.to_owned()).map_err(|error| fail(error.to_string()))?;
    let message = Message::try_from(text.to_owned()).map_err(|error| fail(error.to_string()))?;

    let creator_key = runtime
        .block_on(index.write(creator))
        .map_err(|error| fail(error.reason()))?;
    let message_key = runtime
        .block_on(index.write(message))
        .map_err(|error| fail(error.reason()))?;

    let variant = base.new_variant(
        *file.storage.digest(),
        *creator_key.digest(),
        *message_key.digest(),
    );
    let version = variant.new_version();

    runtime
        .block_on(index.write(variant))
        .map_err(|error| fail(error.reason()))?;
    let version_key = runtime
        .block_on(index.write(version))
        .map_err(|error| fail(error.reason()))?;

    let data = MutableData::new(
        Some(creator_name.to_owned()),
        *version_key.digest(),
        String::new(),
    );

    // A moved file is brought to where it is now: the move is the last thing done, so that a
    // failure before it — writing the version the change is — leaves the Layout as it was.
    if let Some(from) = &file.file.moved_from
        && let Err(error) = layout.move_path(from, &file.file.path)
    {
        return Err(failed(&error));
    }

    if file.file.data.is_some() {
        if let Err(error) = layout.update_entry(file.file.id, data) {
            return Err(failed(&error));
        }

        return Ok(());
    }

    if let Err(error) = layout.create_entry(file.file.id, data) {
        return Err(failed(&error));
    }
    if let Err(error) = layout.create_path(&file.file.path, file.file.id) {
        return Err(failed(&error));
    }

    Ok(())
}

/// The `Version` stored under `hash`, and the stored hash its variant points at.
fn load(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: [u8; 32],
) -> Result<(Version, [u8; 32]), Next> {
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

    Ok((version, *variant.storage_hash()))
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

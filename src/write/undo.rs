//! Putting back what the recorded operations changed.
//!
//! One run pops the newest operations off the stack `rola opslog` lists and reverses them, newest
//! first. What each operation left is compared with the tree as it is now: an operation whose every
//! change still stands is reversed at once, one that stands only in part is the caller's to decide
//! on, and one that stands nowhere stops the walk and leaves the stack below it alone.
//!
//! The journal is a cache, not a log of record: what an operation wrote on disk is put back from the
//! version the entry already recorded, so a content the store does not hold is one this cannot
//! answer. A file that changed after the operation is refused rather than written over, unless the
//! run says to write it anyway, and a run that moves only the record leaves every file as it is.

use std::fs;
use std::path::{Path, PathBuf};

use rorolala_layout::{EntryKind, Layout, LayoutPath, MutableData};
use rorolala_storage::{Key, StorageSet, key_of_file};
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::Workspace;
use uuid::Uuid;

use crate::write::checkout::Checkout;
use crate::write::journal::{Effect, EntryState, Journal};
use crate::write::layout::{LayoutOp, apply_recorded, switch_entry};

/// How many operations to undo, and how.
#[derive(Debug, Clone, Default)]
pub struct UndoOptions {
    /// The count as the caller wrote it, `None` for one operation.
    pub count: Option<String>,
    /// Whether only the Layout moves back, leaving every file as it is.
    pub soft: bool,
    /// Whether a file that changed after the operation is written over.
    pub force: bool,
}

/// How one recorded operation stands against the tree as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoStanding {
    /// Every change the operation made still stands.
    Whole,
    /// Some of them do.
    Part,
    /// None of them does.
    None,
}

/// One recorded operation, read against the tree.
#[derive(Debug)]
pub struct UndoGroup {
    /// The command that made the changes.
    pub command: String,
    /// How the changes stand.
    pub standing: UndoStanding,
    /// The journal file the operation was read from.
    path: PathBuf,
    /// The changes that still stand, in the order the operation made them, so reversing them
    /// backwards is what puts the work back.
    effects: Vec<Effect>,
}

/// What an undo came to.
#[derive(Debug, Clone, Default)]
pub struct UndoOutcome {
    /// The commands whose operations were undone, newest first.
    pub undone: Vec<String>,
}

/// Why an undo could not be carried out.
#[derive(Debug)]
pub enum UndoRefusal {
    /// The Workspace records no operation.
    NoLog,
    /// The count names no operation the Workspace records.
    Count {
        /// The count as it was written.
        written: String,
        /// How many operations are recorded.
        available: usize,
    },
    /// Moving only the record and writing over a file were asked for together.
    Flags,
    /// A file changed after the operation was recorded.
    Changed {
        /// The file.
        path: String,
    },
    /// A file the operation left is gone.
    Absent {
        /// The file.
        path: String,
    },
    /// The Layout, the store or the journal refused.
    Failed {
        /// The command whose operation failed.
        command: String,
        /// What failed.
        reason: String,
    },
}

/// Reads the operations a run would undo, newest first, as the tree stands now.
///
/// The answer is what a caller that asks its own way reads: each operation says whether every change
/// it made still stands. A classification read here is a reading of the moment, and the operation
/// after it is classified against the tree as it still is, not against what undoing the first would
/// leave; [`undo`] classifies each one as it reaches it, which is what a run that carries them out
/// wants.
///
/// # Errors
///
/// Returns [`UndoRefusal::NoLog`] when the Workspace records nothing, [`UndoRefusal::Count`] when the
/// count names no operation, [`UndoRefusal::Changed`] or [`UndoRefusal::Absent`] when a file stands
/// where the operation cannot be refused past, and [`UndoRefusal::Failed`] when a journal file cannot
/// be read.
pub fn preview(
    workspace: &Workspace,
    layout: &Layout,
    index: Option<&VCSIndex>,
    store: Option<&StorageSet>,
    options: &UndoOptions,
) -> Result<Vec<UndoGroup>, UndoRefusal> {
    let keeper = Keeper::of(layout, index, store, workspace.get_root().to_path_buf())?;
    let mut groups = Vec::new();

    for path in stack(workspace, options.count.as_deref())? {
        let group = read(&keeper, &path, options)?;
        let standing = group.standing;
        groups.push(group);

        if standing == UndoStanding::None {
            break;
        }
    }

    Ok(groups)
}

/// Reverses the newest operations, newest first, one at a time.
///
/// `decide` is asked about an operation that stands only in part, and answers whether it is to be
/// reversed as far as it still stands; the operation before it is already undone by the time it is
/// asked, so what it reads is the tree the run has reached. An operation that stands nowhere ends the
/// walk, and the stack below it is left as it is.
///
/// # Errors
///
/// Returns [`UndoRefusal::Flags`] when moving only the record and writing over a file are asked for
/// together, and everything [`preview`] names.
pub fn undo(
    workspace: &Workspace,
    layout: &Layout,
    index: Option<&VCSIndex>,
    store: Option<&StorageSet>,
    options: &UndoOptions,
    decide: &mut dyn FnMut(&UndoGroup) -> bool,
) -> Result<UndoOutcome, UndoRefusal> {
    // The two ask for different undos: one writes over a file, the other writes none at all.
    if options.force && options.soft {
        return Err(UndoRefusal::Flags);
    }

    let keeper = Keeper::of(layout, index, store, workspace.get_root().to_path_buf())?;
    let mut outcome = UndoOutcome::default();

    for path in stack(workspace, options.count.as_deref())? {
        let group = read(&keeper, &path, options)?;

        if group.standing == UndoStanding::None {
            break;
        }

        if group.standing == UndoStanding::Part && !decide(&group) {
            break;
        }

        reverse(&keeper, &group.effects, options.soft).map_err(|reason| UndoRefusal::Failed {
            command: group.command.clone(),
            reason,
        })?;

        fs::remove_file(&group.path).map_err(|error| UndoRefusal::Failed {
            command: group.command.clone(),
            reason: error.to_string(),
        })?;

        outcome.undone.push(group.command);
    }

    Ok(outcome)
}

/// The recorded operations a run takes, newest first.
fn stack(workspace: &Workspace, written: Option<&str>) -> Result<Vec<PathBuf>, UndoRefusal> {
    let files = Journal::files(workspace).map_err(|error| failed(error.to_string()))?;

    if files.is_empty() {
        return Err(UndoRefusal::NoLog);
    }

    // The files are named by the moment each run ended, so the stack is read backwards: the last
    // name is the newest operation. A count is read against what is recorded, so a number past the
    // stack is a number the caller read somewhere else.
    let count = match written.map(str::trim) {
        None | Some("") => 1,
        Some(written) => match written.parse::<usize>() {
            Ok(number) if number >= 1 && number <= files.len() => number,
            _ => {
                return Err(UndoRefusal::Count {
                    written: written.to_owned(),
                    available: files.len(),
                });
            }
        },
    };

    Ok(files.into_iter().rev().take(count).collect())
}

/// Reads one recorded operation and classifies each change it made.
fn read(keeper: &Keeper<'_>, path: &Path, options: &UndoOptions) -> Result<UndoGroup, UndoRefusal> {
    let operation = Journal::read(path).map_err(|error| failed(error.to_string()))?;
    let mut chosen = Vec::new();
    let mut all = true;

    for effect in &operation.effects {
        // `--soft` moves the record and no file, so a change that is only a file has nothing to do.
        if options.soft && mentions_file(effect) {
            continue;
        }

        match standing(keeper, effect, options.soft) {
            Match::AsLeft => chosen.push(effect.clone()),
            Match::Changed => all = false,
            Match::FileChanged(file) => {
                if !options.force {
                    return Err(UndoRefusal::Changed { path: file });
                }

                chosen.push(effect.clone());
            }
            Match::Absent(file) => {
                if !options.force {
                    return Err(UndoRefusal::Absent { path: file });
                }
            }
        }
    }

    let standing = if chosen.is_empty() {
        UndoStanding::None
    } else if all {
        UndoStanding::Whole
    } else {
        UndoStanding::Part
    };

    Ok(UndoGroup {
        command: operation.command,
        standing,
        path: path.to_path_buf(),
        effects: chosen,
    })
}

/// Whether a change is only about a file on disk, so that moving the record has nothing to do.
const fn mentions_file(effect: &Effect) -> bool {
    matches!(effect, Effect::File { .. } | Effect::FileMoved { .. })
}

/// How one recorded change stands against the tree.
enum Match {
    /// The tree is as the change left it.
    AsLeft,
    /// The Layout is not as the change left it.
    Changed,
    /// The file the change wrote holds something else.
    FileChanged(String),
    /// Something the change left is gone.
    Absent(String),
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

impl<'a> Keeper<'a> {
    /// The keeper a run reads and writes through.
    fn of(
        layout: &'a Layout,
        vcs: Option<&'a VCSIndex>,
        store: Option<&'a StorageSet>,
        root: PathBuf,
    ) -> Result<Self, UndoRefusal> {
        let runtime = tokio::runtime::Runtime::new().map_err(|error| failed(error.to_string()))?;

        Ok(Self {
            layout,
            vcs,
            store,
            root,
            runtime,
        })
    }

    /// The content key the version `version` holds, when the index still names it.
    ///
    /// A version names a variant, and the variant names the content: the entry records a version, so
    /// the two are told apart through the index.
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

/// How `effect` stands against the tree.
///
/// Under `--soft` the files are not this run's to read: the record moves back whatever the disk
/// holds, so only the Layout is compared.
fn standing(keeper: &Keeper<'_>, effect: &Effect, soft: bool) -> Match {
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
                return Match::Changed;
            }

            let Some(after) = after else {
                return Match::AsLeft;
            };

            if soft {
                return Match::AsLeft;
            }

            // A tree is one material, and its files are not what the Layout holds, so only a file
            // entry has a file on disk to compare.
            let Some(path) = path else {
                return Match::AsLeft;
            };

            if after.kind != "file" {
                return Match::AsLeft;
            }

            // A version the index no longer names is one this cannot compare against, and the
            // inverse leaves the file alone rather than refusing the run over it.
            let Some(expected) = keeper.content_of_version(&after.version) else {
                return Match::AsLeft;
            };

            match content_of(&keeper.at(path)) {
                Some(content) if content == expected => Match::AsLeft,
                Some(_) => Match::FileChanged(keeper.at(path).display().to_string()),
                None => Match::Absent(keeper.at(path).display().to_string()),
            }
        }
        Effect::Path { id, from, to } => {
            let matched = to.as_ref().map_or_else(
                || !names(layout, from.as_deref(), *id),
                |to| names(layout, Some(to), *id),
            );

            if matched {
                Match::AsLeft
            } else {
                Match::Changed
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
                Match::AsLeft
            } else {
                Match::Changed
            }
        }
        Effect::File { path, after, .. } => match content_of(&PathBuf::from(path)) {
            Some(content) if Some(&content) == after.as_ref() => Match::AsLeft,
            Some(_) => Match::FileChanged(path.clone()),
            None if after.is_some() => Match::Absent(path.clone()),
            None => Match::AsLeft,
        },
        Effect::FileMoved { from, to } => {
            let from = PathBuf::from(from);
            let to = PathBuf::from(to);

            if from.exists() {
                Match::FileChanged(to.display().to_string())
            } else if to.exists() {
                Match::AsLeft
            } else {
                Match::Absent(to.display().to_string())
            }
        }
    }
}

/// Whether `path` names `id`.
fn names(layout: &Layout, path: Option<&str>, id: Uuid) -> bool {
    path.and_then(|path| LayoutPath::new(path).ok())
        .is_some_and(|path| layout.id_of(&path) == Some(id))
}

/// Reverses `effects`, newest change first.
///
/// Each change is written as it is reversed, so what the next one reads is the Layout the one before
/// it left: a change that moves an entry to another `Uuid` may be reversed before the change that
/// put what the entry held, and the two would collide if they were one batch read from one state.
/// The files follow, and each is written down as it is left, so a reading taken after the undo
/// compares the work against what the undo wrote rather than against what stood before it.
fn reverse(keeper: &Keeper<'_>, effects: &[Effect], soft: bool) -> Result<(), String> {
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
                        && keeper.layout.id_of(&to) == Some(*id)
                        && keeper.layout.id_of(&from).is_none()
                    {
                        write(keeper.layout, LayoutOp::MovePath { from: to, to: from })?;
                    }
                }
                (Some(from), None) => {
                    if let Ok(from) = LayoutPath::new(from)
                        && keeper.layout.id_of(&from).is_none()
                        && keeper.layout.entry(*id).is_some()
                    {
                        write(
                            keeper.layout,
                            LayoutOp::CreatePath {
                                path: from,
                                id: *id,
                            },
                        )?;
                    }
                }
                (None, Some(to)) => {
                    if let Ok(to) = LayoutPath::new(to)
                        && keeper.layout.id_of(&to) == Some(*id)
                    {
                        write(keeper.layout, LayoutOp::RemovePath { path: to })?;
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
                    .filter(|path| keeper.layout.id_of(path) == Some(*to));

                if let Some(path) = at_to
                    && keeper.layout.entry(*from).is_none()
                {
                    switch_entry(keeper.layout, &path, *from, data_of(before)?)
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

    let mut checkout = Checkout::open(keeper.layout, &keeper.root);
    let mut touched = false;

    for action in files {
        action.make(keeper.store)?;
        touched |= action.remember(&mut checkout, &keeper.root)?;
    }

    if touched {
        checkout.finish().map_err(|error| error.to_string())?;
    }

    Ok(())
}

/// Puts an entry back to the state `before` holds, and the file to the content of its version.
fn undo_entry(
    keeper: &Keeper<'_>,
    id: Uuid,
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

/// Writes down that `path` under `root` holds what the Layout now names.
///
/// A path outside the Workspace names nothing the Layout keeps, and a layout path is written with
/// the separators the Layout uses whichever way the system spells a directory.
fn remember(checkout: &mut Checkout, root: &Path, path: &Path) -> Result<bool, String> {
    let Some(path) = relative(root, path) else {
        return Ok(false);
    };

    checkout
        .remember(&path)
        .map_err(|error| error.to_string())?;

    Ok(true)
}

/// Forgets the reading of a path the run took out.
fn forget(checkout: &mut Checkout, root: &Path, path: &Path) -> Result<bool, String> {
    let Some(path) = relative(root, path) else {
        return Ok(false);
    };

    checkout.forget(&path).map_err(|error| error.to_string())?;

    Ok(true)
}

/// The path a Layout names, from the whole path a journal recorded.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let text = relative.to_string_lossy().replace('\\', "/");

    (!text.is_empty()).then_some(text)
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
    /// Writes down what the path holds now, so the next reading does not compare against what it
    /// held before the undo. Answers whether there was anything to write.
    fn remember(&self, checkout: &mut Checkout, root: &Path) -> Result<bool, String> {
        match self {
            Self::PutBack { path, .. } => remember(checkout, root, path),
            Self::Delete { path } => forget(checkout, root, path),
            Self::MoveBack { from, to } => {
                forget(checkout, root, from)?;

                remember(checkout, root, to)
            }
        }
    }

    /// Carries the action out.
    fn make(&self, store: Option<&StorageSet>) -> Result<(), String> {
        match self {
            Self::PutBack { path, content } => {
                let Some(store) = store else {
                    return Ok(());
                };

                let hash = parse_hash(content)
                    .ok_or_else(|| format!("`{content}` is not the key of any content"))?;

                if let Some(parent) = path.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(error.to_string());
                }

                let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;

                runtime
                    .block_on(store.extract_file(&Key::new(hash), path))
                    .map_err(|error| error.to_string())
            }
            Self::Delete { path } => match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.to_string()),
            },
            Self::MoveBack { from, to } => {
                if !from.exists() {
                    return Ok(());
                }

                if to.exists() {
                    return Err(format!("`{}` is already there", to.display()));
                }

                if let Some(parent) = to.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(error.to_string());
                }

                fs::rename(from, to).map_err(|error| error.to_string())
            }
        }
    }
}

/// The entry `id` holds, in the shape the journal keeps it.
fn entry_state(layout: &Layout, id: Uuid) -> Option<EntryState> {
    layout.entry(id).map(|data| EntryState::of(&data))
}

/// What a journalled entry state holds, as a Layout entry.
fn data_of(state: &EntryState) -> Result<MutableData, String> {
    let version = parse_hash(&state.version)
        .ok_or_else(|| format!("`{}` is not the hash of any version", state.version))?;
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

/// The refusal a journal that cannot be read is told as.
fn failed(reason: String) -> UndoRefusal {
    UndoRefusal::Failed {
        command: "undo".to_owned(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rorolala_layout::{LayoutPath, MutableData};
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{
        EntryState, Journal, LayoutOp, UndoOptions, UndoRefusal, apply_recorded, data_of,
        parse_hash, undo,
    };

    /// A Workspace under a directory of its own.
    fn scratch(name: &str) -> (PathBuf, Workspace) {
        let dir =
            std::env::temp_dir().join(format!("rola-write-undo-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        (dir, workspace)
    }

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

    #[test]
    fn an_operation_that_stands_whole_is_reversed() {
        let (dir, workspace) = scratch("whole");
        let layout = workspace.layout().unwrap();
        let id = Uuid::from_u128(1);
        let path = LayoutPath::new("a.txt").unwrap();

        apply_recorded(
            &layout,
            &[LayoutOp::CreateEntry {
                id,
                path: Some(path.clone()),
                data: MutableData::new(None, [1; 32], String::new()),
            }],
            None,
        )
        .unwrap();

        // The journal the recording would have written: the entry and the path it was bound to.
        let mut journal = Journal::open(&workspace, "track");
        journal.record(super::Effect::Entry {
            id,
            path: Some("a.txt".to_owned()),
            before: None,
            after: Some(EntryState::of(&layout.entry(id).unwrap())),
        });
        journal.record(super::Effect::Path {
            id,
            from: None,
            to: Some("a.txt".to_owned()),
        });
        journal.finish().unwrap();

        let outcome = undo(
            &workspace,
            &layout,
            None,
            None,
            &UndoOptions::default(),
            &mut |_| true,
        )
        .unwrap();

        assert_eq!(outcome.undone, vec!["track".to_owned()]);
        assert!(layout.id_of(&path).is_none());
        assert_eq!(Journal::files(&workspace).unwrap(), [] as [PathBuf; 0]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_operation_that_stands_nowhere_is_left_alone() {
        let (dir, workspace) = scratch("none");
        let layout = workspace.layout().unwrap();

        // The operation bound a path the Layout no longer names, so none of what it did still stands.
        let mut journal = Journal::open(&workspace, "track");
        journal.record(super::Effect::Path {
            id: Uuid::from_u128(2),
            from: None,
            to: Some("b.txt".to_owned()),
        });
        journal.finish().unwrap();

        let outcome = undo(
            &workspace,
            &layout,
            None,
            None,
            &UndoOptions::default(),
            &mut |_| true,
        )
        .unwrap();

        assert_eq!(outcome.undone, Vec::<String>::new());
        assert_eq!(Journal::files(&workspace).unwrap().len(), 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_count_past_the_stack_is_refused() {
        let (dir, workspace) = scratch("count");
        let layout = workspace.layout().unwrap();
        let mut journal = Journal::open(&workspace, "track");
        journal.record(super::Effect::Path {
            id: Uuid::from_u128(3),
            from: None,
            to: Some("a.txt".to_owned()),
        });
        journal.finish().unwrap();

        let refused = undo(
            &workspace,
            &layout,
            None,
            None,
            &UndoOptions {
                count: Some("9".to_owned()),
                ..UndoOptions::default()
            },
            &mut |_| true,
        );

        assert!(matches!(
            refused,
            Err(UndoRefusal::Count { available: 1, .. })
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_workspace_with_no_operation_refuses_to_undo() {
        let (dir, workspace) = scratch("empty");
        let layout = workspace.layout().unwrap();

        let refused = undo(
            &workspace,
            &layout,
            None,
            None,
            &UndoOptions::default(),
            &mut |_| true,
        );

        assert!(matches!(refused, Err(UndoRefusal::NoLog)));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_two_flags_together_are_refused() {
        let (dir, workspace) = scratch("flags");
        let layout = workspace.layout().unwrap();

        let refused = undo(
            &workspace,
            &layout,
            None,
            None,
            &UndoOptions {
                force: true,
                soft: true,
                ..UndoOptions::default()
            },
            &mut |_| true,
        );

        assert!(matches!(refused, Err(UndoRefusal::Flags)));

        let _ = fs::remove_dir_all(dir);
    }
}

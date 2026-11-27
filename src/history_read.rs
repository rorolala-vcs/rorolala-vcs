//! Reading the operations a Workspace recorded, for the C ABI.
//!
//! The journal `rola opslog` lists is one file per run under `.rola/cache/log/`, and this hands the
//! same list over as values: the command that made each run, when it ended, and how many changes it
//! holds, newest first. A file that does not read is skipped and counted, so one damaged record does
//! not hide the others.
//!
//! The list is a cache, not a log of record: a Workspace that removes the files keeps its work and
//! loses the ability to undo or to list what was recorded there.

use std::path::Path;

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;

use crate::write::journal::{Effect, Journal};

/// The operations a Workspace recorded, newest first, as a C caller reads them.
///
/// # FFI
///
/// `workspace` is the root of the Workspace, and borrowed for the call. A
/// `RolaHistoryOperations *` the caller releases with `free_rola_history_operations` answers it,
/// each operation released with `free_rola_history_operation`, or null when there is nothing to
/// read: no Workspace holds the directory, or its log cannot be listed.
#[lazyffi(export = rola_history_operations)]
#[must_use]
pub fn recorded_operations(workspace: &str) -> Option<HistoryOperations> {
    let workspace = Workspace::locate(Path::new(workspace))?;
    let files = Journal::files(&workspace).ok()?;
    let mut operations = Vec::new();
    let mut unreadable: u64 = 0;

    // The files are named by the moment each run ended, so the list read backwards is the stack
    // `rola undo` pops: newest first.
    for path in files.iter().rev() {
        match Journal::read(path) {
            Ok(group) => operations.push(HistoryOperation {
                number: u64::try_from(operations.len() + 1).unwrap_or(u64::MAX),
                command: group.command,
                time: group.time,
                changes: group.effects.iter().map(HistoryChange::of).collect(),
            }),
            Err(_) => unreadable += 1,
        }
    }

    Some(HistoryOperations {
        operations,
        unreadable,
    })
}

/// One operation a Workspace recorded, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaHistoryOperation *` the caller releases with `free_rola_history_operation`. `_command`
/// and `_time` each return a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaHistoryOperation)]
#[derive(Debug, Clone, Default)]
pub struct HistoryOperation {
    /// Where it sits in the list, counting from the newest.
    number: u64,
    /// The command that made the changes.
    command: String,
    /// When the run ended, in UTC.
    time: String,
    /// What the run did, in the order it did it.
    changes: Vec<HistoryChange>,
}

#[lazyffi(export = rola_)]
impl HistoryOperation {
    /// Where it sits in the list, counting from the newest.
    #[must_use]
    pub const fn number(&self) -> u64 {
        self.number
    }

    /// The command that made the changes.
    #[must_use]
    pub fn command(&self) -> String {
        self.command.clone()
    }

    /// When the run ended, in UTC.
    #[must_use]
    pub fn time(&self) -> String {
        self.time.clone()
    }

    /// How many changes it holds.
    #[must_use]
    pub fn changes(&self) -> u64 {
        u64::try_from(self.changes.len()).unwrap_or(u64::MAX)
    }

    /// One change it holds, in the order the run made it, or null past the end.
    #[must_use]
    pub fn change(&self, index: usize) -> Option<HistoryChange> {
        self.changes.get(index).cloned()
    }
}

/// One change a recorded operation made, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaHistoryChange *` the caller releases with `free_rola_history_change`. `_kind`, `_from`,
/// `_to` and `_state` each return a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaHistoryChange)]
#[derive(Debug, Clone, Default)]
pub struct HistoryChange {
    /// What kind of thing changed: `entry`, `path`, `uuid`, `file` or `file_moved`.
    kind: String,
    /// The path before the change, or the path the change is about.
    from: String,
    /// The path after the change, when the change moved something.
    to: String,
    /// What became of it: `added`, `removed`, `changed`, `bound`, `unbound`, `moved` or
    /// `reidentified`.
    state: String,
}

impl HistoryChange {
    /// What one effect of a run comes to as a change a reader can be shown.
    ///
    /// The effect's own spelling is the journal's, which says more about the layout than a reader
    /// needs: an entry that gained a record, a path that was bound, and a file that was written are
    /// three ways of saying something appeared, moved or went. What crosses is the path or the two
    /// paths and one word for which of the three it was, so that what draws it does not have to know
    /// the journal's shape.
    fn of(effect: &Effect) -> Self {
        match effect {
            Effect::Entry {
                path,
                before,
                after,
                ..
            } => Self {
                kind: "entry".to_owned(),
                from: path.clone().unwrap_or_default(),
                to: String::new(),
                state: presence(before.is_some(), after.is_some()).to_owned(),
            },
            Effect::Path { from, to, .. } => Self {
                kind: "path".to_owned(),
                from: from.clone().unwrap_or_default(),
                to: to.clone().unwrap_or_default(),
                state: match (from.is_some(), to.is_some()) {
                    (false, true) => "bound",
                    (true, false) => "unbound",
                    _ => "moved",
                }
                .to_owned(),
            },
            Effect::Uuid { path, .. } => Self {
                kind: "uuid".to_owned(),
                from: path.clone().unwrap_or_default(),
                to: String::new(),
                state: "reidentified".to_owned(),
            },
            Effect::File {
                path,
                before,
                after,
            } => Self {
                kind: "file".to_owned(),
                from: path.clone(),
                to: String::new(),
                state: presence(before.is_some(), after.is_some()).to_owned(),
            },
            Effect::FileMoved { from, to } => Self {
                kind: "file_moved".to_owned(),
                from: from.clone(),
                to: to.clone(),
                state: "moved".to_owned(),
            },
        }
    }
}

#[lazyffi(export = rola_)]
impl HistoryChange {
    /// What kind of thing changed: `entry`, `path`, `uuid`, `file` or `file_moved`.
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    /// The path before the change, or the path the change is about.
    #[must_use]
    pub fn from(&self) -> String {
        self.from.clone()
    }

    /// The path after the change, when the change moved something.
    #[must_use]
    pub fn to(&self) -> String {
        self.to.clone()
    }

    /// What became of it: `added`, `removed`, `changed`, `bound`, `unbound`, `moved` or
    /// `reidentified`.
    #[must_use]
    pub fn state(&self) -> String {
        self.state.clone()
    }
}

/// The word for what became of something that was not there before, or is not there after.
const fn presence(before: bool, after: bool) -> &'static str {
    match (before, after) {
        (false, true) => "added",
        (true, false) => "removed",
        _ => "changed",
    }
}

/// The operations a Workspace recorded, as the C ABI hands them over.
///
/// # FFI
///
/// A `RolaHistoryOperations *` the caller releases with `free_rola_history_operations`. `_count`
/// says how many operations the list holds, `_unreadable_count` how many files could not be read,
/// and `_at` answers one operation, or null past the end, as a `RolaHistoryOperation *` the caller
/// releases with `free_rola_history_operation`.
#[lazyffi(export = RolaHistoryOperations)]
#[derive(Debug, Clone, Default)]
pub struct HistoryOperations {
    /// Each operation, newest first.
    operations: Vec<HistoryOperation>,
    /// How many log files could not be read.
    unreadable: u64,
}

#[lazyffi(export = rola_)]
impl HistoryOperations {
    /// How many operations the list holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.operations.len()
    }

    /// How many log files could not be read and were skipped.
    #[must_use]
    pub const fn unreadable_count(&self) -> u64 {
        self.unreadable
    }

    /// One operation, newest first, or null past the end.
    #[must_use]
    pub fn at(&self, index: usize) -> Option<HistoryOperation> {
        self.operations.get(index).cloned()
    }
}

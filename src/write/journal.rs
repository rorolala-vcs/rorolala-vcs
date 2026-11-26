//! What a run did, as the stack `rola undo` reverses.
//!
//! One run writes one file, under `.rola/cache/log/`, named by the moment it ended and a `Uuid` so
//! the newest is the last by name. A file holds the command and the effects, each with the state
//! before and after it, which is enough to put the work back without asking the command what it
//! meant. A run that made no change writes nothing.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rorolala_layout::{EntryKind, Layout, MutableData};
use rorolala_storage::{Key, key_of_file};
use rorolala_utils_constants::WORKSPACE_LOG_DIR;
use rorolala_utils_location::Locate as _;
use rorolala_workspace::Workspace;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::write::WriteError;

/// The version of the journal format this build writes.
const FORMAT: u32 = 1;

/// One entry as the journal remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryState {
    /// Whether the entry names a file or a tree.
    pub kind: String,
    /// Who holds it, when anyone does.
    pub owner: Option<String>,
    /// The version it is at, as hex.
    pub version: String,
    /// What it says about itself.
    pub description: String,
    /// The cut its content is written with, when it has one.
    pub preference: Option<String>,
}

impl EntryState {
    /// The state of `data`.
    #[must_use]
    pub fn of(data: &MutableData) -> Self {
        Self {
            kind: match data.kind() {
                EntryKind::File => "file".to_owned(),
                EntryKind::Tree => "tree".to_owned(),
            },
            owner: data.owner().map(str::to_owned),
            version: Key::new(data.version()).hex(),
            description: data.description().to_owned(),
            preference: data.preference().map(str::to_owned),
        }
    }
}

/// One change a run made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    /// An entry was made, changed or removed.
    Entry {
        /// The entry.
        id: Uuid,
        /// The path it was at, when it was at one.
        path: Option<String>,
        /// What it held before.
        before: Option<EntryState>,
        /// What it holds after.
        after: Option<EntryState>,
    },
    /// A path was bound, moved or removed.
    Path {
        /// The entry the path names.
        id: Uuid,
        /// The path before.
        from: Option<String>,
        /// The path after.
        to: Option<String>,
    },
    /// An entry was moved to another `Uuid`.
    Uuid {
        /// The `Uuid` it had.
        from: Uuid,
        /// The `Uuid` it has.
        to: Uuid,
        /// The path it was at.
        path: Option<String>,
        /// What it held before.
        before: EntryState,
        /// What it holds after.
        after: EntryState,
    },
    /// A file was written, overwritten or removed.
    File {
        /// The path of the file.
        path: String,
        /// The content key before, or nothing when there was no file.
        before: Option<String>,
        /// The content key after, or nothing when the file is gone.
        after: Option<String>,
    },
    /// A file was moved.
    FileMoved {
        /// The path before.
        from: String,
        /// The path after.
        to: String,
    },
}

/// One run's effects, as the file on disk holds them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// The version of the format the file was written in.
    pub version: u32,
    /// The command that made the changes.
    pub command: String,
    /// When the run ended, in UTC.
    pub time: String,
    /// What the run did, in the order it did it.
    pub effects: Vec<Effect>,
}

/// The effects of one run, gathered before they are written.
#[derive(Debug)]
pub struct Journal {
    root: PathBuf,
    command: String,
    time: String,
    effects: Vec<Effect>,
}

impl Journal {
    /// A journal for `command`, kept under `workspace`'s cache.
    #[must_use]
    pub fn open(workspace: &Workspace, command: &str) -> Self {
        Self {
            root: workspace.get_root().join(WORKSPACE_LOG_DIR),
            command: command.to_owned(),
            time: utc_stamp(),
            effects: Vec::new(),
        }
    }

    /// Records one change.
    pub fn record(&mut self, effect: Effect) {
        self.effects.push(effect);
    }

    /// How many changes it holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.effects.len()
    }

    /// Whether it holds no change.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Writes the group, and answers the file it wrote.
    ///
    /// A run that made no change writes nothing, so the stack has no empty entry.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Io`] when the directory or the file cannot be written, and
    /// [`WriteError::Internal`] when the group does not encode.
    pub fn finish(self) -> Result<Option<PathBuf>, WriteError> {
        if self.effects.is_empty() {
            return Ok(None);
        }

        let group = Group {
            version: FORMAT,
            command: self.command,
            time: self.time,
            effects: self.effects,
        };
        let bytes = serde_json::to_vec_pretty(&group).map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;
        fs::create_dir_all(&self.root).map_err(|error| WriteError::Io {
            path: self.root.display().to_string(),
            reason: error.to_string(),
        })?;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let name = format!("{nanos:020}-{}.json", Uuid::new_v4());
        let path = self.root.join(&name);
        let temporary = self.root.join(format!("{name}.tmp"));

        fs::write(&temporary, bytes).map_err(|error| WriteError::Io {
            path: temporary.display().to_string(),
            reason: error.to_string(),
        })?;
        fs::rename(&temporary, &path).map_err(|error| WriteError::Io {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;

        Ok(Some(path))
    }

    /// The files a Workspace keeps, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Io`] when the directory cannot be read.
    pub fn files(workspace: &Workspace) -> Result<Vec<PathBuf>, WriteError> {
        let root = workspace.get_root().join(WORKSPACE_LOG_DIR);
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(WriteError::Io {
                    path: root.display().to_string(),
                    reason: error.to_string(),
                });
            }
        };

        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(OsStr::to_str) == Some("json"))
            .collect();
        files.sort();

        Ok(files)
    }

    /// Reads one group.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Io`] when the file cannot be read, and [`WriteError::Internal`] when it
    /// does not read as a group.
    pub fn read(path: &Path) -> Result<Group, WriteError> {
        let bytes = fs::read(path).map_err(|error| WriteError::Io {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;

        serde_json::from_slice(&bytes).map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })
    }
}

/// A Layout's entries and paths, as a run found them or left them.
#[derive(Debug, Default)]
pub struct LayoutSnapshot {
    entries: BTreeMap<Uuid, EntryState>,
    paths: BTreeMap<String, Uuid>,
}

impl LayoutSnapshot {
    /// Reads `layout`.
    #[must_use]
    pub fn of(layout: &Layout) -> Self {
        let entries = layout
            .entries()
            .into_iter()
            .map(|(id, data)| (id, EntryState::of(&data)))
            .collect();
        let paths = layout
            .paths()
            .into_iter()
            .map(|(path, id)| (path.as_str().to_owned(), id))
            .collect();

        Self { entries, paths }
    }

    /// Records what changed between `self` and `after`.
    pub fn record(&self, after: &Self, journal: &mut Journal) {
        let before_paths = by_id(&self.paths);
        let after_paths = by_id(&after.paths);

        for (id, before) in &self.entries {
            match after.entries.get(id) {
                Some(after) if after != before => journal.record(Effect::Entry {
                    id: *id,
                    path: after_paths
                        .get(id)
                        .or_else(|| before_paths.get(id))
                        .cloned(),
                    before: Some(before.clone()),
                    after: Some(after.clone()),
                }),
                None => journal.record(Effect::Entry {
                    id: *id,
                    path: before_paths.get(id).cloned(),
                    before: Some(before.clone()),
                    after: None,
                }),
                _ => {}
            }
        }
        for (id, after) in &after.entries {
            if !self.entries.contains_key(id) {
                journal.record(Effect::Entry {
                    id: *id,
                    path: after_paths.get(id).cloned(),
                    before: None,
                    after: Some(after.clone()),
                });
            }
        }

        for (path, id) in &self.paths {
            match after.paths.get(path) {
                Some(after_id) if after_id != id => {
                    let (Some(before), Some(after)) =
                        (self.entries.get(id), after.entries.get(after_id))
                    else {
                        continue;
                    };
                    journal.record(Effect::Uuid {
                        from: *id,
                        to: *after_id,
                        path: Some(path.clone()),
                        before: before.clone(),
                        after: after.clone(),
                    });
                }
                None => journal.record(Effect::Path {
                    id: *id,
                    from: Some(path.clone()),
                    to: None,
                }),
                _ => {}
            }
        }
        for (path, id) in &after.paths {
            if !self.paths.contains_key(path) {
                journal.record(Effect::Path {
                    id: *id,
                    from: None,
                    to: Some(path.clone()),
                });
            }
        }
    }
}

/// The path each entry is at, by the entry.
fn by_id(paths: &BTreeMap<String, Uuid>) -> BTreeMap<Uuid, String> {
    paths.iter().map(|(path, id)| (*id, path.clone())).collect()
}

/// The content of the files a run touched, by the path each lies at.
///
/// A run that writes a working file records what it held before, which a version cannot answer: the
/// content may be one the run only ever had on disk. A file read here is read again when the run
/// ends, and the two readings are what the journal keeps.
#[derive(Debug, Default)]
pub struct FileStates {
    files: BTreeMap<String, Option<String>>,
}

impl FileStates {
    /// Reads each path in `paths`.
    #[must_use]
    pub fn of(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let files = paths
            .into_iter()
            .map(|path| (path.display().to_string(), content_of(&path)))
            .collect();

        Self { files }
    }

    /// Reads one path.
    #[must_use]
    pub fn read(path: impl Into<PathBuf>) -> Self {
        Self::of([path.into()])
    }

    /// Reads each path in `paths` as holding nothing, for a run that writes only paths it knows were
    /// not there.
    #[must_use]
    pub fn absent_before(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let files = paths
            .into_iter()
            .map(|path| (path.display().to_string(), None))
            .collect();

        Self { files }
    }

    /// Records each path whose content is not what it was when read.
    pub fn record(&self, journal: &mut Journal) {
        for (path, before) in &self.files {
            let after = content_of(Path::new(path));

            if after != *before {
                journal.record(Effect::File {
                    path: path.clone(),
                    before: before.clone(),
                    after,
                });
            }
        }
    }
}

/// The key of the content at `path`, or nothing when there is no file there.
///
/// A directory is not a file of its own, so it reads as nothing: a tree is recorded as one entry
/// rather than as the files under it.
fn content_of(path: &Path) -> Option<String> {
    key_of_file(path).ok().map(|key| key.hex())
}

/// The day and time a run ended, in UTC, as `YYYY-MM-DDTHH:MM:SSZ`.
fn utc_stamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let days = seconds / 86_400;
    let of_day = seconds % 86_400;
    let hour = of_day / 3_600;
    let minute = (of_day % 3_600) / 60;
    let second = of_day % 60;

    format!(
        "{}T{hour:02}:{minute:02}:{second:02}Z",
        civil_from_days(days)
    )
}

/// The date `days` after the Unix epoch, as `YYYY-MM-DD`.
fn civil_from_days(days: u64) -> String {
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_of_year = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_of_year + 2) / 5 + 1;
    let month = if month_of_year < 10 {
        month_of_year + 3
    } else {
        month_of_year - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use rorolala_layout::{LayoutPath, MutableData};

    use super::{Effect, FileStates, Journal, LayoutSnapshot};
    use crate::write::layout::{LayoutOp, apply_recorded};

    /// A Workspace under a directory of its own.
    fn scratch(name: &str) -> (PathBuf, Workspace) {
        let dir =
            std::env::temp_dir().join(format!("rola-write-journal-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        (dir, workspace)
    }

    #[test]
    fn a_group_round_trips() {
        let (dir, workspace) = scratch("round-trip");
        let effect = Effect::Path {
            id: Uuid::from_u128(1),
            from: Some("a.psd".to_owned()),
            to: None,
        };
        let mut journal = Journal::open(&workspace, "test");
        journal.record(effect.clone());
        let path = journal.finish().unwrap().unwrap();
        let group = Journal::read(&path).unwrap();

        assert_eq!(group.command, "test");
        assert_eq!(group.effects, vec![effect]);
        assert_eq!(Journal::files(&workspace).unwrap(), vec![path]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_run_that_made_no_change_writes_nothing() {
        let (dir, workspace) = scratch("empty");
        let journal = Journal::open(&workspace, "test");

        assert_eq!(journal.finish().unwrap(), None);
        assert_eq!(Journal::files(&workspace).unwrap(), [] as [PathBuf; 0]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn two_groups_in_one_second_keep_their_order() {
        let (dir, workspace) = scratch("order");
        let mut first = Journal::open(&workspace, "first");
        first.record(Effect::FileMoved {
            from: "a".to_owned(),
            to: "b".to_owned(),
        });
        let first = first.finish().unwrap().unwrap();

        let mut second = Journal::open(&workspace, "second");
        second.record(Effect::FileMoved {
            from: "b".to_owned(),
            to: "c".to_owned(),
        });
        let second = second.finish().unwrap().unwrap();

        let files = Journal::files(&workspace).unwrap();
        assert_eq!(files.len(), 2);
        assert!(first < second);
        assert_eq!(
            Journal::read(files.last().unwrap()).unwrap().command,
            "second"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_snapshot_records_what_changed() {
        let (dir, workspace) = scratch("snapshot");
        let layout = workspace.layout().unwrap();
        let id = Uuid::from_u128(1);
        apply_recorded(
            &layout,
            &[LayoutOp::CreateEntry {
                id,
                path: Some(LayoutPath::new("a.psd").unwrap()),
                data: MutableData::new(None, [1; 32], String::new()),
            }],
            None,
        )
        .unwrap();
        let before = LayoutSnapshot::of(&layout);

        apply_recorded(
            &layout,
            &[LayoutOp::UpdateEntry {
                id,
                data: MutableData::new(None, [2; 32], String::new()),
            }],
            None,
        )
        .unwrap();
        let after = LayoutSnapshot::of(&layout);
        let mut journal = Journal::open(&workspace, "test");
        before.record(&after, &mut journal);
        let path = journal.finish().unwrap().unwrap();
        let group = Journal::read(&path).unwrap();

        assert_eq!(group.effects.len(), 1);
        assert!(matches!(
            &group.effects[0],
            Effect::Entry {
                before: Some(before),
                after: Some(after),
                ..
            } if before.version != after.version
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_state_records_what_changed() {
        let (dir, workspace) = scratch("file-states");
        let path = dir.join("a.txt");
        fs::write(&path, b"one").unwrap();
        let files = FileStates::read(path.clone());
        fs::write(&path, b"two").unwrap();
        let mut journal = Journal::open(&workspace, "test");
        files.record(&mut journal);
        let written = journal.finish().unwrap().unwrap();
        let group = Journal::read(&written).unwrap();

        assert_eq!(group.effects.len(), 1);
        assert!(matches!(
            &group.effects[0],
            Effect::File {
                before: Some(before),
                after: Some(after),
                ..
            } if before != after
        ));

        // A path that is what it was leaves nothing, and so does a path absent both times.
        let unchanged = FileStates::read(path);
        let missing = FileStates::absent_before([dir.join("gone.txt")]);
        let mut journal = Journal::open(&workspace, "test");
        unchanged.record(&mut journal);
        missing.record(&mut journal);
        assert_eq!(journal.finish().unwrap(), None);

        // A path absent before and written after is a file the run made.
        let made = dir.join("b.txt");
        let files = FileStates::absent_before([made.clone()]);
        fs::write(&made, b"three").unwrap();
        let mut journal = Journal::open(&workspace, "test");
        files.record(&mut journal);
        let written = journal.finish().unwrap().unwrap();
        let group = Journal::read(&written).unwrap();

        assert!(matches!(
            &group.effects[0],
            Effect::File {
                before: None,
                after: Some(_),
                ..
            }
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_refused_batch_writes_no_file() {
        let (dir, workspace) = scratch("refused");
        let layout = workspace.layout().unwrap();
        let mut journal = Journal::open(&workspace, "test");
        let result = apply_recorded(
            &layout,
            &[LayoutOp::RemoveEntry {
                id: Uuid::from_u128(9),
            }],
            Some(&mut journal),
        );

        assert!(result.is_err());
        drop(journal);
        assert_eq!(Journal::files(&workspace).unwrap(), [] as [PathBuf; 0]);

        let _ = fs::remove_dir_all(dir);
    }
}

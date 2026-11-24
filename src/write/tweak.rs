//! Adjusting a chain's history, and rebuilding what follows the point adjusted.
//!
//! A variant and the version it fixes are content addressed, so nothing here rewrites one. An
//! adjustment is written as a new variant and a new version, and every version above the adjusted
//! point is replayed against the new base with the content, creator, message and merge it already
//! carried. The Layout is then made to record the replayed tail, and the old chain stays in the
//! index exactly as it stood, reachable from nowhere but still there.
//!
//! What a run adjusts is a [`Sheet`]: one row a version, the version the entry records first, down
//! to the oldest one being adjusted. The same text is what an editor writes and what waits in the
//! Workspace as a plan, so the two cannot drift: a plan is a sheet that was written down instead of
//! applied, and the list of pending adjustments is the difference between that sheet and the chain.
//!
//! A row stands for a version by its place, and two markers say what a place does beyond standing:
//! `--` removes the version there, and `++` adds one between the rows around it. Nothing else is
//! positional, so a sheet must cover every version of its range, unchanged ones included.

// The workflow reads an entry, a chain, a sheet and the store in one pass, and cutting that pass
// into helpers would only spread the borrows of one run across several signatures.
#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use rorolala_auth::Account;
use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_protocol::VaultAddress;
use rorolala_storage::{Blake3Hash, Key, Resolved, ShortHash, StorageSet, key_of_file};
use rorolala_utils_constants::WORKSPACE_TWEAKING_DIR;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rorolala_vcs::{Creator, Message, VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version};
use rorolala_workspace::{Merging, Workspace};
use uuid::Uuid;

use crate::fetch::Sources;
use crate::write::checkout::remember;
use crate::write::layout::set_version;
use crate::write::{Cancellation, WriteError};

/// How many hex digits a hash is written with when a sheet names it.
const SHORT: usize = 12;

/// The longest a version's head is drawn as when it stands beside its number.
const HEAD: usize = 7;

/// The file a plan is written as, inside its directory.
const PLAN_FILE: &str = "plan.md";

/// The widest a column is padded to keep the table readable.
///
/// A creator or a message longer than this simply runs on: a table is a convenience, and cutting
/// what a person wrote for its sake costs them the words.
const WIDEST: usize = 40;

// ---------------------------------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------------------------------

/// What one row of a sheet does to the version it stands at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// The version stays, saying what the row says.
    Kept,
    /// The version goes, and the chain closes over where it stood.
    Remove,
    /// A version is added where the row stands.
    Insert,
}

/// One version of a chain, as a sheet says it.
///
/// The content and the merge are keys written as a caller typed them, which may be a whole key or
/// the head of one, and are settled against the store and the index when the sheet is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What the row does to the version it stands at.
    pub mark: Mark,
    /// The name of the account that made the version.
    pub creator: String,
    /// What the version says.
    pub message: String,
    /// The content the version points at.
    pub content: String,
    /// The variant the version is a merge with, when it is one.
    pub join: Option<String>,
}

impl Row {
    /// The version `level` stands for, as a row that changes nothing.
    fn kept(level: &Level) -> Self {
        Self {
            mark: Mark::Kept,
            creator: level.creator.clone(),
            message: level.message.clone(),
            content: short(level.variant.storage_hash()),
            join: level.variant.join().map(|join| format!("+{}", short(join))),
        }
    }
}

/// A chain read as rows, the version the entry records first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// The oldest version the sheet stands for, as it was written.
    anchor: Option<String>,
    /// One row a version, the version the entry records first.
    rows: Vec<Row>,
}

/// Why a sheet could not be read.
#[derive(Debug)]
pub struct SheetError {
    /// The line the fault is on, counting from one.
    line: usize,
    /// What is wrong with it.
    what: &'static str,
}

impl SheetError {
    /// The fault on `line`.
    const fn new(line: usize, what: &'static str) -> Self {
        Self { line, what }
    }
}

impl fmt::Display for SheetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.what)
    }
}

impl std::error::Error for SheetError {}

impl Sheet {
    /// A sheet that changes nothing, standing from `anchor` to the version the entry records.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Nothing`] when the chain is shorter than `anchor`.
    pub fn plain(chain: &Chain, anchor: usize) -> Result<Self, TweakRefusal> {
        let rows = (anchor..=chain.top()?)
            .rev()
            .map(|number| Ok(Row::kept(chain.level(number)?)))
            .collect::<Result<Vec<_>, TweakRefusal>>()?;

        Ok(Self {
            anchor: Some(chain.short(anchor)?),
            rows,
        })
    }

    /// The oldest version the sheet stands for, as it was written.
    #[must_use]
    pub fn anchor(&self) -> Option<&str> {
        self.anchor.as_deref()
    }

    /// One row a version, the version the entry records first.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Whether the sheet holds nothing, which is a run with nothing to apply.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Reads a sheet out of the text an editor, or a plan file, holds.
    ///
    /// Comments start with `#` and are skipped, `>>> LOCAL` marks the version the entry records,
    /// and the other `>>>` line names the oldest version the sheet stands for.
    ///
    /// # Errors
    ///
    /// Returns [`SheetError`] when a marker names nothing, a row does not hold the fields it
    /// needs, or the sheet names neither end.
    pub fn parse(text: &str) -> Result<Self, SheetError> {
        let mut anchor = None;
        let mut rows = Vec::new();
        let mut marked = false;

        for (at, line) in text.lines().enumerate() {
            let number = at + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix(">>>") {
                let rest = rest.trim();
                if rest.is_empty() {
                    return Err(SheetError::new(number, "a marker names nothing"));
                }
                if rest.eq_ignore_ascii_case("LOCAL") {
                    marked = true;
                } else {
                    anchor = Some(rest.to_owned());
                }
                continue;
            }

            let (mark, rest) = trimmed.strip_prefix("--").map_or_else(
                || {
                    trimmed
                        .strip_prefix("++")
                        .map_or((Mark::Kept, trimmed), |rest| (Mark::Insert, rest.trim()))
                },
                |rest| (Mark::Remove, rest.trim()),
            );

            // What a removed row says is not read. The version is leaving, and a person who
            // writes the marker in front of the row the tool wrote cannot lose the row to a typo
            // in a field that is about to mean nothing.
            if mark == Mark::Remove {
                rows.push(Row {
                    mark,
                    creator: String::new(),
                    message: String::new(),
                    content: String::new(),
                    join: None,
                });
                continue;
            }

            let words = words(rest).map_err(|what| SheetError::new(number, what))?;
            let row = row_of(mark, words).map_err(|what| SheetError::new(number, what))?;
            rows.push(row);
        }

        if !marked {
            return Err(SheetError::new(
                1,
                "the sheet does not say >>> LOCAL anywhere",
            ));
        }
        if anchor.is_none() {
            return Err(SheetError::new(
                1,
                "the sheet does not say what version it stands down to",
            ));
        }

        Ok(Self { anchor, rows })
    }

    /// The sheet as text, with `comments` written above it as `#` lines.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Sheet`] when the sheet does not fit the chain it names.
    pub fn render(&self, chain: &Chain, comments: &[&str]) -> Result<String, TweakRefusal> {
        let bound = bind(self, chain)?;
        let mut cells: Vec<[String; 4]> = Vec::with_capacity(bound.entries.len());

        for entry in &bound.entries {
            cells.push(cell_of(entry, chain)?);
        }

        let mut widths = [0_usize; 4];
        for cell in &cells {
            for (at, part) in cell.iter().enumerate() {
                widths[at] = widths[at].max(visible(part).min(WIDEST));
            }
        }

        let mut text = String::new();
        for comment in comments {
            text.push_str("# ");
            text.push_str(comment);
            text.push('\n');
        }
        text.push_str(">>> LOCAL\n");
        for cell in &cells {
            for (at, part) in cell.iter().enumerate() {
                text.push_str(part);
                if at + 1 < cell.len() {
                    let pad = widths[at].saturating_sub(visible(part)) + 2;
                    text.extend(std::iter::repeat_n(' ', pad));
                }
            }
            text.push('\n');
        }
        text.push_str(">>> ");
        text.push_str(&chain.short(bound.anchor)?);
        text.push('\n');

        Ok(text)
    }
}

/// The four cells one row is drawn as.
fn cell_of(entry: &Entry<'_>, chain: &Chain) -> Result<[String; 4], TweakRefusal> {
    let level = entry
        .standing
        .map(|number| chain.level(number))
        .transpose()?;

    let (creator, message, content, join) = match entry.row.mark {
        // A removed row is drawn from the version it stands at, so the row a person reads is the
        // one they are removing rather than the empty one a marker leaves.
        Mark::Remove => {
            let level = level.ok_or_else(|| TweakRefusal::Sheet {
                reason: "a removed row stands at no version".to_owned(),
            })?;
            (
                level.creator.clone(),
                level.message.clone(),
                short(level.variant.storage_hash()),
                level
                    .variant
                    .join()
                    .map_or_else(String::new, |join| format!("+{}", short(join))),
            )
        }
        Mark::Kept | Mark::Insert => (
            entry.row.creator.clone(),
            entry.row.message.clone(),
            entry.row.content.clone(),
            entry.row.join.clone().unwrap_or_default(),
        ),
    };

    let creator = match entry.row.mark {
        Mark::Kept => word(&creator),
        Mark::Remove => format!("--{}", word(&creator)),
        Mark::Insert => format!("++{}", word(&creator)),
    };

    Ok([creator, word(&message), content, join])
}

/// The words of a row, with quoted words allowed to hold spaces.
///
/// # Errors
///
/// Returns a sentence saying what is wrong when a quote is left open.
fn words(line: &str) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = line.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                word.push('"');
                started = true;
            }
            '"' => {
                quoted = !quoted;
                started = true;
            }
            character if character.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            character => {
                word.push(character);
                started = true;
            }
        }
    }

    if quoted {
        return Err("a quoted word is not closed");
    }
    if started {
        words.push(word);
    }

    Ok(words)
}

/// The row four words describe.
///
/// # Errors
///
/// Returns a sentence saying what is wrong when a field is missing or a merge is written without
/// the marker a merge is written with.
fn row_of(mark: Mark, mut words: Vec<String>) -> Result<Row, &'static str> {
    // A version with no merge writes no merge column, so the column the table ends on is allowed
    // to be empty.
    if words.len() == 3 {
        words.push(String::new());
    }

    let Ok([creator, message, content, join]) = <[String; 4]>::try_from(words) else {
        return Err("a row holds a creator, a message, a content and a merge");
    };

    let join = match join.as_str() {
        "" | "-" => None,
        join => Some(join.to_owned()),
    };
    if join.as_deref().is_some_and(|join| !join.starts_with('+')) {
        return Err("a merge is written with a + in front of it");
    }

    Ok(Row {
        mark,
        creator,
        message,
        content,
        join,
    })
}

/// The characters of `text` a reader sees, which is what a column is padded to.
fn visible(text: &str) -> usize {
    text.chars().count()
}

/// A word written so that [`words`] reads it back whole.
fn word(text: &str) -> String {
    if text.is_empty() || text.chars().any(char::is_whitespace) || text.contains('"') {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

// ---------------------------------------------------------------------------------------------
// The chain
// ---------------------------------------------------------------------------------------------

/// One version of a chain: the variant that produced it, and what the two say.
#[derive(Debug, Clone)]
pub struct Level {
    /// The version itself.
    version: Version,
    /// The variant the version points at, which is what produced it.
    variant: Variant,
    /// Who made it, as text.
    creator: String,
    /// What it says, as text.
    message: String,
}

/// A file's chain, the oldest version first, numbered the way the chain is.
#[derive(Debug, Clone)]
pub struct Chain {
    /// One level a version, the oldest first, so a level's position is its number.
    levels: Vec<Level>,
}

/// One version as a listing names it.
#[derive(Debug, Clone)]
pub struct Found {
    /// The number of the version in its chain.
    pub number: usize,
    /// The version's hash, as hex.
    pub version: String,
    /// The variant's hash, as hex.
    pub variant: String,
    /// Who made it, as text.
    pub creator: String,
    /// What it says, as text.
    pub message: String,
}

impl Chain {
    /// Reads the chain the version `top` is the top of.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Failed`] when an object the chain names cannot be read.
    pub fn read(index: &VCSIndex, top: &Version) -> Result<Self, TweakRefusal> {
        let mut levels = Vec::new();
        let mut version = top.clone();

        while !version.is_root() {
            let variant = index
                .read_variant(&Key::new(*version.variant()).to_string())
                .map_err(index_failed)?;
            let creator = index
                .read_creator(&Key::new(*variant.creator()).to_string())
                .map_err(index_failed)?;
            let message = index
                .read_message(&Key::new(*variant.message()).to_string())
                .map_err(index_failed)?;
            let base = *variant.base_version();

            levels.push(Level {
                version,
                variant,
                creator: creator.read_to_string().to_owned(),
                message: message.read_to_string().to_owned(),
            });

            if base == [0; 32] {
                break;
            }
            version = index
                .read_version(&Key::new(base).to_string())
                .map_err(index_failed)?;
        }

        levels.reverse();

        Ok(Self { levels })
    }

    /// How many versions the chain holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.levels.len()
    }

    /// Whether the chain holds no version.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    /// The number of the version the entry records.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Nothing`] when the chain holds no version.
    pub fn top(&self) -> Result<usize, TweakRefusal> {
        self.levels
            .len()
            .checked_sub(1)
            .ok_or(TweakRefusal::Nothing)
    }

    /// The level of one version.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Nothing`] when the chain is shorter than `number`.
    pub fn level(&self, number: usize) -> Result<&Level, TweakRefusal> {
        self.levels.get(number).ok_or(TweakRefusal::Nothing)
    }

    /// One version, as a listing names it.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Nothing`] when the chain is shorter than `number`.
    pub fn found(&self, number: usize) -> Result<Found, TweakRefusal> {
        let level = self.level(number)?;

        Ok(Found {
            number,
            version: Key::new(*level.version.hash().digest()).hex(),
            variant: Key::new(*level.variant.hash().digest()).hex(),
            creator: level.creator.clone(),
            message: level.message.clone(),
        })
    }

    /// A version named for a person: its number, and the head of its hash.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Nothing`] when the chain is shorter than `number`.
    pub fn name(&self, number: usize) -> Result<String, TweakRefusal> {
        Ok(named(&self.found(number)?.version, number))
    }

    /// The name of one version: the head of its hash.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Nothing`] when the chain is shorter than `number`.
    pub fn short(&self, number: usize) -> Result<String, TweakRefusal> {
        Ok(short(self.level(number)?.version.hash().digest()))
    }

    /// The versions of the chain, as keys.
    fn version_keys(&self) -> Vec<Key> {
        self.levels
            .iter()
            .map(|level| Key::new(*level.version.hash().digest()))
            .collect()
    }

    /// The number of the version `text` names among this chain's own.
    fn number_of(&self, text: &str) -> Result<usize, HeadMiss> {
        let keys = self.version_keys();
        let key = resolve_head(text, &keys)?;
        keys.iter()
            .position(|candidate| *candidate == key)
            .ok_or(HeadMiss::Unknown)
    }

    /// The versions whose names start with the head `text` writes.
    fn versions_naming(&self, text: &str) -> Vec<usize> {
        let Ok(head) = ShortHash::new(text) else {
            return Vec::new();
        };

        self.levels
            .iter()
            .enumerate()
            .filter(|(_, level)| head.matches(&Key::new(*level.version.hash().digest())))
            .map(|(number, _)| number)
            .collect()
    }
}

/// The hash of a digest, as a sheet names it.
fn short(digest: &Blake3Hash) -> String {
    let hex = Key::new(*digest).hex();

    hex[..hex.len().min(SHORT)].to_owned()
}

/// A name of a version for a person: its number when it is known, and the head of its hash.
fn named(version: &str, number: usize) -> String {
    let head = &version[..version.len().min(HEAD)];

    format!("V{number} ({head})")
}

// ---------------------------------------------------------------------------------------------
// Reading a sheet against a chain
// ---------------------------------------------------------------------------------------------

/// One row of a sheet, with the version it stands for.
#[derive(Debug, Clone, Copy)]
struct Entry<'a> {
    /// The row itself.
    row: &'a Row,
    /// The number of the version it stands for, which is nothing for an added row.
    standing: Option<usize>,
}

/// A sheet read against a chain: its rows, and the oldest version they reach.
struct Bound<'a> {
    /// One row a version, the version the entry records first.
    entries: Vec<Entry<'a>>,
    /// The number of the oldest version the sheet stands for.
    anchor: usize,
}

/// Reads `sheet` against `chain`, settling which version each row stands for.
///
/// # Errors
///
/// Returns [`TweakRefusal::Sheet`] when the sheet names a version the chain does not hold, holds
/// more rows than its range has versions, stops short of the version it names, or leaves an added
/// row with no version below it to stand from.
fn bind<'a>(sheet: &'a Sheet, chain: &Chain) -> Result<Bound<'a>, TweakRefusal> {
    let top = chain.top()?;
    let Some(anchor) = sheet.anchor.as_deref() else {
        return Err(SheetError::new(1, "the sheet names no oldest version").into());
    };
    let anchor = chain
        .number_of(anchor)
        .map_err(|_| SheetError::new(1, "the oldest version is not on this chain"))?;
    if anchor > top {
        return Err(SheetError::new(1, "the oldest version is above the newest").into());
    }

    let mut entries: Vec<Entry<'a>> = Vec::with_capacity(chain.len());
    let mut at = Some(top);

    for row in &sheet.rows {
        match row.mark {
            Mark::Insert => entries.push(Entry {
                row,
                standing: None,
            }),
            Mark::Kept | Mark::Remove => {
                let Some(number) = at else {
                    return Err(SheetError::new(
                        1,
                        "the sheet holds more rows than its range has versions",
                    )
                    .into());
                };
                entries.push(Entry {
                    row,
                    standing: Some(number),
                });
                at = number.checked_sub(1);
            }
        }
    }

    let reached = at.map_or(0, |at| at + 1);
    if reached != anchor {
        return Err(SheetError::new(1, "the sheet stops short of the version it names").into());
    }

    // An added row is placed between two versions, so the sheet has to end on a version: a row
    // added under the oldest one would stand below the range it was written into.
    if entries.last().is_some_and(|entry| entry.standing.is_none()) {
        return Err(SheetError::new(1, "an added row stands below the oldest version").into());
    }

    Ok(Bound { entries, anchor })
}

/// Why a hash named no one object.
#[derive(Debug)]
enum HeadMiss {
    /// It is neither a whole key nor the head of one.
    Malformed,
    /// Nothing the run can reach starts with it.
    Unknown,
    /// More than one thing does.
    Ambiguous,
}

/// Reads `text` as a whole key, or as the head of one among `candidates`.
fn resolve_head(text: &str, candidates: &[Key]) -> Result<Key, HeadMiss> {
    if let Ok(key) = Key::from_str(text) {
        return Ok(key);
    }

    let Ok(head) = ShortHash::new(text) else {
        return Err(HeadMiss::Malformed);
    };

    match head.resolve(candidates) {
        Resolved::ExactlyOne(key) => Ok(key),
        Resolved::NoMatch => Err(HeadMiss::Unknown),
        Resolved::Ambiguous(_) => Err(HeadMiss::Ambiguous),
    }
}

/// Whether `text` names `digest`, whether it is the whole key or the head of one.
fn names(text: &str, digest: &Blake3Hash) -> bool {
    if let Ok(key) = Key::from_str(text) {
        return key.digest() == digest;
    }

    ShortHash::new(text).is_ok_and(|head| head.matches(&Key::new(*digest)))
}

// ---------------------------------------------------------------------------------------------
// Naming the version to adjust
// ---------------------------------------------------------------------------------------------

/// Why a run named no version it can adjust.
#[derive(Debug)]
pub enum TargetMiss {
    /// Nothing on the chain, and nothing the index holds, matches what was written.
    NoMatch {
        /// What was written.
        target: String,
    },
    /// More than one version of the chain matches.
    Ambiguous {
        /// What was written.
        target: String,
        /// The versions it matches, as numbers.
        candidates: Vec<usize>,
    },
    /// The hash names an object that is not on this chain.
    Elsewhere {
        /// What was written.
        target: String,
    },
    /// A step count reaches past the first version.
    OutOfChain {
        /// What was written.
        target: String,
    },
    /// The version is one the plan already removes.
    Removed {
        /// The version, as a name for a person.
        version: String,
    },
}

impl fmt::Display for TargetMiss {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMatch { target } => write!(formatter, "nothing matches {target}"),
            Self::Ambiguous { target, .. } => write!(formatter, "{target} matches more than one"),
            Self::Elsewhere { target } => write!(formatter, "{target} belongs to another chain"),
            Self::OutOfChain { target } => {
                write!(formatter, "{target} reaches past the first version")
            }
            Self::Removed { version } => write!(formatter, "{version} is being removed"),
        }
    }
}

impl std::error::Error for TargetMiss {}

/// The version a run names, by its number on the chain.
///
/// `~N` is N versions back from the one the entry records, a hash is a version or the variant
/// that produced one, and anything else is text matched against the creators and the messages,
/// which names a version only when it names one.
///
/// # Errors
///
/// Returns [`TargetMiss`] when the target names no version, or more than one.
pub fn resolve(chain: &Chain, target: Option<&str>) -> Result<usize, TargetMiss> {
    let top = chain.top().map_err(|_| TargetMiss::NoMatch {
        target: target.unwrap_or_default().to_owned(),
    })?;
    let Some(text) = target else {
        return Ok(top);
    };

    if let Some(steps) = text.strip_prefix('~') {
        let steps: usize = steps.parse().map_err(|_| TargetMiss::NoMatch {
            target: text.to_owned(),
        })?;

        return top
            .checked_sub(steps)
            .ok_or_else(|| TargetMiss::OutOfChain {
                target: text.to_owned(),
            });
    }

    match chain.number_of(text) {
        Ok(number) => return Ok(number),
        Err(HeadMiss::Ambiguous) => {
            return Err(TargetMiss::Ambiguous {
                target: text.to_owned(),
                candidates: chain.versions_naming(text),
            });
        }
        Err(HeadMiss::Malformed | HeadMiss::Unknown) => {}
    }

    // The variant that produced a version names that version, which is what a run that read the
    // hash off a drawing means.
    let variants: Vec<Key> = chain
        .levels
        .iter()
        .map(|level| Key::new(*level.variant.hash().digest()))
        .collect();
    if let Ok(variant) = resolve_head(text, &variants)
        && let Some(number) = variants.iter().position(|candidate| *candidate == variant)
    {
        return Ok(number);
    }

    // A word that is neither a hash nor the head of one is text, and so is a head that names
    // nothing here: a message is allowed to look like a hash.
    let needle = text.to_lowercase();
    let hits: Vec<usize> = chain
        .levels
        .iter()
        .enumerate()
        .filter(|(_, level)| {
            level.creator.to_lowercase().contains(&needle)
                || level.message.to_lowercase().contains(&needle)
        })
        .map(|(number, _)| number)
        .collect();

    match hits.len() {
        0 => Err(if is_a_hash(text) {
            TargetMiss::Elsewhere {
                target: text.to_owned(),
            }
        } else {
            TargetMiss::NoMatch {
                target: text.to_owned(),
            }
        }),
        1 => Ok(hits[0]),
        _ => Err(TargetMiss::Ambiguous {
            target: text.to_owned(),
            candidates: hits,
        }),
    }
}

/// Whether `text` reads as a hash, whole or as a head, rather than as a word.
fn is_a_hash(text: &str) -> bool {
    Key::from_str(text).is_ok() || ShortHash::new(text).is_ok()
}

/// What a miss at naming a head is told as.
fn miss_of(text: &str, miss: &HeadMiss) -> TargetMiss {
    match miss {
        HeadMiss::Malformed => TargetMiss::NoMatch {
            target: text.to_owned(),
        },
        HeadMiss::Unknown => TargetMiss::Elsewhere {
            target: text.to_owned(),
        },
        HeadMiss::Ambiguous => TargetMiss::Ambiguous {
            target: text.to_owned(),
            candidates: Vec::new(),
        },
    }
}

// ---------------------------------------------------------------------------------------------
// Adjusting a sheet
// ---------------------------------------------------------------------------------------------

/// The fields of a version a run sets, all of which are optional since a run may set any of them.
#[derive(Debug, Clone, Default)]
pub struct Adjustment {
    /// The creator to set, as text.
    pub creator: Option<String>,
    /// The message to set, as text.
    pub message: Option<String>,
    /// The content to point at, as a key or the head of one.
    pub content: Option<String>,
    /// The merge to set, or to take away when it is `Some(None)`.
    pub join: Option<Option<String>>,
}

impl Adjustment {
    /// Whether the run said anything at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.creator.is_none()
            && self.message.is_none()
            && self.content.is_none()
            && self.join.is_none()
    }
}

/// What a run does to one place in a sheet.
#[derive(Debug, Clone)]
pub enum Change {
    /// Set some fields of the version there.
    Fields(Adjustment),
    /// Add a version above the version there, a copy of it.
    Insert,
    /// Take the version there away.
    Drop,
}

/// One row of a sheet under construction, with the version it stands at.
type Item = (Option<usize>, Row);

/// Changes `sheet` at `target`, and answers the sheet that covers what it now says.
///
/// `sheet` is the plan a run already has, if it has one. A target older than the plan's range
/// widens the range: the rows between are the versions' own, changing nothing.
///
/// # Errors
///
/// Returns [`TweakRefusal::Sheet`] when the sheet `sheet` does not fit the chain, and
/// [`TweakRefusal::Target`] when the version cannot take the change.
pub fn adjust(
    chain: &Chain,
    sheet: Option<&Sheet>,
    target: usize,
    change: &Change,
) -> Result<Sheet, TweakRefusal> {
    let mut items: Vec<Item> = match sheet {
        Some(sheet) if !sheet.is_empty() => {
            let mut items: Vec<Item> = Vec::new();
            for entry in &bind(sheet, chain)?.entries {
                let row = (*entry.row).clone();
                // The fields of a removed row are the version's own, so a row that lost them to
                // an edit is filled in again here rather than being carried as empty.
                let row = match (row.mark, entry.standing) {
                    (Mark::Remove, Some(number)) => Row {
                        mark: Mark::Remove,
                        ..Row::kept(chain.level(number)?)
                    },
                    _ => row,
                };
                items.push((entry.standing, row));
            }
            items
        }
        _ => {
            let anchor = target.min(chain.top()?);
            (anchor..=chain.top()?)
                .rev()
                .map(|number| Ok((Some(number), Row::kept(chain.level(number)?))))
                .collect::<Result<Vec<_>, TweakRefusal>>()?
        }
    };

    // The target may stand below everything the sheet reaches, in which case the rows between are
    // the chain's own and the range grows down to it.
    let bottom = items
        .last()
        .and_then(|(number, _)| *number)
        .ok_or_else(|| SheetError::new(1, "the sheet ends on an added row"))?;
    if target < bottom {
        for number in (target..bottom).rev() {
            items.push((Some(number), Row::kept(chain.level(number)?)));
        }
    }

    let at = items
        .iter()
        .position(|(number, _)| *number == Some(target))
        .ok_or_else(|| {
            TweakRefusal::Target(TargetMiss::NoMatch {
                target: chain.short(target).unwrap_or_default(),
            })
        })?;

    match change {
        Change::Fields(adjustment) => {
            if items[at].1.mark == Mark::Remove {
                return Err(TweakRefusal::Target(TargetMiss::Removed {
                    version: chain.name(target)?,
                }));
            }
            let row = &mut items[at].1;
            if let Some(creator) = &adjustment.creator {
                row.creator.clone_from(creator);
            }
            if let Some(message) = &adjustment.message {
                row.message.clone_from(message);
            }
            if let Some(content) = &adjustment.content {
                row.content.clone_from(content);
            }
            if let Some(join) = &adjustment.join {
                row.join.clone_from(join);
            }
        }
        Change::Insert => {
            let row = Row {
                mark: Mark::Insert,
                ..items[at].1.clone()
            };
            items.insert(at, (None, row));
        }
        Change::Drop => {
            items[at].1.mark = Mark::Remove;
        }
    }

    narrowed(chain, &items)
}

/// The sheet covering `items` down to the lowest row that differs from the chain.
///
/// # Errors
///
/// Returns [`TweakRefusal::Sheet`] when the lowest difference is an added row with no version
/// below it to stand from.
fn narrowed(chain: &Chain, items: &[Item]) -> Result<Sheet, TweakRefusal> {
    let lowest = items
        .iter()
        .rposition(|(number, row)| number.is_none_or(|number| differs(chain, number, row)));

    let Some(lowest) = lowest else {
        return Ok(Sheet {
            anchor: None,
            rows: Vec::new(),
        });
    };

    // The range is named by the oldest version a row stands at. An added row is how a version
    // above it came to exist, so the row below it is kept to name the range even when that row
    // changes nothing.
    let end = if items[lowest].0.is_some() {
        lowest
    } else {
        lowest + 1
    };
    let anchor = items
        .get(end)
        .and_then(|(number, _)| *number)
        .ok_or_else(|| SheetError::new(1, "an added row has no version below it"))?;

    let rows: Vec<Row> = items[..=end].iter().map(|(_, row)| row.clone()).collect();

    Ok(Sheet {
        anchor: Some(chain.short(anchor)?),
        rows,
    })
}

/// Whether `row` says anything different about the version `number` than the chain does.
fn differs(chain: &Chain, number: usize, row: &Row) -> bool {
    let Ok(level) = chain.level(number) else {
        return true;
    };

    match row.mark {
        Mark::Insert | Mark::Remove => true,
        Mark::Kept => {
            row.creator != level.creator
                || row.message != level.message
                || !names(&row.content, level.variant.storage_hash())
                || !same_join(row.join.as_deref(), level.variant.join())
        }
    }
}

/// Whether a row's merge cell says what the version's merge is.
fn same_join(written: Option<&str>, held: Option<&Blake3Hash>) -> bool {
    match (written, held) {
        (None, None) => true,
        (Some(text), Some(digest)) => names(text.trim_start_matches('+'), digest),
        _ => false,
    }
}

/// One adjustment a sheet makes, and what undoing it takes.
#[derive(Debug, Clone)]
struct Planned {
    /// What the adjustment is, as a listing names it.
    operation: Operation,
    /// Which row, and how, undoing it goes back.
    undo: Undo,
}

/// What undoing one adjustment takes.
#[derive(Debug, Clone, Copy)]
enum Undo {
    /// The row at this place is given the chain's fields back.
    Fields(usize),
    /// The added row at this place goes.
    Insert(usize),
    /// The removed row at this place stays.
    Remove(usize),
}

/// What a sheet would do to its chain, oldest first.
///
/// # Errors
///
/// Returns [`TweakRefusal::Sheet`] when the sheet does not fit the chain.
pub fn operations(chain: &Chain, sheet: &Sheet) -> Result<Vec<Operation>, TweakRefusal> {
    // A sheet with nothing in it is a plan that was just cancelled away, which is nothing to list
    // rather than a sheet that does not fit the chain.
    if sheet.is_empty() {
        return Ok(Vec::new());
    }

    Ok(planned(chain, sheet)?
        .into_iter()
        .map(|planned| planned.operation)
        .collect())
}

/// What a sheet would do to its chain, oldest first, with what undoing each takes.
fn planned(chain: &Chain, sheet: &Sheet) -> Result<Vec<Planned>, TweakRefusal> {
    let bound = bind(sheet, chain)?;

    // An added row names the versions around it, which are the nearest rows that stand at one.
    let mut above = Vec::with_capacity(bound.entries.len());
    let mut standing = chain.top()?;
    for entry in &bound.entries {
        above.push(standing);
        if let Some(number) = entry.standing {
            standing = number;
        }
    }
    let mut below = vec![None; bound.entries.len()];
    let mut seen = None;
    for (at, entry) in bound.entries.iter().enumerate().rev() {
        below[at] = seen;
        if let Some(number) = entry.standing {
            seen = Some(number);
        }
    }

    let mut planned = Vec::new();
    for (at, entry) in bound.entries.iter().enumerate().rev() {
        let row = entry.row;
        match (row.mark, entry.standing) {
            (Mark::Remove, Some(number)) => planned.push(Planned {
                operation: Operation::Remove {
                    version: chain.name(number)?,
                },
                undo: Undo::Remove(at),
            }),
            (Mark::Insert, None) => planned.push(Planned {
                operation: Operation::Insert {
                    below: chain.name(below[at].unwrap_or(bound.anchor))?,
                    above: chain.name(above[at])?,
                },
                undo: Undo::Insert(at),
            }),
            (Mark::Kept, Some(number)) => {
                let level = chain.level(number)?;
                let name = chain.name(number)?;

                if row.creator != level.creator {
                    planned.push(Planned {
                        operation: Operation::Creator {
                            version: name.clone(),
                            text: row.creator.clone(),
                        },
                        undo: Undo::Fields(at),
                    });
                }
                if row.message != level.message {
                    planned.push(Planned {
                        operation: Operation::Message {
                            version: name.clone(),
                            text: row.message.clone(),
                        },
                        undo: Undo::Fields(at),
                    });
                }
                if !names(&row.content, level.variant.storage_hash()) {
                    planned.push(Planned {
                        operation: Operation::Content {
                            version: name.clone(),
                            hash: row.content.clone(),
                        },
                        undo: Undo::Fields(at),
                    });
                }
                match (row.join.as_deref(), level.variant.join()) {
                    (None, Some(join)) => planned.push(Planned {
                        operation: Operation::Unjoin {
                            version: name,
                            variant: short(join),
                        },
                        undo: Undo::Fields(at),
                    }),
                    (Some(join), None) => planned.push(Planned {
                        operation: Operation::Join {
                            version: name,
                            variant: join.trim_start_matches('+').to_owned(),
                        },
                        undo: Undo::Fields(at),
                    }),
                    (Some(join), Some(held)) if !names(join.trim_start_matches('+'), held) => {
                        planned.push(Planned {
                            operation: Operation::Join {
                                version: name,
                                variant: join.trim_start_matches('+').to_owned(),
                            },
                            undo: Undo::Fields(at),
                        });
                    }
                    _ => {}
                }
            }
            // A kept row without a version, or a removed or added one with a version, is what
            // `bind` refuses before this is reached.
            (Mark::Kept | Mark::Remove, None) | (Mark::Insert, Some(_)) => {}
        }
    }

    Ok(planned)
}

/// One adjustment a sheet makes, as a listing names it.
#[derive(Debug, Clone)]
pub enum Operation {
    /// A version's creator changes.
    Creator {
        /// The version, as a name for a person.
        version: String,
        /// The creator it is given.
        text: String,
    },
    /// A version's message changes.
    Message {
        /// The version, as a name for a person.
        version: String,
        /// The message it is given.
        text: String,
    },
    /// A version is pointed at other content.
    Content {
        /// The version, as a name for a person.
        version: String,
        /// The content it is pointed at.
        hash: String,
    },
    /// A version is joined with a variant.
    Join {
        /// The version, as a name for a person.
        version: String,
        /// The variant it is joined with.
        variant: String,
    },
    /// A version is no longer joined with its variant.
    Unjoin {
        /// The version, as a name for a person.
        version: String,
        /// The variant it was joined with.
        variant: String,
    },
    /// A version is added between two others.
    Insert {
        /// The version below the new one.
        below: String,
        /// The version above it.
        above: String,
    },
    /// A version is taken away.
    Remove {
        /// The version, as a name for a person.
        version: String,
    },
}

/// Undoes the `number`th of what a sheet would do, counting from one as a listing does.
///
/// # Errors
///
/// Returns [`TweakRefusal::Sheet`] when the sheet does not fit the chain, and
/// [`TweakRefusal::Target`] when `number` names no adjustment.
pub fn cancel(
    chain: &Chain,
    sheet: &Sheet,
    number: usize,
) -> Result<(Sheet, Operation), TweakRefusal> {
    let planned = planned(chain, sheet)?;
    let Some(planned) = planned.get(number.saturating_sub(1)).cloned() else {
        return Err(TweakRefusal::Target(TargetMiss::NoMatch {
            target: number.to_string(),
        }));
    };

    let mut items: Vec<Item> = bind(sheet, chain)?
        .entries
        .iter()
        .map(|entry| (entry.standing, (*entry.row).clone()))
        .collect();

    match planned.undo {
        Undo::Fields(at) => {
            let number = items[at].0.ok_or_else(|| {
                TweakRefusal::Target(TargetMiss::NoMatch {
                    target: planned.operation.to_string(),
                })
            })?;
            let kept = Row::kept(chain.level(number)?);
            items[at].1 = kept;
        }
        Undo::Insert(at) => {
            items.remove(at);
        }
        Undo::Remove(at) => {
            if let Some(number) = items[at].0 {
                items[at].1 = Row::kept(chain.level(number)?);
            }
        }
    }

    Ok((narrowed(chain, &items)?, planned.operation))
}

// ---------------------------------------------------------------------------------------------
// The plan a Workspace keeps
// ---------------------------------------------------------------------------------------------

/// The directory a plan for `id` at `local` is kept in.
#[must_use]
pub fn plan_dir(root: &Path, id: Uuid, local: &Blake3Hash) -> PathBuf {
    root.join(WORKSPACE_TWEAKING_DIR)
        .components()
        .collect::<PathBuf>()
        .join(id.to_string())
        .join(Key::new(*local).hex())
}

/// Reads the plan a run has waiting for `id` at `local`.
///
/// `None` is a run with nothing waiting, which is what a plan written against another version
/// leaves behind: the path is keyed by the version, so the two cannot be mixed up.
///
/// # Errors
///
/// Returns [`TweakRefusal::Sheet`] when the file is there and cannot be read, and
/// [`TweakRefusal::Failed`] when it cannot be opened.
pub fn read_plan(root: &Path, id: Uuid, local: &Blake3Hash) -> Result<Option<Sheet>, TweakRefusal> {
    let path = plan_dir(root, id, local).join(PLAN_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(TweakRefusal::Failed(WriteError::Io {
                path: path.display().to_string(),
                reason: error.to_string(),
            }));
        }
    };

    Ok(Some(Sheet::parse(&text)?))
}

/// Writes the plan a run has waiting for `id` at `local`.
///
/// # Errors
///
/// Returns [`TweakRefusal::Failed`] when the directory or the file cannot be written.
pub fn write_plan(
    root: &Path,
    id: Uuid,
    local: &Blake3Hash,
    sheet: &Sheet,
    chain: &Chain,
    comments: &[&str],
) -> Result<(), TweakRefusal> {
    let directory = plan_dir(root, id, local);
    if let Err(error) = fs::create_dir_all(&directory) {
        return Err(TweakRefusal::Failed(WriteError::Io {
            path: directory.display().to_string(),
            reason: error.to_string(),
        }));
    }

    let path = directory.join(PLAN_FILE);
    if let Err(error) = fs::write(&path, sheet.render(chain, comments)?) {
        return Err(TweakRefusal::Failed(WriteError::Io {
            path: path.display().to_string(),
            reason: error.to_string(),
        }));
    }

    Ok(())
}

/// Takes away the plan a run has waiting for `id` at `local`.
///
/// The version's directory goes, and the `Uuid`'s with it once nothing waits under it. A plan a
/// run cannot take away is said rather than passed over: it would be applied again.
///
/// # Errors
///
/// Returns [`TweakRefusal::Failed`] when a directory cannot be removed.
pub fn remove_plan(root: &Path, id: Uuid, local: &Blake3Hash) -> Result<(), TweakRefusal> {
    let directory = plan_dir(root, id, local);
    if let Err(error) = fs::remove_dir_all(&directory)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(TweakRefusal::Failed(WriteError::Io {
            path: directory.display().to_string(),
            reason: error.to_string(),
        }));
    }

    let Some(parent) = directory.parent().map(Path::to_path_buf) else {
        return Ok(());
    };
    if fs::read_dir(&parent).is_ok_and(|mut entries| entries.next().is_none())
        && let Err(error) = fs::remove_dir(&parent)
    {
        return Err(TweakRefusal::Failed(WriteError::Io {
            path: parent.display().to_string(),
            reason: error.to_string(),
        }));
    }

    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Applying a sheet
// ---------------------------------------------------------------------------------------------

/// What an applied sheet works over.
pub struct Request<'a> {
    /// The Workspace being worked in.
    pub workspace: &'a Workspace,
    /// The Layout being worked in.
    pub layout: &'a Layout,
    /// The index the versions are read from and written to.
    pub index: Option<&'a VCSIndex>,
    /// The store the content is read from.
    pub store: Option<&'a StorageSet>,
    /// The account the run acts as, when it has one.
    pub account: Option<&'a Account>,
    /// Every Vault the run may bring content from.
    pub candidates: &'a [(String, VaultAddress)],
    /// Whether the run may reach a Vault at all.
    pub offline: bool,
    /// How many units of content a transfer carries at a time.
    pub concurrency: usize,
}

/// What applying a sheet came to.
#[derive(Debug, Clone)]
pub struct TweakOutcome {
    /// The version the entry recorded before.
    pub from: String,
    /// The version it records now.
    pub to: String,
    /// How many versions were written.
    pub rebuilt: usize,
    /// What the run could not do about the file on disk.
    pub warnings: Vec<TweakWarning>,
}

/// What an applied sheet could not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweakWarning {
    /// The content the entry now records is not here, and could not be brought, so the file still
    /// holds the old version's content and stands modified.
    ContentUnavailable,
}

/// Why a sheet could not be applied.
#[derive(Debug)]
pub enum TweakRefusal {
    /// The Layout records no entry at the path.
    NotRecorded,
    /// The entry records no version.
    Nothing,
    /// The entry records a version that is not the top of a chain.
    NotAChain,
    /// The entry names a tree rather than a file.
    Tree,
    /// A variant is waiting to be joined into the file.
    Merging {
        /// The variant that is waiting, as hex.
        variant: String,
    },
    /// The target names no version the run can adjust.
    Target(TargetMiss),
    /// A sheet cannot be read, or does not fit the chain it names.
    Sheet {
        /// Why.
        reason: String,
    },
    /// The run stopped for a reason this build does not name.
    Failed(WriteError),
}

impl fmt::Display for TweakRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRecorded => formatter.write_str("the Layout records no entry at the path"),
            Self::Nothing => formatter.write_str("the entry records no version"),
            Self::NotAChain => formatter.write_str("the version is not the top of a chain"),
            Self::Tree => formatter.write_str("the entry names a tree rather than a file"),
            Self::Merging { variant } => {
                write!(formatter, "the variant {variant} is waiting to be joined")
            }
            Self::Target(miss) => write!(formatter, "{miss}"),
            Self::Sheet { reason } => formatter.write_str(reason),
            Self::Failed(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TweakRefusal {}

impl From<WriteError> for TweakRefusal {
    fn from(error: WriteError) -> Self {
        Self::Failed(error)
    }
}

impl From<SheetError> for TweakRefusal {
    fn from(error: SheetError) -> Self {
        Self::Sheet {
            reason: error.to_string(),
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Creator { version, text } => {
                write!(formatter, "set the creator of {version} to {text}")
            }
            Self::Message { version, text } => {
                write!(formatter, "set the message of {version} to {text}")
            }
            Self::Content { version, hash } => {
                write!(formatter, "point {version} at the content {hash}")
            }
            Self::Join { version, variant } => {
                write!(formatter, "join {version} with {variant}")
            }
            Self::Unjoin { version, variant } => {
                write!(formatter, "unjoin {version} from {variant}")
            }
            Self::Insert { below, above } => {
                write!(formatter, "insert a version between {below} and {above}")
            }
            Self::Remove { version } => write!(formatter, "remove {version}"),
        }
    }
}

/// The failure an index refusing is told as.
fn index_failed(error: impl fmt::Debug) -> TweakRefusal {
    TweakRefusal::Failed(WriteError::Index {
        reason: format!("{error:?}"),
    })
}

/// Reads the chain a path's entry records.
///
/// # Errors
///
/// Returns [`TweakRefusal::NotRecorded`] when the path names no entry, [`TweakRefusal::Nothing`]
/// when it records no version, [`TweakRefusal::Tree`] when it names a tree, and
/// [`TweakRefusal::Failed`] when the index cannot be read.
pub fn chain_of(
    index: &VCSIndex,
    layout: &Layout,
    path: &LayoutPath,
) -> Result<Chain, TweakRefusal> {
    let id = layout.id_of(path).ok_or(TweakRefusal::NotRecorded)?;
    let data = layout.entry(id).ok_or(TweakRefusal::NotRecorded)?;
    if data.kind() == EntryKind::Tree {
        return Err(TweakRefusal::Tree);
    }
    if data.version() == [0; 32] {
        return Err(TweakRefusal::Nothing);
    }

    let version = index
        .read_version(&Key::new(data.version()).to_string())
        .map_err(index_failed)?;

    Chain::read(index, &version).map_err(|refusal| match refusal {
        TweakRefusal::Nothing => TweakRefusal::NotAChain,
        other => other,
    })
}

/// Applies `sheet` to the chain the path's entry records.
///
/// The versions above the sheet's oldest one are written again, the merge of each carried over and
/// pointed at the replayed variant when that variant was replayed too, and the variants based on a
/// version in the range are replayed beside them so a merge names something that is still there.
/// The Layout is then made to record the replayed tail, and the file is brought up to it when it
/// stood as recorded: a file holding changes is left alone, and one whose content is not here is
/// left alone with a [`TweakWarning`].
///
/// # Errors
///
/// Returns [`TweakRefusal`] when the run may not adjust the file, and [`WriteError::Cancelled`]
/// inside [`TweakRefusal::Failed`] when `cancellation` was set.
pub fn apply(
    request: &Request<'_>,
    id: Uuid,
    path: &LayoutPath,
    sheet: &Sheet,
    progress: &Progress,
    cancellation: &Cancellation,
) -> Result<TweakOutcome, TweakRefusal> {
    let Some(index) = request.index else {
        return Err(WriteError::NoIndex.into());
    };
    let Some(data) = request.layout.entry(id) else {
        return Err(TweakRefusal::NotRecorded);
    };
    if data.kind() == EntryKind::Tree {
        return Err(TweakRefusal::Tree);
    }
    if data.version() == [0; 32] {
        return Err(TweakRefusal::Nothing);
    }

    let root = request.workspace.get_root();
    if let Some(pending) = Merging::read(root).at(id) {
        return Err(TweakRefusal::Merging {
            variant: Key::new(*pending.variant()).hex(),
        });
    }

    let top = index
        .read_version(&Key::new(data.version()).to_string())
        .map_err(index_failed)?;
    let chain = Chain::read(index, &top)?;
    let bound = bind(sheet, &chain)?;

    let mut creators: BTreeMap<Blake3Hash, Creator> = BTreeMap::new();
    let mut messages: BTreeMap<Blake3Hash, Message> = BTreeMap::new();
    let mut variants: Vec<Variant> = Vec::new();
    let mut versions: Vec<Version> = Vec::new();
    let mut map: HashMap<Blake3Hash, Blake3Hash> = HashMap::new();

    // What the new chain stands at before its first row, which is the version the range sits on.
    let mut base = if bound.anchor == 0 {
        *Version::root().hash().digest()
    } else {
        *chain.level(bound.anchor - 1)?.version.hash().digest()
    };

    let based = based_on(request, &chain, bound.anchor)?;

    for entry in bound.entries.iter().rev() {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled.into());
        }

        let row = entry.row;
        if row.mark != Mark::Remove {
            let level = entry
                .standing
                .map(|number| chain.level(number))
                .transpose()?;
            let creator = creator_hash(
                row.creator.as_str(),
                level.map(|level| (level.creator.as_str(), level.variant.creator())),
                &mut creators,
            )?;
            let message = message_hash(
                row.message.as_str(),
                level.map(|level| (level.message.as_str(), level.variant.message())),
                &mut messages,
            )?;
            let content = match level {
                Some(level) if names(&row.content, level.variant.storage_hash()) => {
                    *level.variant.storage_hash()
                }
                _ => *content_key(request, &row.content)?.digest(),
            };
            let join = join_of(request, row, level, &map)?;

            let variant = Variant::new_bare_variant(content, base, join, creator, message, 0);
            let version = variant.new_version();
            base = *version.hash().digest();

            if let Some(level) = level {
                map.insert(*level.variant.hash().digest(), *variant.hash().digest());
            }
            variants.push(variant);
            versions.push(version);
        }

        if let Some(number) = entry.standing
            && let Some(rows) = based.get(&number)
        {
            replay(rows, base, &mut map, &mut variants)?;
        }
    }

    let from = Key::new(data.version()).hex();
    let to = Key::new(base).hex();
    let rebuilt = versions.len();

    // Nothing is written until every object that would be written has been worked out, so a run
    // refused halfway through has left none of the chain it would have written behind.
    for creator in creators.values() {
        index.write_creator(creator).map_err(index_failed)?;
    }
    for message in messages.values() {
        index.write_message(message).map_err(index_failed)?;
    }
    for variant in &variants {
        index.write_variant(variant).map_err(index_failed)?;
    }
    for version in &versions {
        index.write_version(version).map_err(index_failed)?;
    }

    set_version(request.layout, id, base)?;

    let warnings = bring_up_to_date(request, path, &chain, base, progress)?;

    remove_plan(root, id, &data.version())?;

    Ok(TweakOutcome {
        from,
        to,
        rebuilt,
        warnings,
    })
}

/// The hash a creator is named by, writing it when the row says something new.
fn creator_hash(
    text: &str,
    held: Option<(&str, &Blake3Hash)>,
    written: &mut BTreeMap<Blake3Hash, Creator>,
) -> Result<Blake3Hash, TweakRefusal> {
    if let Some((said, hash)) = held
        && said == text
    {
        return Ok(*hash);
    }

    let creator = Creator::try_from(text.to_owned()).map_err(|error| WriteError::Precondition {
        what: "creator".to_owned(),
        reason: error.to_string(),
    })?;
    let hash = *creator.hash().digest();
    written.insert(hash, creator);

    Ok(hash)
}

/// The hash a message is named by, writing it when the row says something new.
fn message_hash(
    text: &str,
    held: Option<(&str, &Blake3Hash)>,
    written: &mut BTreeMap<Blake3Hash, Message>,
) -> Result<Blake3Hash, TweakRefusal> {
    if let Some((said, hash)) = held
        && said == text
    {
        return Ok(*hash);
    }

    let message = Message::try_from(text.to_owned()).map_err(|error| WriteError::Precondition {
        what: "message".to_owned(),
        reason: error.to_string(),
    })?;
    let hash = *message.hash().digest();
    written.insert(hash, message);

    Ok(hash)
}

/// The key a content cell names.
fn content_key(request: &Request<'_>, text: &str) -> Result<Key, TweakRefusal> {
    if let Ok(key) = Key::from_str(text) {
        return Ok(key);
    }

    let Some(store) = request.store else {
        return Err(WriteError::NoStore.into());
    };
    let runtime = runtime()?;
    let keys = runtime
        .block_on(store.list_all_keys())
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })?;

    match resolve_head(text, &keys) {
        Ok(key) => Ok(key),
        Err(miss) => Err(TweakRefusal::Target(miss_of(text, &miss))),
    }
}

/// The merge a row carries, settled against the index.
fn join_of(
    request: &Request<'_>,
    row: &Row,
    level: Option<&Level>,
    map: &HashMap<Blake3Hash, Blake3Hash>,
) -> Result<Option<Blake3Hash>, TweakRefusal> {
    let Some(text) = row.join.as_deref() else {
        return Ok(None);
    };
    let text = text.trim_start_matches('+');

    let held = level
        .and_then(|level| level.variant.join().copied())
        .filter(|held| names(text, held));
    let join = if let Some(held) = held {
        held
    } else {
        let Some(index) = request.index else {
            return Err(WriteError::NoIndex.into());
        };
        let runtime = runtime()?;
        let objects = runtime
            .block_on(index.read_objects())
            .map_err(index_failed)?;
        let variants: Vec<Key> = objects
            .into_iter()
            .filter_map(|(hash, object)| object.is_variant().then_some(hash))
            .collect();
        match resolve_head(text, &variants) {
            Ok(key) => *key.digest(),
            Err(miss) => return Err(TweakRefusal::Target(miss_of(text, &miss))),
        }
    };

    Ok(Some(map.get(&join).copied().unwrap_or(join)))
}

/// The variants based on a version of the range that the plan does not stand for.
///
/// A variant based on a version whose content moves is based on a version that no longer exists
/// in the new chain, so it is replayed with the rest: a merge that names it names something that
/// is still there, and a candidate nobody has joined yet stays a candidate.
fn based_on(
    request: &Request<'_>,
    chain: &Chain,
    anchor: usize,
) -> Result<HashMap<usize, Vec<Variant>>, TweakRefusal> {
    let Some(index) = request.index else {
        return Err(WriteError::NoIndex.into());
    };
    let runtime = runtime()?;
    let objects = runtime
        .block_on(index.read_objects())
        .map_err(index_failed)?;

    let numbers: HashMap<Blake3Hash, usize> = chain
        .levels
        .iter()
        .enumerate()
        .map(|(number, level)| (*level.version.hash().digest(), number))
        .collect();
    let spine: Vec<Blake3Hash> = chain
        .levels
        .iter()
        .map(|level| *level.variant.hash().digest())
        .collect();

    let mut based: HashMap<usize, Vec<Variant>> = HashMap::new();
    for (_, object) in objects {
        let VCSIndexObject::Variant(variant) = object else {
            continue;
        };
        let Some(number) = numbers.get(variant.base_version()).copied() else {
            continue;
        };
        if number < anchor {
            continue;
        }
        // The variant that produced a version of the chain is one the sheet already stands for,
        // and replaying it here would leave two objects where the chain has room for one.
        if spine.contains(variant.hash().digest()) {
            continue;
        }
        based.entry(number).or_default().push(variant);
    }

    Ok(based)
}

/// Writes the replayed form of the variants based on one version, all of which share `base`.
///
/// A variant that merges with another of these is written after it, since the merge names the
/// replayed variant and the new hash is not known until it is worked out. Two variants that merge
/// with each other have no hash at all, and are refused rather than written in some order.
fn replay(
    rows: &[Variant],
    base: Blake3Hash,
    map: &mut HashMap<Blake3Hash, Blake3Hash>,
    written: &mut Vec<Variant>,
) -> Result<(), TweakRefusal> {
    let mut waiting: Vec<Variant> = rows.to_vec();

    while !waiting.is_empty() {
        let before = waiting.len();
        let mut at = 0;

        while at < waiting.len() {
            let variant = &waiting[at];
            let join = match variant.join() {
                Some(join) if map.contains_key(join) => Some(map[join]),
                Some(join) if waiting.iter().any(|other| other.hash().digest() == join) => {
                    // The variant it merges with is one of these, and is not worked out yet.
                    at += 1;
                    continue;
                }
                Some(join) => Some(*join),
                None => None,
            };

            let replayed = Variant::new_bare_variant(
                *variant.storage_hash(),
                base,
                join,
                *variant.creator(),
                *variant.message(),
                variant.base_version_num(),
            );
            map.insert(*variant.hash().digest(), *replayed.hash().digest());
            written.push(replayed);
            waiting.swap_remove(at);
        }

        if waiting.len() == before {
            return Err(TweakRefusal::Failed(WriteError::Internal {
                reason: "two variants merge with each other".to_owned(),
            }));
        }
    }

    Ok(())
}

/// Writes the version the entry now records over the file, when the file stood as recorded.
///
/// A file holding changes is left alone: the pointer moves and the work on disk stays what its
/// owner made it. A file whose new content is not here and cannot be brought is left alone too,
/// which leaves it standing as a change to make rather than a change lost.
fn bring_up_to_date(
    request: &Request<'_>,
    path: &LayoutPath,
    chain: &Chain,
    to: Blake3Hash,
    progress: &Progress,
) -> Result<Vec<TweakWarning>, TweakRefusal> {
    let Some(store) = request.store else {
        return Ok(Vec::new());
    };
    let Some(level) = chain.levels.last() else {
        return Ok(Vec::new());
    };

    let was = *level.variant.storage_hash();
    let now = stored_content(request, to)?;
    if was == now {
        return Ok(Vec::new());
    }

    let root = request.workspace.get_root();
    let disk = root.join(path.to_path_buf());
    if !key_of_file(&disk).is_ok_and(|key| key.digest() == &was) {
        return Ok(Vec::new());
    }

    let runtime = runtime()?;
    let held = runtime
        .block_on(store.contains_keys(&[Key::new(now)]))
        .is_ok_and(|presence| presence.held(0));
    if !held {
        let Some(account) = request.account else {
            return Ok(vec![TweakWarning::ContentUnavailable]);
        };
        let sources = Sources::new(
            request.workspace,
            account,
            request.candidates.to_vec(),
            request.offline,
        )
        .map_err(|reason| WriteError::Internal { reason })?
        .reporting(progress.clone())
        .with_concurrency(request.concurrency);
        if !sources.bring(store, &[Key::new(now)]).is_empty() {
            return Ok(vec![TweakWarning::ContentUnavailable]);
        }
    }

    if let Err(error) = runtime.block_on(store.extract_file(&Key::new(now), &disk)) {
        return Err(WriteError::Storage {
            reason: error.to_string(),
        }
        .into());
    }

    remember(request.layout, root, path.as_str())?;

    Ok(Vec::new())
}

/// The content the version `version` is of.
fn stored_content(request: &Request<'_>, version: Blake3Hash) -> Result<Blake3Hash, TweakRefusal> {
    let Some(index) = request.index else {
        return Err(WriteError::NoIndex.into());
    };
    let version = index
        .read_version(&Key::new(version).to_string())
        .map_err(index_failed)?;
    let variant = index
        .read_variant(&Key::new(*version.variant()).to_string())
        .map_err(index_failed)?;

    Ok(*variant.storage_hash())
}

/// The runtime the asynchronous reads and the content transfer are waited on.
fn runtime() -> Result<tokio::runtime::Runtime, TweakRefusal> {
    tokio::runtime::Runtime::new().map_err(|error| {
        TweakRefusal::Failed(WriteError::Internal {
            reason: error.to_string(),
        })
    })
}
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rorolala_layout::LayoutPath;
    use rorolala_storage::Key;
    use rorolala_utils_location::Locate as _;
    use rorolala_utils_progress::Progress;
    use rorolala_vcs::{VCSIndex, VCSWrite as _};
    use rorolala_workspace::Workspace;

    use super::{
        Adjustment, Chain, Change, Mark, Request, Sheet, TweakOutcome, adjust, apply, cancel,
        operations,
    };
    use crate::write::Cancellation;
    use crate::write::track::{TrackOptions, track_file};

    /// A Workspace holding `a.txt`, recorded once per pair of content and message.
    fn recorded(name: &str, versions: &[(&str, &str)]) -> (PathBuf, Workspace) {
        let dir =
            std::env::temp_dir().join(format!("rola-write-tweak-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        for (content, message) in versions {
            fs::write(dir.join("a.txt"), content).unwrap();
            track_file(
                &workspace,
                &dir,
                "a.txt",
                "alice",
                message,
                TrackOptions::default(),
            )
            .unwrap();
        }

        (dir, workspace)
    }

    /// The chain `a.txt` records.
    fn chain_of(workspace: &Workspace) -> Chain {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let id = layout.id_of(&path).unwrap();
        let data = layout.entry(id).unwrap();
        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let version = index
            .read_version(&Key::new(data.version()).to_string())
            .unwrap();

        Chain::read(&index, &version).unwrap()
    }

    /// The version `a.txt` records.
    fn recorded_version(workspace: &Workspace) -> [u8; 32] {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let id = layout.id_of(&path).unwrap();

        layout.entry(id).unwrap().version()
    }

    /// Applies `sheet` to `a.txt`.
    fn applied(workspace: &Workspace, sheet: &Sheet) -> TweakOutcome {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let id = layout.id_of(&path).unwrap();
        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let store = workspace.storage_set().unwrap();
        let candidates: Vec<(String, rorolala_protocol::VaultAddress)> = Vec::new();
        let request = Request {
            workspace,
            layout: &layout,
            index: Some(&index),
            store: Some(&store),
            account: None,
            candidates: &candidates,
            offline: false,
            concurrency: 1,
        };

        apply(
            &request,
            id,
            &path,
            sheet,
            &Progress::silent(),
            &Cancellation::new(),
        )
        .unwrap()
    }

    #[test]
    fn a_sheet_reads_the_rows_and_both_ends_it_names() {
        let text = "\
# a comment
>>> LOCAL
  alice  first   0123456789ab
  \"li wei\"  \"two words\"  ba9876543210  +ffee00001122
>>> 0123456789ab
";
        let sheet = Sheet::parse(text).unwrap();

        assert_eq!(sheet.anchor(), Some("0123456789ab"));
        assert_eq!(sheet.rows().len(), 2);
        assert_eq!(sheet.rows()[0].creator, "alice");
        assert_eq!(sheet.rows()[0].join, None);
        assert_eq!(sheet.rows()[1].creator, "li wei");
        assert_eq!(sheet.rows()[1].message, "two words");
        assert_eq!(sheet.rows()[1].join.as_deref(), Some("+ffee00001122"));
    }

    #[test]
    fn the_two_markers_say_what_a_row_does() {
        let text = "\
>>> LOCAL
  alice  first  0123456789ab
--alice  gone  0123456789ab
++alice  added  ba9876543210
>>> 0123456789ab
";
        let sheet = Sheet::parse(text).unwrap();

        assert_eq!(sheet.rows()[0].mark, Mark::Kept);
        assert_eq!(sheet.rows()[1].mark, Mark::Remove);
        assert_eq!(sheet.rows()[2].mark, Mark::Insert);
    }

    #[test]
    fn a_sheet_that_fits_its_chain_is_drawn_and_read_back() {
        let (dir, workspace) = recorded("round", &[("one", "first"), ("two", "second")]);
        let chain = chain_of(&workspace);
        let sheet = Sheet::plain(&chain, 0).unwrap();
        let text = sheet.render(&chain, &["a comment"]).unwrap();
        let read = Sheet::parse(&text).unwrap();

        assert_eq!(read.render(&chain, &["a comment"]).unwrap(), text);
        assert_eq!(read.rows().len(), chain.len());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_value_written_twice_keeps_only_the_later_one() {
        let (dir, workspace) = recorded("twice", &[("one", "first"), ("two", "second")]);
        let chain = chain_of(&workspace);
        let sheet = Sheet::plain(&chain, 0).unwrap();

        let bob = Change::Fields(Adjustment {
            creator: Some("bob".to_owned()),
            ..Adjustment::default()
        });
        let alice = Change::Fields(Adjustment {
            creator: Some("alice".to_owned()),
            ..Adjustment::default()
        });

        let after_bob = adjust(&chain, Some(&sheet), 0, &bob).unwrap();
        assert_eq!(operations(&chain, &after_bob).unwrap().len(), 1);

        // The second adjustment names what the version already is, so nothing is left to do.
        let after_alice = adjust(&chain, Some(&after_bob), 0, &alice).unwrap();
        assert!(after_alice.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cancelling_the_only_adjustment_leaves_the_sheet_empty() {
        let (dir, workspace) = recorded("cancel", &[("one", "first")]);
        let chain = chain_of(&workspace);
        let sheet = Sheet::plain(&chain, 0).unwrap();
        let changed = adjust(
            &chain,
            Some(&sheet),
            0,
            &Change::Fields(Adjustment {
                message: Some("edited".to_owned()),
                ..Adjustment::default()
            }),
        )
        .unwrap();

        let (back, operation) = cancel(&chain, &changed, 1).unwrap();
        assert!(back.is_empty());
        assert!(matches!(operation, super::Operation::Message { .. }));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn adjusting_an_old_version_rebuilds_every_version_above_it() {
        let (dir, workspace) = recorded(
            "rebuild",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        let chain = chain_of(&workspace);
        assert_eq!(chain.len(), 3);

        let sheet = Sheet::plain(&chain, 0).unwrap();
        let changed = adjust(
            &chain,
            Some(&sheet),
            0,
            &Change::Fields(Adjustment {
                message: Some("edited".to_owned()),
                ..Adjustment::default()
            }),
        )
        .unwrap();
        let outcome = applied(&workspace, &changed);

        assert_eq!(outcome.rebuilt, 3);

        let after = chain_of(&workspace);
        assert_eq!(after.len(), 3);
        assert_eq!(after.level(0).unwrap().message, "edited");
        assert_eq!(after.level(1).unwrap().message, "second");
        assert_eq!(after.level(2).unwrap().message, "third");
        assert_eq!(
            *after.level(2).unwrap().variant.storage_hash(),
            *chain.level(2).unwrap().variant.storage_hash()
        );
        assert_eq!(
            recorded_version(&workspace),
            *after.level(2).unwrap().version.hash().digest()
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn inserting_a_version_and_removing_it_again_lands_on_the_same_tail() {
        let (dir, workspace) = recorded("undone", &[("one", "first"), ("two", "second")]);
        let chain = chain_of(&workspace);
        let before = recorded_version(&workspace);

        let inserted = adjust(
            &chain,
            Some(&Sheet::plain(&chain, 0).unwrap()),
            0,
            &Change::Insert,
        )
        .unwrap();
        let outcome = applied(&workspace, &inserted);
        assert_eq!(outcome.rebuilt, 3);

        let after = chain_of(&workspace);
        assert_eq!(after.len(), 3);

        // The insert added the second version, and dropping it rebuilds what follows into exactly
        // the chain it was before the insert.
        let dropped = adjust(
            &after,
            Some(&Sheet::plain(&after, 0).unwrap()),
            1,
            &Change::Drop,
        )
        .unwrap();
        let outcome = applied(&workspace, &dropped);
        assert_eq!(outcome.rebuilt, 1);
        assert_eq!(recorded_version(&workspace), before);

        let _ = fs::remove_dir_all(dir);
    }
}

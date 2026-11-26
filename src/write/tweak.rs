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
use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::{EntryKind, Layout, LayoutPath};
use rorolala_protocol::VaultAddress;
use rorolala_status::Relation;
use rorolala_storage::{Blake3Hash, Key, StorageSet, key_of_file};
use rorolala_utils_constants::{WORKSPACE_IGNORE_RULES_DIR, WORKSPACE_TWEAKING_DIR};
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rorolala_vcs::{Creator, Message, VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version};
use rorolala_version_search as version_search;
use rorolala_version_search::{
    HeadMiss, miss_of, named, names, number_of as chain_number_of, resolve_head, short,
};
pub use rorolala_version_search::{Level, TargetMiss};
use rorolala_workspace::{Merging, Workspace};
use uuid::Uuid;

use crate::fetch::Sources;
use crate::tree::{Tree, TreeStructure, compare, conflict_copy, read_structure};
use crate::write::checkout::remember;
use crate::write::journal::{Effect, EntryState, Journal};
use crate::write::layout::set_version;
use crate::write::tree::put_back;
use crate::write::{Cancellation, WriteError};

/// The file a plan is written as, inside its directory.
const PLAN_FILE: &str = "plan.md";

/// How far the grid is set in from the left edge, which is also the width of `# `.
const INDENT: usize = 2;

/// How much room a column leaves before the one after it.
const GAP: usize = 4;

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
    #[must_use]
    pub fn kept(level: &Level) -> Self {
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

    /// A sheet made of `rows`, standing from `anchor` to the version the entry records.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Nothing`] when the chain is shorter than `anchor`.
    pub fn from_rows(chain: &Chain, anchor: usize, rows: Vec<Row>) -> Result<Self, TweakRefusal> {
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

    /// The sheet as the grid an editor and a plan file share.
    ///
    /// `header` is written above the grid as `#` lines, and an empty one as a blank line. `columns`
    /// names the four columns in the run's language. A column is as wide as its widest cell, its
    /// own name included, and each starts four spaces after the one before it, so the table reads
    /// the same however long a creator or a message is.
    ///
    /// # Errors
    ///
    /// Returns [`TweakRefusal::Sheet`] when the sheet does not fit the chain it names.
    pub fn render(
        &self,
        chain: &Chain,
        header: &[&str],
        columns: [&str; 4],
    ) -> Result<String, TweakRefusal> {
        let bound = bind(self, chain)?;
        let mut cells: Vec<[String; 4]> = Vec::with_capacity(bound.entries.len());

        for entry in &bound.entries {
            cells.push(cell_of(entry, chain)?);
        }

        let starts = offsets(&cells, columns);

        let mut text = String::new();
        for line in header {
            if !line.is_empty() {
                text.push_str("# ");
                text.push_str(line);
            }
            text.push('\n');
        }
        text.push_str(&laid(
            &[
                format!("# {}", columns[0]),
                columns[1].to_owned(),
                columns[2].to_owned(),
                columns[3].to_owned(),
            ],
            starts,
        ));
        text.push('\n');

        text.push_str(">>> LOCAL\n");
        for cell in &cells {
            let [creator, message, content, join] = cell;
            text.push_str(&laid(
                &[
                    format!("  {creator}"),
                    message.clone(),
                    content.clone(),
                    join.clone(),
                ],
                starts,
            ));
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

/// How many columns `text` takes on a terminal, which is what a column is padded to.
///
/// A CJK word is two columns a character, so counting characters would line a name of one script
/// up under a name of the other only by accident.
fn width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

/// Where each of the four columns starts, its widest cell and its own name deciding.
fn offsets(cells: &[[String; 4]], columns: [&str; 4]) -> [usize; 4] {
    let mut widths = [0_usize; 4];
    for (at, column) in columns.iter().enumerate() {
        widths[at] = width(column);
    }
    for cell in cells {
        for (at, part) in cell.iter().enumerate() {
            widths[at] = widths[at].max(width(part));
        }
    }

    let mut starts = [0_usize; 4];
    for at in 1..4 {
        starts[at] = if at == 1 { INDENT } else { starts[at - 1] } + widths[at - 1] + GAP;
    }

    starts
}

/// One line of the grid, its cells set at the column starts.
fn laid(cells: &[String; 4], starts: [usize; 4]) -> String {
    let mut line = cells[0].clone();
    for (at, cell) in cells.iter().enumerate().skip(1) {
        line.extend(std::iter::repeat_n(
            ' ',
            starts[at].saturating_sub(width(&line)),
        ));
        line.push_str(cell);
    }

    line.trim_end().to_owned()
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

    /// The number of the version `text` names among this chain's own.
    fn number_of(&self, text: &str) -> Result<usize, HeadMiss> {
        chain_number_of(&self.levels, text)
    }
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

// ---------------------------------------------------------------------------------------------
// Naming the version to adjust
// ---------------------------------------------------------------------------------------------

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

    version_search::resolve(&chain.levels, top, target)
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
    header: &[&str],
    columns: [&str; 4],
) -> Result<(), TweakRefusal> {
    let directory = plan_dir(root, id, local);
    if let Err(error) = fs::create_dir_all(&directory) {
        return Err(TweakRefusal::Failed(WriteError::Io {
            path: directory.display().to_string(),
            reason: error.to_string(),
        }));
    }

    let path = directory.join(PLAN_FILE);
    if let Err(error) = fs::write(&path, sheet.render(chain, header, columns)?) {
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
    /// The content the entry now records is not here, and could not be brought, so the work still
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

impl From<crate::tree::TreeError> for TweakRefusal {
    fn from(error: crate::tree::TreeError) -> Self {
        Self::Failed(WriteError::Tree {
            reason: error.to_string(),
        })
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

/// The number of the version `upstream` names on `chain`.
#[must_use]
pub fn origin_number(chain: &Chain, upstream: &[u8; 32]) -> Option<usize> {
    chain
        .levels
        .iter()
        .position(|level| level.version.hash().digest() == upstream)
}

/// The level a chain starts at when a run adjusts from the origin rather than from the first version.
///
/// The origin names a level only while this chain holds it, and only while the work stands ahead of
/// it: a chain that is apart from the Vault, or behind it, has no shared point to start from, so the
/// run falls back to the first version.
#[must_use]
pub fn anchor_at_origin(
    index: &VCSIndex,
    chain: &Chain,
    local: [u8; 32],
    upstream: [u8; 32],
) -> Option<usize> {
    let runtime = runtime().ok()?;
    let base = rorolala_status::compare(index, &runtime, &local, &upstream);
    if matches!(
        base.relation,
        Relation::Apart | Relation::Behind | Relation::Unknown
    ) {
        return None;
    }

    origin_number(chain, &upstream)
}

/// Reads the chain a path's entry records.
///
/// A tree records a version the same way a file does, and the chain above it is read here without
/// asking what the entry holds: a tree's history is one chain like any other.
///
/// # Errors
///
/// Returns [`TweakRefusal::NotRecorded`] when the path names no entry, [`TweakRefusal::Nothing`]
/// when it records no version, and [`TweakRefusal::Failed`] when the index cannot be read.
pub fn chain_of(
    index: &VCSIndex,
    layout: &Layout,
    path: &LayoutPath,
) -> Result<Chain, TweakRefusal> {
    let id = layout.id_of(path).ok_or(TweakRefusal::NotRecorded)?;
    let data = layout.entry(id).ok_or(TweakRefusal::NotRecorded)?;
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

/// How an application chooses its base and what it replays.
#[derive(Debug, Clone)]
pub struct ApplyOptions {
    /// The version the first row is based on, when a run names one.
    pub base: Option<Blake3Hash>,
    /// Whether the variants based on a version of the range are replayed beside it.
    pub replay_based: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            base: None,
            replay_based: true,
        }
    }
}

/// Applies `sheet` to the chain the path's entry records.
///
/// The versions above the sheet's oldest one are written again, the merge of each carried over and
/// pointed at the replayed variant when that variant was replayed too, and the variants based on a
/// version in the range are replayed beside them so a merge names something that is still there.
/// The Layout is then made to record the replayed tail, and the work is brought up to it when it
/// stood as recorded: work holding changes is left alone, and work whose content is not here is
/// left alone with a [`TweakWarning`].
///
/// A tree is brought up the way a file is, except that what stands recorded is a structure and the
/// work it names is many files: a tree holding no change has its work rewritten, and one holding a
/// change has only its baseline moved.
///
/// # Errors
///
/// Returns [`TweakRefusal`] when the run may not adjust the entry, and [`WriteError::Cancelled`]
/// inside [`TweakRefusal::Failed`] when `cancellation` was set.
pub fn apply(
    request: &Request<'_>,
    id: Uuid,
    path: &LayoutPath,
    sheet: &Sheet,
    progress: &Progress,
    cancellation: &Cancellation,
) -> Result<TweakOutcome, TweakRefusal> {
    apply_with(
        request,
        id,
        path,
        sheet,
        &ApplyOptions {
            base: None,
            replay_based: true,
        },
        progress,
        cancellation,
        None,
    )
}

/// Applies `sheet` with the base and the replay `options` give.
///
/// # Errors
///
/// As [`apply`].
#[allow(
    clippy::too_many_arguments,
    reason = "an apply reads the request, the entry, the sheet, the options, the watching and the journal it records into, and gathering them would only move the count"
)]
pub fn apply_with(
    request: &Request<'_>,
    id: Uuid,
    path: &LayoutPath,
    sheet: &Sheet,
    options: &ApplyOptions,
    progress: &Progress,
    cancellation: &Cancellation,
    mut journal: Option<&mut Journal>,
) -> Result<TweakOutcome, TweakRefusal> {
    let Some(index) = request.index else {
        return Err(WriteError::NoIndex.into());
    };
    let Some(data) = request.layout.entry(id) else {
        return Err(TweakRefusal::NotRecorded);
    };
    if data.version() == [0; 32] {
        return Err(TweakRefusal::Nothing);
    }

    let root = request.workspace.get_root();
    if let Some(pending) = Merging::read(root).at(id) {
        return Err(TweakRefusal::Merging {
            variant: Key::new(*pending.variant()).hex(),
        });
    }

    // A tree's work is read and written through its directory, so a tree that is not here stops the
    // run before the chain has moved rather than after.
    if data.kind() == EntryKind::Tree {
        Tree::open(root, &root.join(path.to_path_buf()))?;
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

    // What the new chain stands at before its first row, which is the version the range sits on
    // unless a run named a base of its own.
    let mut base = match options.base {
        Some(base) => base,
        None if bound.anchor == 0 => *Version::root().hash().digest(),
        None => *chain.level(bound.anchor - 1)?.version.hash().digest(),
    };

    let based = if options.replay_based {
        based_on(request, &chain, bound.anchor)?
    } else {
        HashMap::new()
    };

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

    if let Some(journal) = journal.as_deref_mut() {
        let owner = data.owner().map(str::to_owned);
        journal.record(Effect::Entry {
            id,
            path: Some(path.as_str().to_owned()),
            before: Some(EntryState::of(&data)),
            after: Some(EntryState::of(&data.renewed(owner, base))),
        });
    }

    // A tree is one entry whose work is many files, so what a file's bring-up reads in one content
    // key is a structure and the files it names. It is brought up before the Layout names the new
    // version, since a tree whose content does not decode as a structure must not be recorded.
    let warnings = if data.kind() == EntryKind::Tree {
        let warnings = bring_tree_up_to_date(
            request,
            path,
            id,
            data.version(),
            base,
            progress,
            cancellation,
        )?;
        set_version(request.layout, id, base)?;

        warnings
    } else {
        set_version(request.layout, id, base)?;

        bring_up_to_date(request, path, &chain, base, progress, journal)?
    };

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
    journal: Option<&mut Journal>,
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

    if let Some(journal) = journal {
        journal.record(Effect::File {
            path: path.as_str().to_owned(),
            before: Some(Key::new(was).hex()),
            after: Some(Key::new(now).hex()),
        });
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

/// Writes the version a tree now records over its work when the work stood as recorded, and moves
/// the baseline alone when it did not.
///
/// A tree is one entry whose work is many files, so what a file's bring-up reads in one content key
/// is a structure and the files it names. A tree holding a change is the owner's: only the copy of
/// what is recorded moves, and the work on disk is left exactly as it stands.
fn bring_tree_up_to_date(
    request: &Request<'_>,
    path: &LayoutPath,
    id: Uuid,
    before: Blake3Hash,
    to: Blake3Hash,
    progress: &Progress,
    cancellation: &Cancellation,
) -> Result<Vec<TweakWarning>, TweakRefusal> {
    let Some(index) = request.index else {
        return Err(WriteError::NoIndex.into());
    };
    let root = request.workspace.get_root();
    let tree = Tree::open(root, &root.join(path.to_path_buf()))?;
    let runtime = runtime()?;

    let old = recorded_structure(request, &runtime, index, &tree, before)?;

    // A version whose content is the one recorded before is the same structure, so it is answered
    // from the top of the old chain rather than read again. A metadata-only adjustment therefore
    // needs nothing new from the store and nothing from a Vault.
    let was = stored_content(request, before)?;
    let now = stored_content(request, to)?;
    let new = if now == was {
        old.clone()
    } else {
        let Some(store) = request.store else {
            return Ok(vec![TweakWarning::ContentUnavailable]);
        };
        if !bring_keys(request, &runtime, store, &[Key::new(now)], progress)? {
            return Ok(vec![TweakWarning::ContentUnavailable]);
        }
        runtime
            .block_on(read_structure(index, store, &to))
            .map_err(TweakRefusal::from)?
    };

    let staging = tree.staging();
    let staged = staging.structure()?.unwrap_or_else(|| old.clone());
    let conflicts = staging.conflicts()?;
    let rules = tree.rules(workspace_rules(request.workspace))?;
    let scanned = tree.scan(&rules, false)?;
    let status = compare(&old, &staged, &scanned.structure, &conflicts);
    let clean =
        status.staged.is_empty() && status.unstaged.is_empty() && status.conflicts.is_empty();

    let baseline = tree.baseline();
    if !clean {
        baseline.write(&new, &to)?;

        return Ok(Vec::new());
    }

    if old != new {
        let Some(store) = request.store else {
            return Ok(vec![TweakWarning::ContentUnavailable]);
        };
        let keys: Vec<Key> = new
            .entries()
            .iter()
            .filter(|entry| old.entry(entry.path()) != Some(entry))
            .filter_map(|entry| entry.storage().copied())
            .collect();
        if !bring_keys(request, &runtime, store, &keys, progress)? {
            return Ok(vec![TweakWarning::ContentUnavailable]);
        }
        put_back(&runtime, store, tree.root(), &new, &old, true, cancellation)
            .map_err(TweakRefusal::Failed)?;
    }

    for conflict in &conflicts {
        let _ = fs::remove_file(tree.root().join(conflict_copy(conflict)));
    }
    staging.clear()?;

    // What was just written is read again, so the cache names the files now on disk rather than the
    // ones the reading started from.
    let after = tree.scan(&rules, false)?;
    baseline.write(&new, &to)?;
    baseline.write_entries(&after.cache)?;
    tree.write_local_id(id)?;

    Ok(Vec::new())
}

/// The structure the tree recorded at `version`, read from the baseline when it is the one held
/// there and from the store otherwise.
fn recorded_structure(
    request: &Request<'_>,
    runtime: &tokio::runtime::Runtime,
    index: &VCSIndex,
    tree: &Tree,
    version: Blake3Hash,
) -> Result<TreeStructure, TweakRefusal> {
    let baseline = tree.baseline();
    if baseline.fresh_for(&version)
        && let Some(structure) = baseline.tree()?
    {
        return Ok(structure);
    }

    let Some(store) = request.store else {
        return Err(WriteError::NoStore.into());
    };

    runtime
        .block_on(read_structure(index, store, &version))
        .map_err(TweakRefusal::from)
}

/// Brings `keys` here, answering whether every one of them is held afterwards.
fn bring_keys(
    request: &Request<'_>,
    runtime: &tokio::runtime::Runtime,
    store: &StorageSet,
    keys: &[Key],
    progress: &Progress,
) -> Result<bool, TweakRefusal> {
    if keys.is_empty() {
        return Ok(true);
    }

    let presence = runtime
        .block_on(store.contains_keys(keys))
        .map_err(|error| WriteError::Storage {
            reason: error.to_string(),
        })?;
    let missing: Vec<Key> = keys
        .iter()
        .enumerate()
        .filter(|(at, _)| !presence.held(*at))
        .map(|(_, key)| *key)
        .collect();
    if missing.is_empty() {
        return Ok(true);
    }

    let Some(account) = request.account else {
        return Ok(false);
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

    Ok(sources.bring(store, &missing).is_empty())
}

/// The rules a tree's work is read by: the Workspace's own, which is what a commit reads it with.
fn workspace_rules(workspace: &Workspace) -> IgnoreRuleSet {
    IgnoreRuleSet::load(
        &workspace.get_root().join(WORKSPACE_IGNORE_RULES_DIR),
        workspace.get_root(),
    )
    .unwrap_or_else(|_| IgnoreRuleSet::empty())
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
    use rorolala_vcs::{VCSIndex, VCSWrite as _, Version};
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{
        Adjustment, ApplyOptions, Chain, Change, Mark, Operation, Request, Sheet, SheetError,
        TargetMiss, TweakOutcome, TweakRefusal, adjust, anchor_at_origin, apply, apply_with,
        cancel, operations, plan_dir, read_plan, remove_plan, resolve, width, write_plan,
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

    #[test]
    fn a_chain_starts_at_the_origin_only_when_it_holds_it() {
        let (dir, workspace) = recorded(
            "origin-anchor",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        let chain = chain_of(&workspace);
        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let local = recorded_version(&workspace);
        let origin = *chain.level(1).unwrap().version.hash().digest();

        assert_eq!(anchor_at_origin(&index, &chain, local, origin), Some(1));
        assert_eq!(anchor_at_origin(&index, &chain, local, [9; 32]), None);

        let _ = fs::remove_dir_all(dir);
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

    /// Applies `sheet` to `a.txt` with the base and the replay `options` give.
    fn applied_with(workspace: &Workspace, sheet: &Sheet, options: &ApplyOptions) -> TweakOutcome {
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

        apply_with(
            &request,
            id,
            &path,
            sheet,
            options,
            &Progress::silent(),
            &Cancellation::new(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn a_base_of_its_own_rebases_the_range_on_the_root() {
        let (dir, workspace) = recorded(
            "base-root",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        let chain = chain_of(&workspace);
        let sheet = Sheet::plain(&chain, 1).unwrap();
        let root = *Version::root().hash().digest();
        let outcome = applied_with(
            &workspace,
            &sheet,
            &ApplyOptions {
                base: Some(root),
                replay_based: true,
            },
        );

        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let top = index.read_version(&outcome.to).unwrap();
        let top_variant = index
            .read_variant(&Key::new(*top.variant()).to_string())
            .unwrap();
        let rebased = index
            .read_version(&Key::new(*top_variant.base_version()).to_string())
            .unwrap();
        let rebased_variant = index
            .read_variant(&Key::new(*rebased.variant()).to_string())
            .unwrap();

        assert_eq!(*rebased_variant.base_version(), root);

        let _ = fs::remove_dir_all(dir);
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
    fn a_column_starts_after_the_widest_cell_it_holds() {
        let (dir, workspace) =
            recorded("grid", &[("one", "增加第一个版本"), ("two", "第二个版本")]);
        let chain = chain_of(&workspace);
        let sheet = Sheet::plain(&chain, 0).unwrap();
        let columns = ["名字", "描述", "内容", "合入"];
        let text = sheet.render(&chain, &["a note", ""], columns).unwrap();
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(lines[0], "# a note");
        assert_eq!(lines[1], "");
        assert!(lines[2].starts_with("# 名字"));
        assert!(lines[4].starts_with("  alice"));

        // The widest creator is `alice`, five columns, and `名字` is four: the message column
        // starts two spaces in, five columns later, and four spaces after that.
        let message = 2 + 5 + 4;
        let content = message + width("增加第一个版本") + 4;
        let join = content + 12 + 4;
        assert_eq!(join, content + 16);

        for line in [lines[2], lines[4]] {
            let at = line
                .find("描述")
                .or_else(|| line.find("第二个版本"))
                .unwrap();
            assert_eq!(width(&line[..at]), message, "{line}");
        }
        let content_at = lines[4]
            .find(&*super::short(
                chain.level(1).unwrap().variant.storage_hash(),
            ))
            .unwrap();
        assert_eq!(width(&lines[4][..content_at]), content, "{}", lines[4]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_sheet_that_fits_its_chain_is_drawn_and_read_back() {
        let (dir, workspace) = recorded("round", &[("one", "first"), ("two", "second")]);
        let chain = chain_of(&workspace);
        let sheet = Sheet::plain(&chain, 0).unwrap();
        let columns = ["creator", "message", "content", "join"];
        let text = sheet.render(&chain, &["a comment", ""], columns).unwrap();
        let read = Sheet::parse(&text).unwrap();

        assert_eq!(
            read.render(&chain, &["a comment", ""], columns).unwrap(),
            text
        );
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
    fn every_fault_is_written_as_a_sentence() {
        assert_eq!(SheetError::new(2, "bad").to_string(), "line 2: bad");
        assert_eq!(
            TargetMiss::NoMatch {
                target: "x".to_owned()
            }
            .to_string(),
            "nothing matches x"
        );
        assert_eq!(
            TargetMiss::Ambiguous {
                target: "x".to_owned(),
                candidates: vec![1]
            }
            .to_string(),
            "x matches more than one"
        );
        assert_eq!(
            TargetMiss::Elsewhere {
                target: "x".to_owned()
            }
            .to_string(),
            "x belongs to another chain"
        );
        assert_eq!(
            TargetMiss::OutOfChain {
                target: "x".to_owned()
            }
            .to_string(),
            "x reaches past the first version"
        );
        assert_eq!(
            TargetMiss::Removed {
                version: "V1".to_owned()
            }
            .to_string(),
            "V1 is being removed"
        );

        assert_eq!(
            TweakRefusal::NotRecorded.to_string(),
            "the Layout records no entry at the path"
        );
        assert_eq!(
            TweakRefusal::Nothing.to_string(),
            "the entry records no version"
        );
        assert_eq!(
            TweakRefusal::NotAChain.to_string(),
            "the version is not the top of a chain"
        );
        assert_eq!(
            TweakRefusal::Merging {
                variant: "ab".to_owned()
            }
            .to_string(),
            "the variant ab is waiting to be joined"
        );
        assert_eq!(
            TweakRefusal::Sheet {
                reason: "bad".to_owned()
            }
            .to_string(),
            "bad"
        );
        assert_eq!(
            TweakRefusal::Target(TargetMiss::NoMatch {
                target: "x".to_owned()
            })
            .to_string(),
            "nothing matches x"
        );
        let from_sheet: TweakRefusal = SheetError::new(1, "bad").into();
        assert!(matches!(from_sheet, TweakRefusal::Sheet { .. }));
        let from_write: TweakRefusal = crate::write::WriteError::NoIndex.into();
        assert!(matches!(from_write, TweakRefusal::Failed(_)));

        for operation in [
            Operation::Creator {
                version: "V0".to_owned(),
                text: "a".to_owned(),
            },
            Operation::Message {
                version: "V0".to_owned(),
                text: "a".to_owned(),
            },
            Operation::Content {
                version: "V0".to_owned(),
                hash: "a".to_owned(),
            },
            Operation::Join {
                version: "V0".to_owned(),
                variant: "a".to_owned(),
            },
            Operation::Unjoin {
                version: "V0".to_owned(),
                variant: "a".to_owned(),
            },
            Operation::Insert {
                below: "V0".to_owned(),
                above: "V1".to_owned(),
            },
            Operation::Remove {
                version: "V0".to_owned(),
            },
        ] {
            assert_ne!(operation.to_string(), "");
        }
    }

    #[test]
    fn a_sheet_that_does_not_read_is_refused() {
        // A marker with nothing after it.
        assert!(Sheet::parse(">>> \n").is_err());
        // No version the entry records.
        assert!(Sheet::parse("alice first abc\n>>> abcd\n").is_err());
        // No oldest version.
        assert!(Sheet::parse(">>> LOCAL\nalice first abc\n").is_err());
        // An unclosed quote.
        assert!(Sheet::parse(">>> LOCAL\n\"alice first abc\n>>> abcd\n").is_err());
        // Too few fields.
        assert!(Sheet::parse(">>> LOCAL\nalice first\n>>> abcd\n").is_err());
        // A merge without the marker a merge is written with.
        assert!(Sheet::parse(">>> LOCAL\nalice first abc dead\n>>> abcd\n").is_err());
    }

    #[test]
    fn a_chain_resolves_a_name_by_number_hash_and_words() {
        let (dir, workspace) = recorded("resolve", &[("one", "first"), ("two", "second")]);
        let chain = chain_of(&workspace);
        let top = chain.top().unwrap();

        assert_eq!(resolve(&chain, None).unwrap(), top);
        assert_eq!(resolve(&chain, Some("~0")).unwrap(), top);
        assert_eq!(resolve(&chain, Some("~1")).unwrap(), 0);
        assert!(matches!(
            resolve(&chain, Some("~9")),
            Err(TargetMiss::OutOfChain { .. })
        ));
        assert!(matches!(
            resolve(&chain, Some("~x")),
            Err(TargetMiss::NoMatch { .. })
        ));

        let head = chain.short(0).unwrap();
        assert_eq!(resolve(&chain, Some(&head)).unwrap(), 0);
        let variant = super::short(chain.level(1).unwrap().variant.hash().digest());
        assert_eq!(resolve(&chain, Some(&variant)).unwrap(), 1);

        assert_eq!(resolve(&chain, Some("second")).unwrap(), 1);
        assert!(matches!(
            resolve(&chain, Some("alice")),
            Err(TargetMiss::Ambiguous { .. })
        ));
        assert!(matches!(
            resolve(&chain, Some("nothing")),
            Err(TargetMiss::NoMatch { .. })
        ));
        assert!(matches!(
            resolve(&chain, Some("0123456789ab")),
            Err(TargetMiss::Elsewhere { .. })
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn adjusting_a_row_below_and_above_the_range_extends_and_renders_it() {
        let (dir, workspace) = recorded(
            "wide",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        let chain = chain_of(&workspace);
        let columns = ["creator", "message", "content", "join"];

        // A sheet standing from the second version is widened down to the first.
        let widened = adjust(
            &chain,
            Some(&Sheet::plain(&chain, 1).unwrap()),
            0,
            &Change::Fields(Adjustment {
                creator: Some("bob".to_owned()),
                message: Some("edited".to_owned()),
                content: Some("ba9876543210".to_owned()),
                join: Some(None),
            }),
        )
        .unwrap();
        assert!(
            widened
                .render(&chain, &["note"], columns)
                .unwrap()
                .contains("bob")
        );

        // An added row at the top stands below the version the entry records.
        let inserted = adjust(
            &chain,
            Some(&Sheet::plain(&chain, 0).unwrap()),
            chain.top().unwrap(),
            &Change::Insert,
        )
        .unwrap();
        assert!(
            inserted
                .render(&chain, &[], columns)
                .unwrap()
                .contains("++")
        );
        assert_eq!(operations(&chain, &inserted).unwrap().len(), 1);

        let dropped = adjust(
            &chain,
            Some(&Sheet::plain(&chain, 0).unwrap()),
            0,
            &Change::Drop,
        )
        .unwrap();
        assert!(dropped.render(&chain, &[], columns).unwrap().contains("--"));
        assert!(matches!(
            adjust(
                &chain,
                Some(&dropped),
                0,
                &Change::Fields(Adjustment::default())
            ),
            Err(TweakRefusal::Target(TargetMiss::Removed { .. }))
        ));

        let (back, _) = cancel(&chain, &dropped, 1).unwrap();
        assert!(back.is_empty());
        assert!(matches!(
            cancel(&chain, &dropped, 9),
            Err(TweakRefusal::Target(TargetMiss::NoMatch { .. }))
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_plan_is_written_read_back_and_removed() {
        let (dir, workspace) = recorded("plan", &[("one", "first")]);
        let chain = chain_of(&workspace);
        let sheet = Sheet::plain(&chain, 0).unwrap();
        let id = Uuid::from_u128(7);
        let local = [3_u8; 32];

        assert!(read_plan(&dir, id, &local).unwrap().is_none());
        write_plan(
            &dir,
            id,
            &local,
            &sheet,
            &chain,
            &["note"],
            ["creator", "message", "content", "join"],
        )
        .unwrap();
        let read = read_plan(&dir, id, &local).unwrap().unwrap();
        assert_eq!(read.rows().len(), sheet.rows().len());

        remove_plan(&dir, id, &local).unwrap();
        assert!(read_plan(&dir, id, &local).unwrap().is_none());
        // Taking away what is already gone is not a failure.
        remove_plan(&dir, id, &local).unwrap();
        assert_eq!(
            plan_dir(&dir, id, &local),
            dir.join(super::WORKSPACE_TWEAKING_DIR)
                .join(id.to_string())
                .join(Key::new(local).hex())
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

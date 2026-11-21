//! The version chain as the terminal draws it.
//!
//! One row per variant: its hash, who made it, what it says, and the lines that tie the rows
//! together. There is no row for a version, and no version number or version hash is drawn: a
//! version is only what its variants hang from, so the rows say what the chain is made of.
//!
//! Two kinds of line stand beside the rows. The bracket on the left groups the variants based on one
//! version, the merged ones above and the spine last. The lines on the right are the merges: from the
//! variant that joins, down to the variant it merges in, or out to the hash of one the chain does not
//! hold. Colours say which kind a row is, so the shape is read before the words are.
//!
//! Every column is a display column, measured with `unicode_width`: a creator, a message and the word
//! beside an origin may be Chinese, and a wide character takes two columns of the terminal.

use std::collections::{HashMap, HashSet};

use librorolala::vcs::{TopKind, VariantView, short_hash};
use rorolala_utils_cli_theme::{Colorize as _, ThemeChoice, theme_choice};
use rust_i18n::t;
use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};

use crate::vcs_index::cmd_vcs_index_lookback::{
    DEFAULT_MESSAGE_LENGTH, ResLookback, ResultVcsIndexLookback,
};

/// The column a variant's hash starts at, after the room the left lines stand in.
const LABEL: usize = 11;

/// How many columns a hash takes.
const HASH: usize = 7;

/// The column the creator's colon stands at, which every creator is right-aligned to.
const COLON: usize = 37;

/// The column a message starts at.
const MESSAGE: usize = 39;

/// Where the word of an origin's line ends, the column before its dash.
const ORIGIN_END: usize = 9;

/// How far right of a merge's arrowhead its line turns down.
const REACH: usize = 15;

/// How many dashes stand between an arrowhead and the hash of a variant the chain does not hold.
const HASH_LEAD: usize = 9;

/// The first column of the bracket beside a group.
const BRACKET: usize = 1;

/// What a drawing shows, as the run that asked for it.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Whether what each variant says is drawn.
    pub message: bool,
    /// Whether who made each variant is drawn.
    pub creator: bool,
    /// How much of a message is drawn before it is cut short.
    pub max_message_length: usize,
    /// Whether the rows are drawn one line each, without the lines between them.
    pub compact: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            message: true,
            creator: true,
            max_message_length: DEFAULT_MESSAGE_LENGTH,
            compact: false,
        }
    }
}

impl Options {
    /// What a lookback run asked for.
    pub(crate) fn of(lookback: &ResLookback) -> Self {
        Self {
            message: lookback.message(),
            creator: lookback.creator(),
            max_message_length: lookback.max_message_length(),
            compact: lookback.compact(),
        }
    }
}

/// Draws `result` as the lines a terminal shows.
pub fn draw(result: &ResultVcsIndexLookback, options: &Options) -> Vec<String> {
    draw_with(result, options, Glyphs::of(theme_choice()))
}

/// Draws `result` with the lines `glyphs` names, which a test states rather than leaving to the terminal.
fn draw_with(result: &ResultVcsIndexLookback, options: &Options, glyphs: Glyphs) -> Vec<String> {
    let rows = rows(result, options);

    if options.compact {
        return rows.iter().map(compact).collect();
    }

    let placed: Vec<Placed> = rows
        .iter()
        .map(|row| placed(row, Some(glyphs), row.hue))
        .collect();
    let joins = joins(&rows, &placed);
    let lines = assembled(&rows, &placed, &joins, glyphs);

    lines.iter().map(|line| render(line)).collect()
}

/// What a row of the drawing is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hue {
    /// A word that is the reader's own, drawn as it is.
    Plain,
    /// A variant the chain is made of.
    Chain,
    /// A variant merged into it, whose own base version the chain holds.
    Merged,
    /// A variant from outside: it is based on a version the chain does not hold.
    Outside,
    /// The unrecorded work, or the merge still to be made.
    Work,
    /// The line over the version the Vault records.
    Origin,
}

impl Hue {
    /// The colour this hue is drawn in.
    const fn ink(self) -> Ink {
        Ink {
            hue: self,
            bold: false,
            italic: false,
        }
    }

    /// The same colour, bold, for a hash.
    const fn hashed(self) -> Ink {
        Ink {
            hue: self,
            bold: true,
            italic: false,
        }
    }

    /// The same colour, italic, for what a variant says.
    const fn said(self) -> Ink {
        Ink {
            hue: self,
            bold: false,
            italic: true,
        }
    }
}

/// Where a row stands in the bracket beside its group.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bracket {
    /// It has no bracket of its own.
    None,
    /// The first row of a group.
    Open,
    /// A row of a group that is neither its first nor its last.
    Middle,
    /// The last row of a group, whose variant the chain continues down from.
    Close,
    /// The last row of a group that has no spine in it.
    ClosePlain,
}

/// What is not recorded: the work being edited, or a merge waiting to be made.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    /// Changes that were never recorded.
    Editing,
    /// A variant waiting to be joined into the file.
    Joining,
}

/// One row of the drawing.
struct Row {
    /// The hash the index names it by, or nothing for the unrecorded work.
    hash: String,
    /// The seven characters the row shows.
    label: String,
    /// The colour it is drawn in.
    hue: Hue,
    /// Whether the hash is the version the Vault records.
    origin: bool,
    /// Who made it, or nothing when the run leaves that out.
    creator: String,
    /// What it says, already cut to the length the run asked for.
    message: String,
    /// The variant it merges in, as a hash.
    join: Option<String>,
    /// Whether the variant it merges in is drawn as a row of its own.
    join_drawn: bool,
    /// What it is, when it is not a recorded variant.
    mood: Option<Mood>,
    /// Where it stands in its group's bracket.
    bracket: Bracket,
}

/// The rows a result draws, top to bottom.
fn rows(result: &ResultVcsIndexLookback, options: &Options) -> Vec<Row> {
    let mut rows = Vec::new();

    if let Some(mood) = mood(result) {
        rows.push(Row {
            hash: String::new(),
            label: "???????".to_owned(),
            hue: Hue::Work,
            origin: false,
            creator: if options.creator {
                "?????".to_owned()
            } else {
                String::new()
            },
            message: format!("[{}]", state_word(mood)),
            join: pending(result),
            join_drawn: false,
            mood: Some(mood),
            bracket: Bracket::None,
        });
    }

    // One group per version the chain or its merges name, the highest first.
    let mut numbers: Vec<u64> = result
        .levels
        .iter()
        .map(|level| level.number)
        .chain(result.unjoined.iter().map(|one| one.number))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    numbers.reverse();

    // Where the variant that produced each version is drawn, so the Vault's version can be marked.
    let mut produced: HashMap<u64, usize> = HashMap::new();

    // What a merge has already named from a row above it. A variant merged in from the same version
    // as the one that merges it has no row: it is named by that row's hash instead, since a row of
    // its own would stand over the one it belongs to and say the merge ran the wrong way.
    let mut named_above: HashSet<String> = rows
        .first()
        .and_then(|row| row.join.clone())
        .into_iter()
        .collect();

    for number in numbers {
        let level = result.levels.iter().find(|level| level.number == number);
        let mut group: Vec<Row> = Vec::new();

        if let Some(level) = level {
            for merged in &level.merged {
                if named_above.contains(&merged.hash) {
                    group.push(variant_row(merged, Hue::Merged, options));
                }
            }
        }
        for unjoined in result.unjoined.iter().filter(|one| one.number == number) {
            group.push(variant_row(&unjoined.variant, Hue::Outside, options));
        }
        if let Some(level) = level {
            produced.insert(number.saturating_add(1), rows.len() + group.len());
            group.push(variant_row(&level.variant, Hue::Chain, options));
        }

        for row in &group {
            if let Some(join) = &row.join {
                named_above.insert(join.clone());
            }
        }

        bracketed(&mut group);
        rows.append(&mut group);
    }

    if let Some(first) = &result.first {
        produced.insert(0, rows.len());
        rows.push(variant_row(first, Hue::Chain, options));
    }

    let drawn: HashSet<String> = rows
        .iter()
        .filter(|row| !row.hash.is_empty())
        .map(|row| row.hash.clone())
        .collect();
    for row in &mut rows {
        row.join_drawn = row.join.as_ref().is_some_and(|join| drawn.contains(join));
    }

    if let Some(origin) = &result.origin
        && let Some(at) = produced.get(&origin.number)
        && let Some(row) = rows.get_mut(*at)
    {
        row.origin = true;
    }

    rows
}

/// One row for a variant, as the run asked for it.
fn variant_row(variant: &VariantView, hue: Hue, options: &Options) -> Row {
    Row {
        hash: variant.hash.clone(),
        label: short_hash(&variant.hash).to_owned(),
        hue,
        origin: false,
        creator: if options.creator {
            variant.creator.clone()
        } else {
            String::new()
        },
        message: if options.message {
            truncate(&variant.message, options.max_message_length)
        } else {
            String::new()
        },
        join: variant.join.clone(),
        join_drawn: false,
        mood: None,
        bracket: Bracket::None,
    }
}

/// Puts a group's brackets on its rows: the first opens it, the last closes it, and a group of one
/// variant is not bracketed at all.
fn bracketed(group: &mut [Row]) {
    if group.len() < 2 {
        return;
    }

    let last = group.len() - 1;
    for (at, row) in group.iter_mut().enumerate() {
        row.bracket = match at {
            0 => Bracket::Open,
            _ if at == last => Bracket::ClosePlain,
            _ => Bracket::Middle,
        };
    }

    if let Some(spine) = group.iter_mut().find(|row| row.hue == Hue::Chain) {
        spine.bracket = Bracket::Close;
    }
}

/// What is not recorded in this result, when anything is.
fn mood(result: &ResultVcsIndexLookback) -> Option<Mood> {
    if result
        .standing
        .as_ref()
        .is_some_and(|standing| standing.merging.is_some())
    {
        return Some(Mood::Joining);
    }

    (result.top == TopKind::Editing).then_some(Mood::Editing)
}

/// The variant waiting to be joined, when one is.
fn pending(result: &ResultVcsIndexLookback) -> Option<String> {
    result
        .standing
        .as_ref()
        .and_then(|standing| standing.merging.as_ref())
        .map(|merging| merging.variant.clone())
}

/// What a place in the chain that is not recorded says.
fn state_word(mood: Mood) -> String {
    let word = match mood {
        Mood::Editing => t!("vcs_index_lookback.state_editing"),
        Mood::Joining => t!("vcs_index_lookback.state_joining"),
    };

    word.trim().to_owned()
}

/// The word the line over a Vault's version begins with.
fn origin_word() -> String {
    t!("vcs_index_lookback.origin").trim().to_owned()
}

/// `text` cut to `longest` characters, ending in an ellipsis when it was cut.
///
/// The ellipsis is part of what is shown, so a message of `longest` characters exactly is shown
/// whole and a longer one is cut to `longest - 3` and the three dots that say so. A `longest` too
/// small to hold the ellipsis cuts the text alone: what a run asked for is a length, and an
/// ellipsis that overran it would be saying something else.
fn truncate(text: &str, longest: usize) -> String {
    const ELLIPSIS: &str = "...";

    if text.chars().count() <= longest {
        return text.to_owned();
    }

    if longest <= ELLIPSIS.len() {
        return text.chars().take(longest).collect();
    }

    let kept: String = text.chars().take(longest - ELLIPSIS.len()).collect();

    format!("{kept}{ELLIPSIS}")
}

/// What a colour is, and whether it is bold or italic.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Ink {
    /// The colour.
    hue: Hue,
    /// Whether the text is bold.
    bold: bool,
    /// Whether the text is italic.
    italic: bool,
}

impl Ink {
    /// No colour at all, for the words that are the reader's own.
    const fn plain() -> Self {
        Self {
            hue: Hue::Plain,
            bold: false,
            italic: false,
        }
    }

    /// The line over a Vault's version.
    const fn origin() -> Self {
        Self {
            hue: Hue::Origin,
            bold: true,
            italic: false,
        }
    }
}

/// One column of a line.
#[derive(Clone, Copy)]
struct Cell {
    /// The character drawn here.
    ch: char,
    /// How it is drawn.
    ink: Ink,
    /// Whether the column is the second half of a wide character and is not drawn itself.
    covered: bool,
}

impl Cell {
    /// A column nothing is drawn in.
    const fn blank() -> Self {
        Self {
            ch: ' ',
            ink: Ink::plain(),
            covered: false,
        }
    }
}

/// What a row's own text is, and where a line that starts after it begins.
struct Placed {
    /// The row, as columns.
    text: Vec<Cell>,
    /// The column just after the message, where a merge line starts.
    message_end: usize,
}

/// Lays a row's own text out: its bracket, hash, creator and message.
fn placed(row: &Row, glyphs: Option<Glyphs>, hue: Hue) -> Placed {
    let mut line: Vec<Cell> = Vec::new();

    if let Some(glyphs) = glyphs {
        if let Some(text) = glyphs.bracket(row.bracket) {
            write(&mut line, BRACKET, text, hue.ink());
        }

        if row.origin {
            put(&mut line, LABEL - 1, '[', Ink::plain());
        }
    }

    write(&mut line, LABEL, &row.label, hue.hashed());

    if row.origin && glyphs.is_some() {
        put(&mut line, LABEL + HASH, ']', Ink::plain());
    }

    let creator = row.creator.width();
    let colon = if creator > 0 {
        let at = COLON.saturating_sub(creator).max(LABEL + HASH + 1);
        write(&mut line, at, &row.creator, Ink::plain());
        put(&mut line, at + creator, ':', Ink::plain());
        Some(at + creator)
    } else {
        None
    };

    let start = colon.map_or(LABEL + HASH + 1, |colon| MESSAGE.max(colon + 2));
    let said = if row.message.is_empty() {
        String::new()
    } else if row.mood.is_some() {
        // What is not recorded has no message: the word says what it is, and quoting it would say
        // it is words somebody wrote.
        row.message.clone()
    } else {
        format!("\"{}\"", row.message)
    };
    let ink = if row.mood.is_some() {
        hue.ink()
    } else {
        hue.said()
    };
    let end = write(&mut line, start, &said, ink);

    Placed {
        text: line,
        message_end: end.max(start),
    }
}

/// One row as the compact drawing shows it: the same text, and the joins named after it.
fn compact(row: &Row) -> String {
    // The compact drawing is a list rather than a shape, so where a row came from is no longer the
    // thing being read: a variant from outside is drawn like any other.
    let hue = match row.hue {
        Hue::Outside => Hue::Chain,
        hue => hue,
    };

    let placed = placed(row, None, hue);
    let mut text = placed.text;
    let mut end = placed.message_end;

    if let Some(join) = &row.join {
        let ink = if row.join_drawn {
            hue.ink()
        } else {
            Hue::Outside.ink()
        };
        end = write(
            &mut text,
            end,
            &format!(" (join: {})", short_hash(join)),
            ink,
        );
    }

    if row.origin {
        let _ = write(
            &mut text,
            end,
            &format!(" ({})", origin_word()),
            Ink::origin(),
        );
    }

    render(&text)
}

/// One merge line: where it leaves, where it turns down, and where it lands.
struct Join {
    /// The row it leaves, and its arrowhead's column.
    source: (usize, usize),
    /// The row it lands on, when that row is drawn below it.
    target: Option<usize>,
    /// Whether the variant it names is drawn nowhere, which means it came in from outside the drawing.
    outside: bool,
    /// The column it turns down at.
    turn: usize,
    /// The column a landing row's own line begins at.
    landing: usize,
}

/// The merge lines a drawing has, with the columns they nest in.
fn joins(rows: &[Row], placed: &[Placed]) -> Vec<Join> {
    let at: HashMap<&str, usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.hash.is_empty())
        .map(|(index, row)| (row.hash.as_str(), index))
        .collect();

    let mut joins: Vec<Join> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(join) = &row.join else {
            continue;
        };

        let found = at.get(join.as_str()).copied();

        // A line only turns down into a row below it. A variant merged in from the row above is
        // named by its hash instead: a line that ran back up through the rows would cross the ones
        // it passes, and the bracket already says both are based on one version.
        let landing = found.filter(|target| *target > index);

        let start = placed[index].message_end + 1;
        let landing_from = landing.map_or(0, |at| placed[at].message_end + 1);

        joins.push(Join {
            source: (index, start),
            target: landing,
            outside: found.is_none(),
            turn: 0,
            landing: landing_from,
        });
    }

    // A line above another turns further right, so the lower line's own run reaches its turn
    // without crossing the one that passes over it.
    let mut below: Option<usize> = None;
    for join in joins.iter_mut().rev() {
        let least = join
            .source
            .1
            .saturating_add(REACH)
            .max(join.landing)
            .max(below.map_or(0, |turn| turn + 1));

        join.turn = least;
        below = Some(least);
    }

    joins
}

/// The drawing as columns: every row's text, every merge line, and the line over a Vault's version.
fn assembled(rows: &[Row], placed: &[Placed], joins: &[Join], glyphs: Glyphs) -> Vec<Vec<Cell>> {
    // The line that marks a Vault's version is a row of its own, above the row it marks.
    let mut canvas: Vec<Vec<Cell>> = Vec::new();
    let mut at: Vec<usize> = Vec::with_capacity(rows.len());
    let mut origins: Vec<usize> = Vec::new();

    for (index, row) in rows.iter().enumerate() {
        if row.origin {
            origins.push(canvas.len());
            canvas.push(Vec::new());
        }

        at.push(canvas.len());
        canvas.push(placed[index].text.clone());
    }

    // The merges, drawn over whatever row they pass.
    for join in joins {
        let row = &rows[join.source.0];
        let ink = row.hue.ink();
        // A variant that is not drawn at all came in from outside the chain, so the line that names
        // it is drawn in the colour of one, arrowhead and all.
        let line_ink = if join.outside {
            Hue::Outside.ink()
        } else {
            ink
        };
        let line = &mut canvas[at[join.source.0]];
        put(line, join.source.1, '<', line_ink);

        if let Some(target) = join.target {
            for column in (join.source.1 + 1)..join.turn {
                put(line, column, '-', ink);
            }
            put(line, join.turn, glyphs.down, ink);

            for row in canvas
                .iter_mut()
                .take(at[target])
                .skip(at[join.source.0] + 1)
            {
                put(row, join.turn, glyphs.drop, rows[target].hue.ink());
            }

            let line = &mut canvas[at[target]];
            for column in join.landing..join.turn {
                put(line, column, '-', rows[target].hue.ink());
            }
            put(line, join.turn, glyphs.up, rows[target].hue.ink());
        } else {
            for column in (join.source.1 + 1)..=(join.source.1 + HASH_LEAD) {
                put(line, column, '-', line_ink);
            }
            let said = format!(" {}", short_hash(row.join.as_deref().unwrap_or_default()));
            write(line, join.source.1 + HASH_LEAD + 1, &said, line_ink);
        }
    }

    for row in origins {
        let word = origin_word();
        let start = ORIGIN_END.saturating_sub(word.width());
        write(&mut canvas[row], start, &word, Ink::origin());
        put(&mut canvas[row], ORIGIN_END + 1, '-', Ink::origin());
        put(&mut canvas[row], LABEL, glyphs.down, Ink::origin());
    }

    canvas
}

/// Puts `ch` at `column`, growing the line as far as it has to.
fn put(line: &mut Vec<Cell>, column: usize, ch: char, ink: Ink) {
    while line.len() <= column {
        line.push(Cell::blank());
    }

    if line[column].covered {
        return;
    }

    line[column] = Cell {
        ch,
        ink,
        covered: false,
    };
}

/// Writes `text` from `column`, and answers the column just after it.
fn write(line: &mut Vec<Cell>, column: usize, text: &str, ink: Ink) -> usize {
    let mut at = column;

    for ch in text.chars() {
        let wide = ch.width().unwrap_or(1).max(1);
        put(line, at, ch, ink);

        if wide > 1 {
            while line.len() <= at + 1 {
                line.push(Cell::blank());
            }
            line[at + 1] = Cell {
                ch: ' ',
                ink,
                covered: true,
            };
        }

        at += wide;
    }

    at
}

/// One line of the drawing as the terminal is given it.
fn render(line: &[Cell]) -> String {
    let end = line
        .iter()
        .rposition(|cell| cell.covered || cell.ch != ' ')
        .map_or(0, |at| at + 1);

    let mut said = String::new();
    let mut at = 0;
    while at < end {
        let ink = line[at].ink;
        let mut run = String::new();
        while at < end && line[at].ink == ink {
            if !line[at].covered {
                run.push(line[at].ch);
            }
            at += 1;
        }

        if !run.is_empty() {
            said.push_str(&styled(&run, ink));
        }
    }

    said
}

/// `text` in `ink`, as the terminal is given it.
fn styled(text: &str, ink: Ink) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut painted = text.normal();
    painted = match ink.hue {
        Hue::Plain => painted,
        Hue::Chain => painted.bright_yellow(),
        Hue::Merged => painted.bright_green(),
        Hue::Outside => painted.bright_magenta(),
        Hue::Work => painted.bright_cyan(),
        Hue::Origin => painted.bright_blue(),
    };
    if ink.bold {
        painted = painted.bold();
    }
    if ink.italic {
        painted = painted.italic();
    }

    painted.to_string()
}

/// The lines a terminal is given, in the two shapes the themes allow.
#[derive(Clone, Copy)]
struct Glyphs {
    /// The first row of a bracket.
    open: &'static str,
    /// A bracket's middle row.
    middle: &'static str,
    /// The last row of a bracket whose variant the chain continues down from.
    close_spine: &'static str,
    /// The last row of a bracket with no spine in it.
    close: &'static str,
    /// The corner a merge line turns down at.
    down: char,
    /// The corner a merge line lands with.
    up: char,
    /// A merge line's drop.
    drop: char,
}

impl Glyphs {
    /// The glyphs `choice` draws with: box drawing where the terminal carries it, ASCII everywhere else.
    const fn of(choice: ThemeChoice) -> Self {
        match choice {
            ThemeChoice::Pretty => Self {
                open: "╭- - - ->",
                middle: "|- - - ->",
                close_spine: "╰-------<",
                close: "╰-------",
                down: '╮',
                up: '╯',
                drop: '│',
            },
            ThemeChoice::Text | ThemeChoice::Simple | ThemeChoice::DesktopFriendly => Self {
                open: ",- - - ->",
                middle: "|- - - ->",
                close_spine: "`-------<",
                close: "`-------",
                down: '.',
                up: '\'',
                drop: '|',
            },
        }
    }

    /// The bracket a row wears, or nothing when it wears none.
    const fn bracket(self, bracket: Bracket) -> Option<&'static str> {
        match bracket {
            Bracket::None => None,
            Bracket::Open => Some(self.open),
            Bracket::Middle => Some(self.middle),
            Bracket::Close => Some(self.close_spine),
            Bracket::ClosePlain => Some(self.close),
        }
    }
}

#[cfg(test)]
mod tests {
    use librorolala::vcs::{LevelView, Origin, TopKind, VariantView};
    use rorolala_utils_cli_theme::ThemeChoice;
    use unicode_width::{UnicodeWidthChar as _, UnicodeWidthStr as _};

    use super::{Glyphs, Options, draw_with};
    use crate::vcs_index::cmd_vcs_index_lookback::{
        MergeHint, ResultVcsIndexLookback, Standing, StandingRelation,
    };

    /// A variant named by `hash` that says `message`.
    fn variant(hash: &str, creator: &str, message: &str) -> VariantView {
        VariantView {
            hash: hash.to_owned(),
            join: None,
            creator: creator.to_owned(),
            message: message.to_owned(),
        }
    }

    /// The same variant, merging `join` in.
    fn merging(hash: &str, creator: &str, message: &str, join: &str) -> VariantView {
        VariantView {
            join: Some(join.to_owned()),
            ..variant(hash, creator, message)
        }
    }

    /// A level: its version's number, its spine, and what it merges in.
    fn level(number: u64, spine: VariantView, merged: Vec<VariantView>) -> LevelView {
        LevelView {
            number,
            variant: spine,
            merged,
        }
    }

    /// The chain a test draws, without a file behind it.
    fn chain(levels: Vec<LevelView>, first: Option<VariantView>) -> ResultVcsIndexLookback {
        ResultVcsIndexLookback {
            levels,
            top: TopKind::Version,
            origin: None,
            first,
            standing: None,
            unjoined: Vec::new(),
        }
    }

    /// The drawing, with the box-drawing glyphs, as the plain text of each line.
    fn shown(result: &ResultVcsIndexLookback) -> Vec<String> {
        drawn(result, Options::default())
    }

    /// The drawing a run with `options` makes, as the plain text of each line.
    fn drawn(result: &ResultVcsIndexLookback, options: Options) -> Vec<String> {
        draw_with(result, &options, Glyphs::of(ThemeChoice::Pretty))
            .iter()
            .map(|line| plain(line))
            .collect()
    }

    /// A line without the codes the terminal is given.
    fn plain(line: &str) -> String {
        let mut said = String::new();
        let mut skipping = false;
        for ch in line.chars() {
            if skipping {
                skipping = ch != 'm';
                continue;
            }
            if ch == '\u{1b}' {
                skipping = true;
                continue;
            }
            said.push(ch);
        }

        said
    }

    /// The display column `needle` begins at in `line`.
    fn column(line: &str, needle: char) -> usize {
        let mut at = 0;
        for ch in line.chars() {
            if ch == needle {
                return at;
            }
            at += ch.width().unwrap_or(1);
        }

        panic!("{needle:?} is not in {line:?}");
    }

    #[test]
    fn a_chain_of_one_variant_is_one_row() {
        let result = chain(Vec::new(), Some(variant("a1b2c3d", "alice", "one")));

        let lines = shown(&result);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("           a1b2c3d"), "{:?}", lines[0]);
        assert!(lines[0].contains("alice: \"one\""), "{:?}", lines[0]);
        assert!(!lines[0].contains('╭') && !lines[0].contains('╰'));
    }

    #[test]
    fn a_group_draws_its_merged_variants_above_its_spine() {
        let result = chain(
            vec![
                level(
                    2,
                    merging("b200000", "alice", "joins c", "c000000"),
                    Vec::new(),
                ),
                level(
                    1,
                    merging("b100000", "alice", "joins d", "d000000"),
                    Vec::new(),
                ),
                level(
                    0,
                    variant("e000000", "alice", "the base"),
                    vec![
                        variant("c000000", "bob", "first merged"),
                        variant("d000000", "bob", "second merged"),
                    ],
                ),
            ],
            Some(variant("a000000", "alice", "the first")),
        );

        let lines = shown(&result);

        assert_eq!(lines.len(), 6);
        assert!(lines[2].starts_with(" ╭- - - -> c000000"), "{:?}", lines[2]);
        assert!(lines[3].starts_with(" |- - - -> d000000"), "{:?}", lines[3]);
        assert!(lines[4].starts_with(" ╰-------< e000000"), "{:?}", lines[4]);
        assert!(lines[5].starts_with("           a000000"), "{:?}", lines[5]);
    }

    #[test]
    fn a_merge_lands_on_the_row_it_merges_in() {
        let result = chain(
            vec![
                level(
                    2,
                    merging("b200000", "alice", "joins", "c000000"),
                    Vec::new(),
                ),
                level(
                    1,
                    merging("b100000", "alice", "joins too", "d000000"),
                    Vec::new(),
                ),
                level(
                    0,
                    variant("e000000", "alice", "the base"),
                    vec![
                        variant("c000000", "bob", "merged high"),
                        variant("d000000", "bob", "merged low"),
                    ],
                ),
            ],
            Some(variant("a000000", "alice", "the first")),
        );

        let lines = shown(&result);

        // Every row, then the two merges: the upper one turns further right than the lower one.
        let upper = column(&lines[0], '╮');
        let lower = column(&lines[1], '╮');
        assert!(upper > lower, "{upper} is not right of {lower}");

        // The upper merge lands on the first merged variant, the lower on the second.
        assert_eq!(column(&lines[2], '╯'), upper);
        assert_eq!(column(&lines[3], '╯'), lower);
        assert!(lines[2].contains("bob: \"merged high\""), "{:?}", lines[2]);
    }

    #[test]
    fn a_merge_into_a_variant_not_drawn_ends_in_its_hash() {
        let result = chain(
            vec![level(
                1,
                merging("b000000", "alice", "joins", "fffffff"),
                Vec::new(),
            )],
            Some(variant("a000000", "alice", "the first")),
        );

        let lines = shown(&result);

        assert!(lines[0].contains("<--------- fffffff"), "{:?}", lines[0]);
        assert!(!lines[0].contains('╮'));
    }

    #[test]
    fn a_merge_of_a_variant_above_it_ends_in_its_hash() {
        let result = chain(
            vec![level(
                1,
                merging("b000000", "alice", "joins", "c000000"),
                vec![variant("c000000", "bob", "merged")],
            )],
            Some(variant("a000000", "alice", "the first")),
        );

        let lines = shown(&result);

        // The variant merged in is based on the same version, so it has no row of its own: the line
        // that merges it in names it by hash, and there is no group to bracket.
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("<--------- c000000"), "{:?}", lines[0]);
        assert!(!lines[0].contains('╭'), "{:?}", lines[0]);
        assert!(!lines.iter().any(|line| line.contains('╮')));
    }

    #[test]
    fn a_wide_message_pushes_the_merge_line_along() {
        let narrow = chain(
            vec![level(
                1,
                merging("b000000", "alice", "join", "fffffff"),
                Vec::new(),
            )],
            Some(variant("a000000", "alice", "the first")),
        );
        let wide = chain(
            vec![level(
                1,
                merging("b000000", "alice", "加入", "fffffff"),
                Vec::new(),
            )],
            Some(variant("a000000", "alice", "the first")),
        );

        let narrow = shown(&narrow);
        let wide = shown(&wide);

        assert_eq!(column(&narrow[0], '<'), 39 + "\"join\"".width() + 1);
        assert_eq!(column(&wide[0], '<'), 39 + "\"加入\"".width() + 1);
    }

    #[test]
    fn a_wide_creator_is_right_aligned_by_display_columns() {
        let result = chain(Vec::new(), Some(variant("a000000", "张三", "one")));

        let lines = shown(&result);

        assert_eq!(column(&lines[0], ':'), 37);
        assert_eq!(column(&lines[0], '"'), 39);
    }

    #[test]
    fn a_merge_still_to_be_made_sits_over_the_chain() {
        let mut result = chain(
            vec![level(
                0,
                variant("e000000", "alice", "the base"),
                vec![variant("c000000", "bob", "waiting")],
            )],
            Some(variant("a000000", "alice", "the first")),
        );
        result.standing = Some(Standing {
            vault: String::new(),
            source: None,
            holder: Some("alice".to_owned()),
            mine: true,
            modified: true,
            relation: StandingRelation::Unknown,
            distance: None,
            merging: Some(MergeHint {
                variant: "c000000".to_owned(),
                path: "hero.txt".to_owned(),
            }),
        });

        let lines = shown(&result);

        assert!(lines[0].starts_with("           ???????"), "{:?}", lines[0]);
        assert!(lines[0].contains("?????: [JOIN]"), "{:?}", lines[0]);
        assert_eq!(column(&lines[0], '╮'), column(&lines[1], '╯'));
    }

    #[test]
    fn the_vaults_version_is_marked_with_a_line_over_it() {
        let mut result = chain(
            vec![level(
                1,
                variant("b000000", "alice", "the spine"),
                Vec::new(),
            )],
            Some(variant("a000000", "alice", "the first")),
        );
        result.origin = Some(Origin {
            number: 2,
            vault: "it.vault".to_owned(),
        });

        let lines = shown(&result);

        // Version 2 is the one the level 1 spine produced, which is drawn first.
        assert!(lines[0].contains("ORIGIN -╮"), "{:?}", lines[0]);
        assert!(lines[1].contains("[b000000]"), "{:?}", lines[1]);
    }

    #[test]
    fn a_compact_drawing_is_one_line_a_variant() {
        let result = chain(
            vec![
                level(
                    1,
                    merging("b100000", "alice", "joins", "c000000"),
                    Vec::new(),
                ),
                level(
                    0,
                    variant("e000000", "alice", "the base"),
                    vec![variant("c000000", "bob", "merged")],
                ),
            ],
            Some(variant("a000000", "alice", "the first")),
        );

        let lines = drawn(
            &result,
            Options {
                compact: true,
                ..Options::default()
            },
        );

        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains(" (join: c000000)"), "{:?}", lines[0]);
        assert!(
            !lines
                .iter()
                .any(|line| line.contains('╭') || line.contains('╮') || line.contains('|'))
        );
    }

    #[test]
    fn a_compact_drawing_names_the_vaults_version() {
        let mut result = chain(Vec::new(), Some(variant("a000000", "alice", "one")));
        result.origin = Some(Origin {
            number: 0,
            vault: "it.vault".to_owned(),
        });

        let lines = drawn(
            &result,
            Options {
                compact: true,
                ..Options::default()
            },
        );

        assert!(lines[0].contains("(ORIGIN)"), "{:?}", lines[0]);
    }

    #[test]
    fn the_ascii_glyphs_are_ascii() {
        let glyphs = Glyphs::of(ThemeChoice::Simple);
        let drawn = [glyphs.open, glyphs.middle, glyphs.close_spine, glyphs.close];

        assert!(drawn.iter().all(|text| text.is_ascii()));
        assert!(glyphs.down.is_ascii() && glyphs.up.is_ascii() && glyphs.drop.is_ascii());
    }

    #[test]
    fn a_message_that_fits_is_shown_whole() {
        let result = chain(Vec::new(), Some(variant("a000000", "alice", "one")));
        let options = Options {
            max_message_length: 3,
            ..Options::default()
        };

        let lines = drawn(&result, options);

        assert!(lines[0].contains("\"one\""), "{:?}", lines[0]);
    }

    #[test]
    fn a_message_that_does_not_fit_ends_in_an_ellipsis() {
        let result = chain(Vec::new(), Some(variant("a000000", "alice", "lengthy")));
        let options = Options {
            max_message_length: 6,
            ..Options::default()
        };

        let lines = drawn(&result, options);

        assert!(lines[0].contains("\"len...\""), "{:?}", lines[0]);
    }
}

//! Pipe tables, drawn by `prettytable`.

use prettytable::format::Alignment;
use prettytable::format::TableFormat;
use prettytable::format::consts::{FORMAT_BOX_CHARS, FORMAT_DEFAULT};
use prettytable::{Cell, Row, Table};

use crate::ThemeChoice;

use super::ansi::{display_width, escape_count};
use super::inline;
use super::palette::{RenderOptions, StyleState};

/// How few `-` a separator cell may hold and still separate anything.
const RULE: usize = 3;

/// How much space the format puts either side of a cell's content.
///
/// Both formats this module draws with use one column on each side, and the widths a
/// rule is drawn from have to include them.
const PADDING: usize = 1;

/// An escape sequence that draws nothing.
///
/// `ESC [ m` is a reset with nothing to reset. It is here to be counted, which is what
/// [`even_out`] explains.
const NOTHING: &str = "\u{1b}[m";

/// Whether the line at `index` opens a table.
///
/// A table opens where a row is followed by the row that says how wide each column is,
/// which is what tells a table apart from a line that happens to hold a `|`.
#[must_use]
pub(super) fn starts(lines: &[&str], index: usize) -> bool {
    lines.get(index).is_some_and(|line| is_row(line))
        && lines.get(index + 1).is_some_and(|line| is_separator(line))
}

/// Draws the table at `index`, and says where the next block begins.
#[must_use]
pub(super) fn render(lines: &[&str], index: usize, options: RenderOptions) -> (Vec<String>, usize) {
    // A link is drawn with an `ESC ] ...` sequence, which `prettytable` does not skip at
    // all: it counts every character of the address as a column. Inside a table a link is
    // therefore drawn as the text it holds, and the address is dropped.
    let options = RenderOptions {
        hyperlinks: false,
        ..options
    };

    let alignments: Vec<Alignment> = split(lines[index + 1])
        .iter()
        .map(|cell| alignment(cell))
        .collect();

    // The header is a row like any other, so both alphabets draw one line under it and
    // the two differ only in what the lines are made of.
    let mut rows = vec![drawn_row(&split(lines[index]), &alignments, options)];
    let mut next = index + 2;
    while next < lines.len() && is_row(lines[next]) {
        rows.push(drawn_row(&split(lines[next]), &alignments, options));
        next += 1;
    }
    even_out(&mut rows);
    let widths = column_widths(&rows);

    let mut table = Table::new();
    table.set_format(format(options.theme));
    for row in rows {
        table.add_row(Row::new(cells(row, &alignments)));
    }

    let mut rendered: Vec<String> = table.to_string().lines().map(str::to_owned).collect();
    while rendered.last().is_some_and(String::is_empty) {
        rendered.pop();
    }
    redraw_rules(&mut rendered, &widths, frame(options.theme));

    (rendered, next)
}

/// One cell of a table, drawn but not yet handed over.
struct Drawn {
    /// What the cell holds, escapes and all.
    content: String,
    /// How wide it is on screen, escapes discounted.
    width: usize,
    /// How many escape sequences it holds.
    escapes: usize,
}

/// One row, with every cell drawn in `options`.
fn drawn_row(row: &[&str], alignments: &[Alignment], options: RenderOptions) -> Vec<Drawn> {
    (0..alignments.len())
        .map(|column| {
            let text = row.get(column).copied().unwrap_or_default();
            let content = inline::render(text, StyleState::plain(), options);
            Drawn {
                width: display_width(&content),
                escapes: escape_count(&content),
                content,
            }
        })
        .collect()
}

/// How wide each column really is: the widest cell in it, escapes discounted.
fn column_widths(rows: &[Vec<Drawn>]) -> Vec<usize> {
    let columns = rows.first().map_or(0, Vec::len);
    (0..columns)
        .map(|column| rows.iter().map(|row| row[column].width).max().unwrap_or(0))
        .collect()
}

/// Gives every cell in a column the same number of escape sequences.
///
/// `prettytable` measures a column by the widest cell in it, and it measures a cell by
/// taking the payload of each `ESC [ ... m` off the width — but leaving the `ESC`
/// itself, which `unicode-width` counts as one column. A cell drawn in a colour is
/// therefore measured one column too wide for every sequence it holds, and the padding
/// a row gets is short by the difference between its cells.
///
/// A cell has no width to set, so what is evened out is the count: every cell is given
/// as many sequences as the widest count in its column, the extras being [`NOTHING`].
/// Each cell then measures as its own width plus the column's largest count — the same
/// number for the whole column, so it cancels out of the padding, and every row ends up
/// as wide as the column with its cells in the right places. What is left over is the
/// rules, which are drawn from the measurement itself; [`redraw_rules`] takes those.
fn even_out(rows: &mut [Vec<Drawn>]) {
    let columns = rows.first().map_or(0, Vec::len);
    for column in 0..columns {
        let most = rows
            .iter()
            .map(|row| row[column].escapes)
            .max()
            .unwrap_or(0);
        for row in rows.iter_mut() {
            let cell = &mut row[column];
            for _ in cell.escapes..most {
                cell.content.push_str(NOTHING);
            }
        }
    }
}

/// Draws the rules again, at the width each column really takes up.
///
/// The other half of [`even_out`]: a rule is drawn as wide as the column was *measured*,
/// and a column that holds escape sequences measured wider than it is, so every rule
/// comes out wider than the rows under it and the corners do not line up. The rows are
/// the authority now — they are even, and their widths are known — so the rules are
/// drawn from those widths instead.
fn redraw_rules(rendered: &mut [String], widths: &[usize], frame: Frame) {
    let rules: Vec<usize> = rendered
        .iter()
        .enumerate()
        .filter(|(_, line)| is_rule(line, frame))
        .map(|(index, _)| index)
        .collect();
    let last = rules.len().saturating_sub(1);

    for (position, index) in rules.into_iter().enumerate() {
        let corners = match position {
            0 => frame.above,
            _ if position == last => frame.below,
            _ => frame.between,
        };
        rendered[index] = rule(widths, frame, corners);
    }
}

/// Draws a rule across a table whose columns are `widths` wide.
fn rule(widths: &[usize], frame: Frame, corners: [char; 3]) -> String {
    let mut drawn = String::new();
    drawn.push(corners[0]);
    for (column, width) in widths.iter().enumerate() {
        if column > 0 {
            drawn.push(corners[1]);
        }
        for _ in 0..width + PADDING * 2 {
            drawn.push(frame.line);
        }
    }
    drawn.push(corners[2]);
    drawn
}

/// Whether `line` is one of the table's rules rather than one of its rows.
///
/// A row holds a separator and spaces and text; a rule holds nothing but the frame's own
/// characters, and none of those is ever the space a row is padded with.
fn is_rule(line: &str, frame: Frame) -> bool {
    !line.is_empty()
        && line.chars().all(|found| {
            found == frame.line
                || found == frame.column
                || frame.above.contains(&found)
                || frame.between.contains(&found)
                || frame.below.contains(&found)
        })
}

/// The characters a table's frame is drawn with, in one alphabet.
#[derive(Clone, Copy)]
struct Frame {
    /// The character between two cells.
    column: char,
    /// The character a rule is made of.
    line: char,
    /// What a rule draws at the left border, at a column and at the right border —
    /// above the table, between two rows, and below it.
    above: [char; 3],
    /// Between two rows.
    between: [char; 3],
    /// Below the table.
    below: [char; 3],
}

/// The frame every terminal carries.
const PLAIN: Frame = Frame {
    column: '|',
    line: '-',
    above: ['+'; 3],
    between: ['+'; 3],
    below: ['+'; 3],
};

/// The frame a theme that carries the glyphs draws.
const BOX: Frame = Frame {
    column: '│',
    line: '─',
    above: ['┌', '┬', '┐'],
    between: ['├', '┼', '┤'],
    below: ['└', '┴', '┘'],
};

/// How a table is drawn, in two halves that have to agree.
///
/// `prettytable` lays the rows out and keeps its characters to itself, so what it is
/// asked for is the format; the rules are drawn back by this module, so what it is
/// asked for is the frame. A theme that can carry the box-drawing characters gets the
/// box, and every other theme gets the `|` and `-` that every terminal carries.
fn format(theme: ThemeChoice) -> TableFormat {
    if theme == ThemeChoice::Pretty {
        *FORMAT_BOX_CHARS
    } else {
        *FORMAT_DEFAULT
    }
}

/// The frame that goes with [`format`].
const fn frame(theme: ThemeChoice) -> Frame {
    if matches!(theme, ThemeChoice::Pretty) {
        BOX
    } else {
        PLAIN
    }
}

/// The cells of one row, aligned as the separator row asked.
fn cells(row: Vec<Drawn>, alignments: &[Alignment]) -> Vec<Cell> {
    row.into_iter()
        .zip(alignments)
        .map(|(cell, alignment)| Cell::new_align(&cell.content, *alignment))
        .collect()
}

/// How the column whose separator cell is `cell` is aligned, as Markdown writes it.
fn alignment(cell: &str) -> Alignment {
    let cell = cell.trim();
    match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Alignment::CENTER,
        (false, true) => Alignment::RIGHT,
        _ => Alignment::LEFT,
    }
}

/// Whether `line` is a row of a table.
fn is_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// Whether `line` is the row that says how wide each column is.
fn is_separator(line: &str) -> bool {
    let cells = split(line);
    !cells.is_empty() && cells.iter().all(|cell| is_separator_cell(cell))
}

/// Whether `cell` says a column is as wide as its contents.
fn is_separator_cell(cell: &str) -> bool {
    let dashes = cell.trim().trim_matches(':');
    dashes.chars().count() >= RULE && dashes.chars().all(|found| found == '-')
}

/// The cells of a row, without the `|` that hold them.
fn split(line: &str) -> Vec<&str> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(str::trim).collect()
}

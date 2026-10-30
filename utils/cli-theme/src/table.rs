//! Tables drawn across the terminal, in the shape a listing such as `gh issue list` takes.
//!
//! A table is columns and rows. A column is sized from what it holds, told exactly how wide to be,
//! or told to take whatever the other columns leave; every line is padded to the whole width, so
//! the table fills the terminal instead of stopping at the last word of the widest row. There are
//! no borders: the header is bold and underlined, and the columns are separated by two spaces.
//!
//! A cell is a sequence of runs, so a caller can draw part of it another way — the word a search
//! matched, say — without handing this module a colour language to parse. Widths are terminal
//! columns: a character that takes two on screen counts as two, so a table of Chinese text lines
//! up the same way one of Latin text does.
//!
//! ```rust
//! use rorolala_utils_cli_theme::{Cell, Column, Table};
//!
//! let mut table = Table::new([Column::auto("Name"), Column::fill("Description")]);
//! table.row([Cell::text("rola"), Cell::text("a version control system for binary assets")]);
//!
//! let drawn = table.render_with_width(60);
//! assert!(drawn.lines().all(|line| line.chars().count() <= 60));
//! ```

use colored::Colorize as _;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// How many blank columns separate two columns.
const GAP: usize = 2;

/// How wide a table is drawn when there is no terminal to ask.
const FALLBACK_COLUMNS: usize = 100;

/// How little a filled column may be left for itself when the columns before it are as wide as
/// they need.
const MIN_FILL: usize = 8;

/// How a column is sized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnWidth {
    /// As wide as its widest line needs.
    Auto,
    /// Exactly this many terminal columns.
    Fixed(usize),
    /// Whatever the other columns leave.
    Fill,
}

/// A column of a [`Table`]: its title and how it is sized.
#[derive(Debug, Clone)]
pub struct Column {
    /// What the column is headed with.
    title: String,
    /// How wide it is drawn.
    width: ColumnWidth,
}

impl Column {
    /// A column as wide as its widest line needs.
    #[must_use]
    pub fn auto(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: ColumnWidth::Auto,
        }
    }

    /// A column exactly `width` terminal columns wide.
    #[must_use]
    pub fn fixed(title: impl Into<String>, width: usize) -> Self {
        Self {
            title: title.into(),
            width: ColumnWidth::Fixed(width),
        }
    }

    /// A column taking whatever the other columns leave.
    #[must_use]
    pub fn fill(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: ColumnWidth::Fill,
        }
    }
}

/// How one run of a cell is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RunStyle {
    /// As ordinary text.
    #[default]
    Plain,
    /// Bold.
    Bold,
    /// Underlined.
    Underline,
    /// Red and bold, as the word a search matched is drawn.
    Highlight,
}

/// One run of a cell: text drawn one way.
#[derive(Debug, Clone)]
pub struct Run {
    /// What the run says.
    text: String,
    /// How it is drawn.
    style: RunStyle,
}

impl Run {
    /// A run of `text`, drawn in `style`.
    #[must_use]
    pub fn new(text: impl Into<String>, style: RunStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// One cell of a [`Table`]: text in runs, and the lines a run's `\n` makes.
#[derive(Debug, Clone, Default)]
pub struct Cell {
    /// The runs, in the order they are drawn.
    runs: Vec<Run>,
}

impl Cell {
    /// A cell of one plain run.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::runs([Run::new(text, RunStyle::Plain)])
    }

    /// A cell of `runs`, in the order they are drawn.
    #[must_use]
    pub fn runs(runs: impl IntoIterator<Item = Run>) -> Self {
        Self {
            runs: runs.into_iter().collect(),
        }
    }

    /// The cell's logical lines, as characters and the style each is drawn in.
    ///
    /// A `\n` in a run ends one line and starts the next; the newline itself is not drawn.
    fn lines(&self) -> Vec<Vec<(char, RunStyle)>> {
        let mut lines: Vec<Vec<(char, RunStyle)>> = vec![Vec::new()];

        for run in &self.runs {
            for character in run.text.chars() {
                if character == '\n' {
                    lines.push(Vec::new());
                } else if let Some(line) = lines.last_mut() {
                    line.push((character, run.style));
                }
            }
        }

        lines
    }

    /// The widest line the cell holds, before it is wrapped.
    fn natural(&self) -> usize {
        self.lines()
            .iter()
            .map(|line| width_of(line))
            .max()
            .unwrap_or(0)
    }
}

/// A table: columns, their titles, and the rows under them.
#[derive(Debug, Clone)]
pub struct Table {
    /// The columns, in the order they are drawn.
    columns: Vec<Column>,
    /// The rows; a row shorter than the columns is padded with empty cells.
    rows: Vec<Vec<Cell>>,
}

impl Table {
    /// A table with `columns`, and no rows yet.
    #[must_use]
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            rows: Vec::new(),
        }
    }

    /// Adds a row under the columns already given.
    ///
    /// A row that names fewer cells than there are columns is drawn with the rest empty; one that
    /// names more has the extra cells left out, since there is no column for them.
    pub fn row(&mut self, cells: impl IntoIterator<Item = Cell>) {
        self.rows.push(cells.into_iter().collect());
    }

    /// The table drawn across the terminal this program is writing to.
    ///
    /// A terminal that will not say how wide it is — a pipe, a file — is drawn 100 columns wide.
    #[must_use]
    pub fn render(&self) -> String {
        let width = console::Term::stdout()
            .size_checked()
            .map_or(FALLBACK_COLUMNS, |(_, columns)| usize::from(columns));

        self.render_with_width(width)
    }

    /// The table drawn across `width` terminal columns.
    ///
    /// Every line is exactly `width` columns wide, trailing padding included, so a table of short
    /// rows still fills the width it was given.
    #[must_use]
    pub fn render_with_width(&self, width: usize) -> String {
        if self.columns.is_empty() {
            return String::new();
        }

        let gaps = GAP.saturating_mul(self.columns.len().saturating_sub(1));
        let whole = width.max(self.columns.len().saturating_add(gaps));
        let widths = self.widths(whole);
        let mut lines = Vec::new();

        let header: Vec<Cell> = self
            .columns
            .iter()
            .map(|column| Cell::text(column.title.clone()))
            .collect();

        for line in wrapped_row(&header, &widths) {
            lines.push(header_line(&line, &widths, whole));
        }

        let empty = Cell::default();
        for row in &self.rows {
            let cells: Vec<&Cell> = (0..self.columns.len())
                .map(|at| row.get(at).unwrap_or(&empty))
                .collect();

            for line in wrapped_row_refs(&cells, &widths) {
                lines.push(row_line(&line, &widths, whole));
            }
        }

        lines.join("\n")
    }

    /// The width of each column, in terminal columns.
    fn widths(&self, whole: usize) -> Vec<usize> {
        let gaps = GAP.saturating_mul(self.columns.len().saturating_sub(1));
        let fills = self
            .columns
            .iter()
            .filter(|column| column.width == ColumnWidth::Fill)
            .count();

        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .enumerate()
            .map(|(at, column)| match column.width {
                ColumnWidth::Fixed(width) => width,
                ColumnWidth::Fill => 0,
                ColumnWidth::Auto => {
                    let mut natural = UnicodeWidthStr::width(column.title.as_str());

                    for row in &self.rows {
                        natural = natural.max(row.get(at).map_or(0, Cell::natural));
                    }

                    natural.max(1)
                }
            })
            .collect();

        if fills == 0 {
            return widths;
        }

        // A filled column is the one that has to be left room for: the columns before it give up
        // width, widest first, until the fill has the least it may have.
        loop {
            let used: usize = widths.iter().sum::<usize>() + gaps;
            let wanted = MIN_FILL.saturating_mul(fills);

            if used.saturating_add(wanted) <= whole {
                break;
            }

            let Some(widest) = (0..self.columns.len())
                .filter(|at| self.columns[*at].width == ColumnWidth::Auto && widths[*at] > 1)
                .max_by_key(|at| widths[*at])
            else {
                break;
            };

            if let Some(width) = widths.get_mut(widest) {
                *width -= 1;
            }
        }

        let used: usize = widths.iter().sum::<usize>() + gaps;
        let remaining = whole.saturating_sub(used);
        let each = remaining / fills;
        let mut extra = remaining % fills;

        for (at, column) in self.columns.iter().enumerate() {
            if column.width != ColumnWidth::Fill {
                continue;
            }

            let share = if extra > 0 {
                extra -= 1;
                each.saturating_add(1)
            } else {
                each
            };

            if let Some(width) = widths.get_mut(at) {
                *width = share;
            }
        }

        widths
    }
}

/// The lines of a row of cells, each already wrapped to its column.
fn wrapped_row(cells: &[Cell], widths: &[usize]) -> Vec<Vec<(String, usize)>> {
    let borrowed: Vec<&Cell> = cells.iter().collect();

    wrapped_row_refs(&borrowed, widths)
}

/// The lines of a row of cell references, each already wrapped to its column.
fn wrapped_row_refs(cells: &[&Cell], widths: &[usize]) -> Vec<Vec<(String, usize)>> {
    let mut wrapped: Vec<Vec<(String, usize)>> = cells
        .iter()
        .zip(widths)
        .map(|(cell, width)| wrapped_cell(cell, *width))
        .collect();

    let height = wrapped.iter().map(Vec::len).max().unwrap_or(0);

    for lines in &mut wrapped {
        lines.resize(height, (String::new(), 0));
    }

    let mut rows = vec![Vec::new(); height];
    for lines in wrapped {
        for (row, line) in rows.iter_mut().zip(lines) {
            row.push(line);
        }
    }

    rows
}

/// One cell wrapped to `width`, as drawn lines and the width each takes.
fn wrapped_cell(cell: &Cell, width: usize) -> Vec<(String, usize)> {
    let mut drawn = Vec::new();

    for line in cell.lines() {
        for wrapped in wrap(&line, width) {
            drawn.push(draw(&wrapped));
        }
    }

    if drawn.is_empty() {
        drawn.push((String::new(), 0));
    }

    drawn
}

/// The header line of `pieces`, bold and underlined across the whole width.
fn header_line(pieces: &[(String, usize)], widths: &[usize], whole: usize) -> String {
    let line = stitched(pieces, widths, whole);

    line.bold().underline().to_string()
}

/// One row line of `pieces`, padded out to the table's whole width.
fn row_line(pieces: &[(String, usize)], widths: &[usize], whole: usize) -> String {
    stitched(pieces, widths, whole)
}

/// `pieces` stitched into one line, every column padded and the line filled to `whole`.
fn stitched(pieces: &[(String, usize)], widths: &[usize], whole: usize) -> String {
    let mut line = String::new();
    let mut used = 0;

    for (at, (text, width)) in pieces.iter().enumerate() {
        line.push_str(text);
        used += width;

        if let Some(column) = widths.get(at) {
            let padding = column.saturating_sub(*width);

            line.extend(std::iter::repeat_n(' ', padding));
            used += padding;

            if at.saturating_add(1) < pieces.len() {
                line.extend(std::iter::repeat_n(' ', GAP));
                used += GAP;
            }
        }
    }

    line.extend(std::iter::repeat_n(' ', whole.saturating_sub(used)));

    line
}

/// `line` drawn, with each run of one style painted once.
fn draw(line: &[(char, RunStyle)]) -> (String, usize) {
    let mut drawn = String::new();
    let mut used = 0;
    let mut at = 0;

    while at < line.len() {
        let style = line[at].1;
        let start = at;

        while at < line.len() && line[at].1 == style {
            used += character_width(line[at].0);
            at += 1;
        }

        let text: String = line[start..at]
            .iter()
            .map(|(character, _)| *character)
            .collect();

        drawn.push_str(&painted(&text, style));
    }

    (drawn, used)
}

/// `text` drawn in `style`, as far as the program's colouring switch lets it.
fn painted(text: &str, style: RunStyle) -> String {
    match style {
        RunStyle::Plain => text.to_owned(),
        RunStyle::Bold => text.bold().to_string(),
        RunStyle::Underline => text.underline().to_string(),
        RunStyle::Highlight => text.red().bold().to_string(),
    }
}

/// `characters` wrapped into lines of at most `width` terminal columns.
///
/// Words are kept whole where they fit, and a word too long for a line is cut at the width. The
/// spaces between words are the wrapper's rather than the cell's, so a wrapped line never starts
/// or ends with one.
fn wrap(characters: &[(char, RunStyle)], width: usize) -> Vec<Vec<(char, RunStyle)>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line: Vec<(char, RunStyle)> = Vec::new();
    let mut used = 0;

    for word in words(characters) {
        let word_width: usize = word
            .iter()
            .map(|(character, _)| character_width(*character))
            .sum();

        if word_width <= width {
            let space = usize::from(!line.is_empty());

            if used + space + word_width > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }

            if !line.is_empty() {
                line.push((' ', RunStyle::Plain));
                used += 1;
            }

            line.extend(word);
            used += word_width;
            continue;
        }

        for (character, style) in word {
            let character_width = character_width(character);

            if used + character_width > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }

            line.push((character, style));
            used += character_width;
        }
    }

    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }

    lines
}

/// The words of `characters`: runs of non-space characters, each with its own styles.
fn words(characters: &[(char, RunStyle)]) -> Vec<Vec<(char, RunStyle)>> {
    let mut words = Vec::new();
    let mut word = Vec::new();

    for (character, style) in characters {
        if character.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }

        word.push((*character, *style));
    }

    if !word.is_empty() {
        words.push(word);
    }

    words
}

/// The width `characters` take on screen.
fn width_of(characters: &[(char, RunStyle)]) -> usize {
    characters
        .iter()
        .map(|(character, _)| character_width(*character))
        .sum()
}

/// How many terminal columns a character takes.
fn character_width(character: char) -> usize {
    UnicodeWidthChar::width(character).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthStr;

    use crate::support::{with_color, without_color};

    use super::{Cell, Column, Run, RunStyle, Table};

    /// A table of the given columns and one row, drawn at `width` without colouring.
    fn drawn(width: usize, table: &Table) -> Vec<String> {
        without_color(|| table.render_with_width(width))
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn every_line_is_as_wide_as_the_table() {
        let mut table = Table::new([Column::auto("Name"), Column::fill("Snippet")]);
        table.row([Cell::text("a"), Cell::text("short")]);

        for line in drawn(40, &table) {
            assert_eq!(UnicodeWidthStr::width(line.as_str()), 40, "{line:?}");
        }
    }

    #[test]
    fn a_column_is_as_wide_as_its_widest_line() {
        let mut table = Table::new([Column::auto("Name"), Column::fill("Snippet")]);
        table.row([Cell::text("alpha"), Cell::text("first")]);

        let lines = drawn(30, &table);

        assert_eq!(lines[0], format!("Name   Snippet{}", " ".repeat(16)));
        assert_eq!(lines[1], format!("alpha  first{}", " ".repeat(18)));
    }

    #[test]
    fn a_filled_column_holds_what_the_others_leave() {
        let mut table = Table::new([Column::fixed("N", 3), Column::fill("Text")]);
        table.row([Cell::text("ab"), Cell::text("one two three four five")]);

        let lines = drawn(20, &table);

        for line in &lines {
            assert_eq!(UnicodeWidthStr::width(line.as_str()), 20, "{line:?}");
        }

        // The filled column is 20 - 3 - 2 = 15 columns, so the words after the first line are
        // wrapped inside it.
        assert!(lines[0].starts_with("N    Text"), "{lines:?}");
        assert!(lines.len() > 2, "the fragment should wrap: {lines:?}");
    }

    #[test]
    fn a_cell_with_a_newline_is_drawn_on_several_lines() {
        let mut table = Table::new([Column::auto("Name"), Column::fill("Snippet")]);
        table.row([
            Cell::text("one"),
            Cell::runs([
                Run::new("first", RunStyle::Plain),
                Run::new("\n", RunStyle::Plain),
                Run::new("second", RunStyle::Plain),
            ]),
        ]);

        let lines = drawn(30, &table);

        assert!(lines[0].contains("Snippet"));
        assert!(lines[1].contains("first"));
        assert!(lines[2].contains("second"));
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn a_wide_character_takes_two_columns() {
        let mut table = Table::new([Column::auto("名字"), Column::fill("说明")]);
        table.row([Cell::text("书"), Cell::text("安装说明")]);

        for line in drawn(30, &table) {
            assert_eq!(UnicodeWidthStr::width(line.as_str()), 30, "{line:?}");
        }
    }

    #[test]
    fn a_row_shorter_than_the_columns_is_padded_with_nothing() {
        let mut table = Table::new([Column::auto("Name"), Column::fill("Snippet")]);
        table.row([Cell::text("only")]);

        let lines = drawn(20, &table);

        assert!(lines[1].starts_with("only"), "{lines:?}");
    }

    #[test]
    fn the_header_is_drawn_in_bold_and_underlined() {
        let mut table = Table::new([Column::auto("Name"), Column::fill("Snippet")]);
        table.row([Cell::text("a"), Cell::text("b")]);

        let lines = with_color(|| table.render_with_width(30))
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        assert!(lines[0].contains("\u{1b}[1;4m"), "{:?}", lines[0]);
        assert!(lines[0].ends_with("\u{1b}[0m"), "{:?}", lines[0]);
    }

    #[test]
    fn a_highlighted_run_is_drawn_red_and_bold() {
        let mut table = Table::new([Column::auto("Name"), Column::fill("Snippet")]);
        table.row([
            Cell::text("a"),
            Cell::runs([
                Run::new("one ", RunStyle::Plain),
                Run::new("two", RunStyle::Highlight),
            ]),
        ]);

        let lines = with_color(|| table.render_with_width(30))
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        assert!(lines[1].contains("\u{1b}[1;31mtwo"), "{:?}", lines[1]);
    }
}

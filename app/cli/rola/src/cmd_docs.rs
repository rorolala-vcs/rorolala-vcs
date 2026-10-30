//! The `rola docs` command: an article of the built-in book, or the articles a word finds.
//!
//! The book is one file, `docs/builtin.book`, laid beside the programs by an export or an install;
//! it is read whole and searched in memory. This command is the reader over it:
//!
//! - `rola docs <TEXT>` names one article exactly. It is printed as it was written, drawn by the
//!   program's own Markdown rendering, so a heading is a heading and a fence is highlighted code.
//! - `rola docs --search <TEXT>` searches the book. The answer is a table: the article's name on
//!   the left, the fragments the query matched on the right, with the matched words drawn in red
//!   and bold. A run that asked for `--json` is answered with the same entries as fields.
//!
//! The table is the program's own rather than the colour language's pipe table, for two reasons: a
//! fragment is styled run by run out of the ranges the search hands back, which a cell of text
//! cannot carry, and the columns are 3:7 rather than as wide as their content, so a long snippet
//! wraps where the reader expects instead of pushing the name out of the way.
//!
//! `--no-pager` is a global flag and is honoured here like anywhere else: the pager is the program's
//! post-render hook, and this command knows nothing about it.

use std::path::PathBuf;

use librorolala::book_search::{BookSearch, Hit, Snippet};
use mingling::{
    Grouped, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{Colorize as _, err_line, help_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::complete::{filling_flag, strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_DOCS, EC_ERR_DOCS_ARGUMENT, EC_HELP};
use crate::failure::failure;

/// Search the book rather than naming one article.
const ARG_SEARCH: PickerArg<'static, Flag> = arg![search: Flag];

/// How many search answers are shown at most, when the run names none.
const DEFAULT_LIMIT: usize = 12;

/// How many search answers are shown at most.
const ARG_LIMIT: PickerArg<'static, Option<usize>> = arg![limit: Option<usize>];

/// What separates the two columns of the search table.
const SEPARATOR: &str = " | ";

/// How many parts of the table the article's name takes.
const NAME_SHARE: usize = 3;

/// How many parts the table is divided into.
const TOTAL_SHARE: usize = 10;

/// How narrow the name column is allowed to become.
const MIN_NAME_WIDTH: usize = 12;

/// How narrow the snippet column is allowed to become.
const MIN_SNIPPET_WIDTH: usize = 24;

/// How wide the table is drawn when there is no terminal to ask.
const FALLBACK_COLUMNS: usize = 100;

/// How many columns a terminal is assumed to have when it says it has fewer.
const MIN_COLUMNS: usize = 40;

#[help(buffer)]
pub fn help_docs(_: EntryDocs, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("docs.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryDocs)]
pub fn desc_docs() -> Description {
    t!("docs.cmd_docs_description").to_string().into()
}

/// Completes what `rola docs` can be given next.
///
/// The text is the article's name or a query, which this cannot know without reading the book; a
/// flag, though, is knowable, and the limit takes a number that has no list to offer.
#[completion(EntryDocs)]
pub fn complete_docs(ctx: ShellContext) -> Suggest {
    if filling_flag(&ctx, &ARG_LIMIT) {
        return suggest!();
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_SEARCH: t!("docs.complete.search"),
                ARG_LIMIT: t!("docs.complete.limit"),
            },
        );
    }

    suggest!()
}

/// Read an article of the built-in book, or search it
///
/// `TEXT` names one article exactly, and the article is printed as it was written, through the
/// program's Markdown rendering. `--search` makes `TEXT` a query instead: every word of it has to
/// be in an article for it to be found, and what is drawn is a table of the names and the matched
/// fragments, the matched words in red and bold.
///
/// A query in Chinese is cut into characters and adjacent pairs, so a one-character query finds
/// its line and a longer one asks for its phrase. `--limit` caps how many answers are drawn; it is
/// only meaningful with `--search`.
///
/// A `--json` run is answered with the article's `title`, `description`, `keywords` and `content`
/// when one is named, and with the search `entries` when a query is.
///
/// # Errors
///
/// Renders [`ErrorDocsNoBook`] when no book is beside the program or above the directory it was run
/// in, [`ErrorDocsUnknownArticle`] when the name matches no article, [`ErrorDocsMissingText`] when
/// `--search` is written with no text, and [`ErrorDocsLimitWithoutSearch`] when `--limit` is
/// written without `--search`.
#[command(node = "docs", entry = EntryDocs)]
pub fn docs(args: EntryDocs) -> Next {
    let picked = args
        .pick(ARG_SEARCH)
        .pick(ARG_LIMIT)
        .pick(arg![Option<String>])
        .pick(&UNMATCHED)
        .to_result();
    let (search, limit, text, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "docs") {
        return next;
    }

    let searching = matches!(search, Flag::Active);

    let Some(text) = text else {
        if searching {
            return ErrorDocsMissingText.into();
        }

        return ResultDocsHelp.into();
    };

    if !searching {
        if limit.is_some() {
            return ErrorDocsLimitWithoutSearch.into();
        }

        return StateDocs::article(text).into();
    }

    StateDocs::search(text, limit.unwrap_or(DEFAULT_LIMIT)).into()
}

/// The state a `docs` run starts in: which of the two readings it is.
#[derive(Grouped)]
pub struct StateDocs {
    /// What was asked for.
    mode: Mode,
}

/// What `rola docs` was asked to do with its text.
enum Mode {
    /// The article named exactly.
    Article(String),
    /// The articles a query finds.
    Search {
        /// The words to find.
        query: String,
        /// How many answers to draw at most.
        limit: usize,
    },
}

impl StateDocs {
    /// A run that names one article.
    fn article(name: String) -> Self {
        Self {
            mode: Mode::Article(name),
        }
    }

    /// A run that searches the book.
    fn search(query: String, limit: usize) -> Self {
        Self {
            mode: Mode::Search { query, limit },
        }
    }
}

/// Reads the book, and answers with the article or the entries the run asked for.
#[chain(routeify)]
pub fn handle_docs(state: StateDocs) -> Next {
    let Some(book) = open_book() else {
        return ErrorDocsNoBook.into();
    };

    match state.mode {
        Mode::Article(name) => book.article(&name).map_or_else(
            || ErrorDocsUnknownArticle { name }.into(),
            |article| {
                ResultDocsArticle {
                    title: article.title().to_owned(),
                    description: article.description().to_owned(),
                    keywords: article.keywords().to_vec(),
                    content: article.markdown().to_owned(),
                }
                .into()
            },
        ),
        Mode::Search { query, limit } => ResultDocsSearch::of(&book.search(&query, limit)).into(),
    }
}

/// The book this run can read: beside the program, or above the directory it was run in.
///
/// Two places are looked in because two ways of running the program exist. A program that was
/// installed or exported has the book beside it, under the root its own directory sits in. A
/// developer's `cargo run` has neither, but a checkout has `build/docs/builtin.book` above it,
/// which is what makes `rola docs` usable from the tree it is developed in. The program's own
/// place is read first, so an installed book is not shadowed by a checkout the run happens to be
/// inside.
fn open_book() -> Option<BookSearch> {
    let mut starts: Vec<PathBuf> = Vec::new();

    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|program| program.parent().map(std::path::Path::to_path_buf))
    {
        starts.push(directory);
    }

    if let Ok(directory) = std::env::current_dir() {
        // A checkout lays the book under `build/`, and a run made from the checkout's root is
        // answered by the second of these; the first is what a run made inside an export or an
        // install reads.
        starts.push(directory.join("build"));
        starts.push(directory);
    }

    starts
        .into_iter()
        .find_map(|start| BookSearch::locate(&start))
}

/// Result: the run named an article, and it was read.
///
/// The fields are the ones a `--json` reader is given and nothing else: the name is what named the
/// article rather than a fact about it, and the plain text is the index's reading, not the
/// article's. What a person is shown is drawn from `content`, which is the article as written.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultDocsArticle {
    /// The article's title.
    title: String,
    /// The article's description.
    description: String,
    /// The article's keywords.
    keywords: Vec<String>,
    /// The article as it was written, Markdown and all.
    content: String,
}

#[renderer(buffer)]
pub fn render_result_docs_article(result: ResultDocsArticle) {
    r_println!("{}", trd!(result.content));
}

/// Result: what `rola docs` can do was named.
///
/// A run that named neither an article nor a query asked about the command rather than about the
/// book, so it is shown what the command is for.
#[derive(Grouped)]
pub struct ResultDocsHelp;

#[renderer(buffer)]
pub fn render_result_docs_help(_: ResultDocsHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("docs.help")).trim());
    ec.exit_code = EC_HELP;
}

/// Result: the run searched the book.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultDocsSearch {
    /// The articles the query found, best first.
    entries: Vec<ResultDocsEntry>,
}

impl ResultDocsSearch {
    /// The result the hits are drawn from.
    fn of(hits: &[Hit]) -> Self {
        Self {
            entries: hits.iter().map(ResultDocsEntry::of).collect(),
        }
    }
}

/// One article a `--search` found, as `--json` writes it.
#[derive(Serialize)]
pub struct ResultDocsEntry {
    /// The article's name.
    name: String,
    /// The article's title.
    title: String,
    /// The article's description.
    description: String,
    /// How well the query matched, relative to the other entries.
    score: f64,
    /// The fragments the query matched.
    snippets: Vec<ResultDocsSnippet>,
}

impl ResultDocsEntry {
    /// One hit, as the fields a reader is answered with.
    fn of(hit: &Hit) -> Self {
        Self {
            name: hit.name.clone(),
            title: hit.title.clone(),
            description: hit.description.clone(),
            score: hit.score,
            snippets: hit.snippets.iter().map(ResultDocsSnippet::of).collect(),
        }
    }
}

/// One fragment of a `--search` entry, as `--json` writes it.
#[derive(Serialize)]
pub struct ResultDocsSnippet {
    /// The field the fragment was cut from.
    field: String,
    /// The fragment's text, with `…` where it was cut.
    text: String,
    /// The matched words inside the text, in characters.
    ranges: Vec<ResultDocsRange>,
}

impl ResultDocsSnippet {
    /// One fragment, as the fields a reader is answered with.
    fn of(snippet: &Snippet) -> Self {
        Self {
            field: snippet.field.label().to_owned(),
            text: snippet.text.clone(),
            ranges: snippet
                .ranges
                .iter()
                .map(|range| ResultDocsRange {
                    start: range.start,
                    length: range.length(),
                })
                .collect(),
        }
    }
}

/// One matched range of a fragment, as `--json` writes it.
#[derive(Serialize)]
pub struct ResultDocsRange {
    /// The first character of the range.
    start: usize,
    /// How many characters it covers.
    length: usize,
}

#[renderer(buffer)]
pub fn render_result_docs_search(result: ResultDocsSearch) {
    if result.entries.is_empty() {
        r_println!("{}", t!("docs.no_match").trim());
    } else {
        let (name_width, snippet_width) = columns();
        let header = format!(
            "{}{}{}",
            pad_to(t!("docs.table_name").trim(), name_width),
            SEPARATOR,
            t!("docs.table_snippet").trim(),
        );
        r_println!("{}", header.bold().underline());

        for entry in &result.entries {
            let name = wrapped(&plain(&entry.name), name_width);
            let snippets = snippet_lines(entry, snippet_width);
            let height = name.len().max(snippets.len());

            for line in 0..height {
                let left = name.get(line).map_or("", String::as_str);
                let right = snippets.get(line).map_or("", String::as_str);

                r_println!("{}{}{}", pad_to(left, name_width), SEPARATOR, right);
            }
        }
    }
}

/// How wide the two columns are drawn, in characters.
///
/// The two columns divide what is left of the terminal after the separator 3:7, which is what puts
/// the name and the fragment at the widths the table is meant to have rather than at whatever the
/// content happens to be. A terminal too narrow for both minimums is drawn at the minimums anyway:
/// squeezing a column is worse than wrapping the terminal.
fn columns() -> (usize, usize) {
    let width = console::Term::stdout()
        .size_checked()
        .map_or(FALLBACK_COLUMNS, |(_, columns)| usize::from(columns))
        .max(MIN_COLUMNS);
    let usable = width.saturating_sub(SEPARATOR.len());

    let name = (usable * NAME_SHARE / TOTAL_SHARE).max(MIN_NAME_WIDTH);
    let snippet = usable.saturating_sub(name).max(MIN_SNIPPET_WIDTH);

    (name, snippet)
}

/// `text` followed by as many spaces as `width` needs to hold it.
fn pad_to(text: &str, width: usize) -> String {
    let used = console::measure_text_width(text);
    let mut padded = String::with_capacity(text.len() + width.saturating_sub(used));

    padded.push_str(text);
    padded.extend(std::iter::repeat_n(' ', width.saturating_sub(used)));

    padded
}

/// The fragments of an entry, wrapped to the width of the column.
fn snippet_lines(entry: &ResultDocsEntry, width: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for snippet in &entry.snippets {
        lines.extend(wrapped(&emphasised(snippet), width));
    }

    lines
}

/// The text of `snippet` with the characters a range covers marked as matched.
fn emphasised(snippet: &ResultDocsSnippet) -> Vec<(char, bool)> {
    let characters: Vec<char> = snippet.text.chars().collect();
    let mut matched = vec![false; characters.len()];

    for range in &snippet.ranges {
        let start = range.start.min(characters.len());
        let end = range
            .start
            .saturating_add(range.length)
            .min(characters.len());

        if let Some(window) = matched.get_mut(start..end) {
            window.fill(true);
        }
    }

    characters.into_iter().zip(matched).collect()
}

/// `text` as ordinary characters, which is what a name is drawn as.
fn plain(text: &str) -> Vec<(char, bool)> {
    text.chars().map(|character| (character, false)).collect()
}

/// `characters` wrapped into lines of at most `width` characters on screen.
///
/// Words are kept together where they fit, and a word too long for a line is cut at the width: a
/// fragment has no obligation to a word's spelling, and a line that overflows its column would
/// push the table out of shape. The widening characters of Chinese text are counted as two, so a
/// column of them is as wide as the terminal says rather than twice as wide.
#[must_use]
fn wrapped(characters: &[(char, bool)], width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line: Vec<(char, bool)> = Vec::new();
    let mut used = 0;

    for word in words(characters) {
        let word_width: usize = word
            .iter()
            .map(|(character, _)| display_width(*character))
            .sum();

        if word_width <= width {
            let space = usize::from(!line.is_empty());

            if used + space + word_width > width && !line.is_empty() {
                lines.push(drawn_line(&line));
                line = Vec::new();
                used = 0;
            }

            if !line.is_empty() {
                line.push((' ', false));
                used += 1;
            }

            line.extend(word);
            used += word_width;
            continue;
        }

        // A word wider than a line is cut into pieces; what is left of the line is filled first.
        for (character, matched) in word {
            let character_width = display_width(character);

            if used + character_width > width && !line.is_empty() {
                lines.push(drawn_line(&line));
                line = Vec::new();
                used = 0;
            }

            line.push((character, matched));
            used += character_width;
        }
    }

    if !line.is_empty() {
        lines.push(drawn_line(&line));
    }

    lines
}

/// The words of `characters`: runs of non-space characters, each with its own marks.
fn words(characters: &[(char, bool)]) -> Vec<Vec<(char, bool)>> {
    let mut words = Vec::new();
    let mut word = Vec::new();

    for (character, matched) in characters {
        if character.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }

        word.push((*character, *matched));
    }

    if !word.is_empty() {
        words.push(word);
    }

    words
}

/// One line of a fragment, with the matched runs drawn in red and bold.
fn drawn_line(characters: &[(char, bool)]) -> String {
    let mut drawn = String::new();
    let mut at = 0;

    while at < characters.len() {
        let matched = characters[at].1;
        let start = at;

        while at < characters.len() && characters[at].1 == matched {
            at += 1;
        }

        let run: String = characters[start..at]
            .iter()
            .map(|(character, _)| *character)
            .collect();

        if matched {
            drawn.push_str(&run.red().bold().to_string());
        } else {
            drawn.push_str(&run);
        }
    }

    drawn
}

/// How many terminal columns a character takes.
fn display_width(character: char) -> usize {
    console::measure_text_width(&character.to_string())
}

/// Error: no book could be found for this run to read.
#[derive(Grouped)]
pub struct ErrorDocsNoBook;

impl Failure for ErrorDocsNoBook {
    fn name(&self) -> &'static str {
        "error_docs_no_book"
    }

    fn reason(&self) -> String {
        t!("docs.err_no_book").trim().to_string()
    }
}

failure!(ErrorDocsNoBook);

#[renderer(buffer)]
pub fn render_error_docs_no_book(error: ErrorDocsNoBook, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("docs.err_no_book_help").trim()));
    ec.exit_code = EC_ERR_DOCS;
}

/// Error: the name given matches no article.
#[derive(Grouped)]
pub struct ErrorDocsUnknownArticle {
    /// The name that was given.
    name: String,
}

impl Failure for ErrorDocsUnknownArticle {
    fn name(&self) -> &'static str {
        "error_docs_unknown_article"
    }

    fn reason(&self) -> String {
        t!("docs.err_unknown_article", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorDocsUnknownArticle);

#[renderer(buffer)]
pub fn render_error_docs_unknown_article(error: ErrorDocsUnknownArticle, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("docs.err_unknown_article_help").trim()));
    ec.exit_code = EC_ERR_DOCS;
}

/// Error: `--search` was written with no text.
#[derive(Grouped)]
pub struct ErrorDocsMissingText;

impl Failure for ErrorDocsMissingText {
    fn name(&self) -> &'static str {
        "error_docs_missing_text"
    }

    fn reason(&self) -> String {
        t!("docs.err_missing_text").trim().to_string()
    }
}

failure!(ErrorDocsMissingText);

#[renderer(buffer)]
pub fn render_error_docs_missing_text(error: ErrorDocsMissingText, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("docs.err_missing_text_help").trim()));
    ec.exit_code = EC_ERR_DOCS_ARGUMENT;
}

/// Error: `--limit` was written without `--search`.
#[derive(Grouped)]
pub struct ErrorDocsLimitWithoutSearch;

impl Failure for ErrorDocsLimitWithoutSearch {
    fn name(&self) -> &'static str {
        "error_docs_limit_without_search"
    }

    fn reason(&self) -> String {
        t!("docs.err_limit_without_search").trim().to_string()
    }
}

failure!(ErrorDocsLimitWithoutSearch);

#[renderer(buffer)]
pub fn render_error_docs_limit_without_search(
    error: ErrorDocsLimitWithoutSearch,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("docs.err_limit_without_search_help").trim())
    );
    ec.exit_code = EC_ERR_DOCS_ARGUMENT;
}

#[cfg(test)]
mod tests {
    use super::{ResultDocsRange, ResultDocsSnippet, drawn_line, emphasised, wrapped};

    fn text(pieces: &[(&str, bool)]) -> Vec<(char, bool)> {
        pieces
            .iter()
            .flat_map(|(piece, matched)| {
                piece
                    .chars()
                    .map(|character| (character, *matched))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn a_word_the_query_matched_is_marked() {
        let snippet = ResultDocsSnippet {
            field: "content".to_owned(),
            text: "the needle is here".to_owned(),
            ranges: vec![ResultDocsRange {
                start: 4,
                length: 6,
            }],
        };

        let marked = emphasised(&snippet);
        let matched: String = marked
            .iter()
            .filter(|(_, matched)| *matched)
            .map(|(character, _)| *character)
            .collect();

        assert_eq!(matched, "needle");
    }

    #[test]
    fn a_marked_run_is_drawn_and_the_rest_is_not_changed() {
        let drawn = drawn_line(&text(&[("the ", false), ("needle", true)]));

        assert_eq!(drawn, "the needle");
    }

    #[test]
    fn wrapping_keeps_words_whole_where_they_fit() {
        let lines = wrapped(&text(&[("one two three", false)]), 7);

        assert_eq!(lines, ["one two", "three"]);
    }

    #[test]
    fn a_word_wider_than_the_column_is_cut() {
        let lines = wrapped(&text(&[("abcdef", false)]), 4);

        assert_eq!(lines, ["abcd", "ef"]);
    }

    #[test]
    fn wrapping_counts_a_chinese_character_as_two_columns() {
        let lines = wrapped(&text(&[("安装说明", false)]), 5);

        assert_eq!(lines, ["安装", "说明"]);
    }
}

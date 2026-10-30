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
//! The table is drawn by this workspace's reusable table component: the article's name is as wide
//! as the names that matched, the fragments take the rest of the terminal, and a matched word is a
//! run drawn red and bold. A run that asked for `--json` is answered with the same entries as
//! fields.
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
use rorolala_cli_setups::ResNoPager;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{Cell, Column, Run, RunStyle, Table, err_line, help_line, trd};
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

/// Print the article as it was written rather than as rendered Markdown.
const ARG_RAW: PickerArg<'static, Flag> = arg![raw: Flag];

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
pub fn docs(args: EntryDocs, no_pager: &mut ResNoPager) -> Next {
    let picked = args
        .pick(ARG_SEARCH)
        .pick(ARG_LIMIT)
        .pick(ARG_RAW)
        .pick(arg![Option<String>])
        .pick(&UNMATCHED)
        .to_result();
    let (search, limit, raw, text, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "docs") {
        return next;
    }

    let searching = matches!(search, Flag::Active);
    let raw = matches!(raw, Flag::Active);

    // `--raw` writes the article as it was written, so there is nothing for a pager to add: the
    // output is the text itself, not a rendering of it. Settling the resource here covers every
    // answer the run can end with, a refusal or the help included.
    if raw {
        **no_pager = true;
    }

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

        return StateDocs::article(text, raw).into();
    }

    StateDocs::search(text, limit.unwrap_or(DEFAULT_LIMIT), raw).into()
}

/// The state a `docs` run starts in: which of the two readings it is.
#[derive(Grouped)]
pub struct StateDocs {
    /// What was asked for.
    mode: Mode,
    /// Whether the article is to be written as it was written rather than as rendered Markdown.
    raw: bool,
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
    fn article(name: String, raw: bool) -> Self {
        Self {
            mode: Mode::Article(name),
            raw,
        }
    }

    /// A run that searches the book.
    fn search(query: String, limit: usize, raw: bool) -> Self {
        Self {
            mode: Mode::Search { query, limit },
            raw,
        }
    }
}

/// Reads the book, and answers with the article or the entries the run asked for.
#[chain(routeify)]
pub fn handle_docs(state: StateDocs) -> Next {
    let Some(book) = open_book() else {
        return ErrorDocsNoBook.into();
    };

    let raw = state.raw;

    match state.mode {
        Mode::Article(name) => book.article(&name).map_or_else(
            || ErrorDocsUnknownArticle { name }.into(),
            |article| {
                ResultDocsArticle {
                    title: article.title().to_owned(),
                    description: article.description().to_owned(),
                    keywords: article.keywords().to_vec(),
                    content: article.markdown().to_owned(),
                    raw,
                }
                .into()
            },
        ),
        Mode::Search { query, limit } => {
            let hits = book.search(&query, limit);

            ResultDocsSearch::of(&book, &hits, raw).into()
        }
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
    /// Whether the article is to be written as it was written rather than as rendered Markdown.
    ///
    /// It is a fact about the run rather than about the article, so `--json` leaves it out: the
    /// `content` it writes is already the article's own text.
    #[serde(skip)]
    raw: bool,
}

#[renderer(buffer)]
pub fn render_result_docs_article(result: ResultDocsArticle) {
    if result.raw {
        r_println!("{}", result.content);
    } else {
        r_println!("{}", trd!(result.content));
    }
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
    /// Whether a one-article answer is to be written as it was written rather than as rendered
    /// Markdown.
    ///
    /// It is a fact about the run rather than about the search, so `--json` leaves it out.
    #[serde(skip)]
    raw: bool,
}

impl ResultDocsSearch {
    /// The result the hits are drawn from.
    fn of(book: &BookSearch, hits: &[Hit], raw: bool) -> Self {
        Self {
            entries: hits
                .iter()
                .map(|hit| ResultDocsEntry::of(book, hit))
                .collect(),
            raw,
        }
    }

    /// The one article the answer is, when the run is to be given the article itself.
    ///
    /// A search that found one article, and a search the run limited to one, are the same answer:
    /// one article. The table is for the case there is a choice to make between several.
    fn only(&self) -> Option<&ResultDocsEntry> {
        if self.entries.len() == 1 {
            self.entries.first()
        } else {
            None
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
    /// The article as it was written, Markdown and all, for the run that is given the article
    /// itself rather than the table.
    ///
    /// The search already answered with the article's name, so this is what the book can look up
    /// rather than another thing the query found; `--json` leaves it out because it is not one of
    /// the entry's fields.
    #[serde(skip)]
    content: String,
}

impl ResultDocsEntry {
    /// One hit, as the fields a reader is answered with.
    fn of(book: &BookSearch, hit: &Hit) -> Self {
        Self {
            name: hit.name.clone(),
            title: hit.title.clone(),
            description: hit.description.clone(),
            score: hit.score,
            snippets: hit.snippets.iter().map(ResultDocsSnippet::of).collect(),
            content: book
                .article(&hit.name)
                .map_or_else(String::new, |article| article.markdown().to_owned()),
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
    } else if let Some(entry) = result.only() {
        if result.raw {
            r_println!("{}", entry.content);
        } else {
            r_println!("{}", trd!(entry.content));
        }
    } else {
        let mut table = Table::new([
            Column::auto(t!("docs.table_name").to_string()),
            Column::fill(t!("docs.table_snippet").to_string()),
        ]);

        for entry in &result.entries {
            table.row([Cell::text(entry.name.clone()), snippet_cell(entry)]);
        }

        r_println!("{}", table.render());
    }
}

/// The matched fragments of `entry` as one cell: every fragment on its own line, the ranges the
/// search marked drawn red and bold.
fn snippet_cell(entry: &ResultDocsEntry) -> Cell {
    let mut runs = Vec::new();

    for (at, snippet) in entry.snippets.iter().enumerate() {
        if at > 0 {
            runs.push(Run::new("\n", RunStyle::Plain));
        }

        let mut from = 0;
        for range in &snippet.ranges {
            if range.start > from {
                runs.push(Run::new(
                    slice(&snippet.text, from, range.start),
                    RunStyle::Plain,
                ));
            }

            let end = range.start.saturating_add(range.length);
            runs.push(Run::new(
                slice(&snippet.text, range.start, end),
                RunStyle::Highlight,
            ));
            from = end;
        }

        if from < snippet.text.chars().count() {
            runs.push(Run::new(
                slice(&snippet.text, from, usize::MAX),
                RunStyle::Plain,
            ));
        }
    }

    if runs.is_empty() {
        return Cell::text(String::new());
    }

    Cell::runs(runs)
}

/// The characters of `text` from `start` up to `end`, counted in characters.
fn slice(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
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
    use super::slice;

    #[test]
    fn a_range_is_cut_on_characters_rather_than_bytes() {
        assert_eq!(slice("安装说明", 1, 3), "装说");
        assert_eq!(slice("install", 3, 8), "tall");
    }
}

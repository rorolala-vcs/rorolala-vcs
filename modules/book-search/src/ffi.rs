//! The C ABI of a searchable book.
//!
//! A book crosses as an owned handle, a search as an owned list of hits, and a hit as the facts a
//! list draws it with: a name, a title, a description, a score, and the fragments the query
//! matched. Every handle is released with the `free_*` named after its type, and every string read
//! out of one is released with `free_string`. Nothing is cloned across the boundary that the
//! caller could have taken from the book itself: a hit is a value of its own, because the list it
//! came from may be released before the hit is drawn.

use std::path::Path;

use rorolala_utils_lazyffi::lazyffi;

use crate::{BookError, BookSearch, Hit, Snippet};

/// Opens the book at `path`.
///
/// # Errors
///
/// Returns [`BookError`] when the file cannot be read, when it is not a book this build knows, or
/// when it is a format version this build does not read.
///
/// # FFI
///
/// Answers with a `RolaBookSearch` the caller releases with `free_rola_book_search`, or with a
/// `RolaBookError` released with `free_rola_book_error`.
#[lazyffi(export = rola_book_search_open)]
pub fn open(path: &Path) -> Result<BookSearch, BookError> {
    BookSearch::open(path)
}

/// The book laid beside the programs under `start`, if there is one.
///
/// # FFI
///
/// Answers with a `RolaBookSearch` the caller releases with `free_rola_book_search`, or with null
/// when there is no book to open.
#[must_use]
#[lazyffi(export = rola_book_search_locate)]
pub fn locate(start: &Path) -> Option<BookSearch> {
    BookSearch::locate(start)
}

#[lazyffi(export = rola_book_)]
impl BookSearch {
    /// The articles `query` finds, best first, at most `limit` of them.
    ///
    /// # FFI
    ///
    /// Answers with a `RolaBookResults` the caller releases with `free_rola_book_results`. Nothing
    /// matched is an empty list rather than a failure.
    #[must_use]
    #[lazyffi(export = rola_book_search_run)]
    pub fn search_for_ffi(&self, query: &str, limit: usize) -> BookResults {
        BookResults(self.search(query, limit))
    }

    /// How many articles the book holds.
    #[must_use]
    #[lazyffi(export = rola_book_articles_count)]
    pub const fn count_for_ffi(&self) -> usize {
        self.count()
    }
}

/// The articles one search found, best first.
#[lazyffi(export = RolaBookResults)]
#[derive(Debug)]
pub struct BookResults(Vec<Hit>);

#[lazyffi(export = rola_book_results_)]
impl BookResults {
    /// How many articles the search found.
    #[must_use]
    #[lazyffi(export = rola_book_results_count)]
    pub const fn count(&self) -> usize {
        self.0.len()
    }

    /// The hit at `index`, if the search found that many.
    ///
    /// # FFI
    ///
    /// Answers with a `RolaBookHit` the caller releases with `free_rola_book_hit`, or with null
    /// past the end.
    #[must_use]
    #[lazyffi(export = rola_book_results_at)]
    pub fn hit(&self, index: usize) -> Option<BookHit> {
        self.0.get(index).cloned().map(BookHit)
    }
}

/// One article a search found, as a list of hits draws it.
#[lazyffi(export = RolaBookHit)]
#[derive(Debug, Clone)]
pub struct BookHit(Hit);

#[lazyffi(export = rola_book_hit_)]
impl BookHit {
    /// The article's name, which finds it again through the book.
    ///
    /// # FFI
    ///
    /// Answers with a string the caller releases with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_book_hit_name)]
    pub fn name(&self) -> String {
        self.0.name.clone()
    }

    /// The article's title.
    ///
    /// # FFI
    ///
    /// Answers with a string the caller releases with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_book_hit_title)]
    pub fn title(&self) -> String {
        self.0.title.clone()
    }

    /// The article's description.
    ///
    /// # FFI
    ///
    /// Answers with a string the caller releases with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_book_hit_description)]
    pub fn description(&self) -> String {
        self.0.description.clone()
    }

    /// How well the query matched the article, relative to the other hits of the same search.
    #[must_use]
    #[lazyffi(export = rola_book_hit_score)]
    pub const fn score(&self) -> f64 {
        self.0.score
    }

    /// How many fragments the query matched in the article.
    #[must_use]
    #[lazyffi(export = rola_book_hit_snippet_count)]
    pub const fn snippet_count(&self) -> usize {
        self.0.snippets.len()
    }

    /// The fragment at `index`, if the query matched that many.
    ///
    /// # FFI
    ///
    /// Answers with a `RolaBookSnippet` the caller releases with `free_rola_book_snippet`, or with
    /// null past the end.
    #[must_use]
    #[lazyffi(export = rola_book_hit_snippet_at)]
    pub fn snippet(&self, index: usize) -> Option<BookSnippet> {
        self.0.snippets.get(index).cloned().map(BookSnippet)
    }
}

/// One fragment of an article, and the words in it the query matched.
#[lazyffi(export = RolaBookSnippet)]
#[derive(Debug, Clone)]
pub struct BookSnippet(Snippet);

#[lazyffi(export = rola_book_snippet_)]
impl BookSnippet {
    /// The field the fragment was cut from: `content`, `description`, `keywords`, `title` or
    /// `name`.
    ///
    /// # FFI
    ///
    /// Answers with a string the caller releases with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_book_snippet_field)]
    pub fn field(&self) -> String {
        self.0.field.label().to_owned()
    }

    /// The fragment's text, with `...` where it was cut.
    ///
    /// # FFI
    ///
    /// Answers with a string the caller releases with `free_string`.
    #[must_use]
    #[lazyffi(export = rola_book_snippet_text)]
    pub fn text(&self) -> String {
        self.0.text.clone()
    }

    /// How many matched ranges the fragment carries.
    #[must_use]
    #[lazyffi(export = rola_book_snippet_range_count)]
    pub const fn range_count(&self) -> usize {
        self.0.ranges.len()
    }

    /// Where the range at `index` starts, counted in characters of the fragment.
    ///
    /// # FFI
    ///
    /// Answers zero past the end; a caller that asks a range at all asks within
    /// [`Self::range_count`].
    #[must_use]
    #[lazyffi(export = rola_book_snippet_range_start)]
    pub fn range_start(&self, index: usize) -> usize {
        self.0.ranges.get(index).map_or(0, |range| range.start)
    }

    /// How many characters the range at `index` covers.
    ///
    /// # FFI
    ///
    /// Answers zero past the end, as [`Self::range_start`] does.
    #[must_use]
    #[lazyffi(export = rola_book_snippet_range_length)]
    pub fn range_length(&self, index: usize) -> usize {
        self.0.ranges.get(index).map_or(0, crate::Range::length)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Field, Hit, Range, Snippet};

    use super::{BookHit, BookResults, BookSnippet, locate, open};

    fn snippet() -> Snippet {
        Snippet {
            field: Field::Content,
            text: "the text".to_owned(),
            ranges: vec![Range { start: 4, end: 8 }],
        }
    }

    fn hit() -> Hit {
        Hit {
            name: "a".to_owned(),
            title: "title".to_owned(),
            description: "description".to_owned(),
            score: 1.5,
            snippets: vec![snippet()],
        }
    }

    #[test]
    fn a_result_list_answers_its_count_and_its_hits() {
        let results = BookResults(vec![hit()]);

        assert_eq!(results.count(), 1);
        assert!(results.hit(0).is_some());
        assert!(results.hit(1).is_none());
    }

    #[test]
    fn a_hit_answers_what_a_list_draws() {
        let hit = BookHit(hit());

        assert_eq!(hit.name(), "a");
        assert_eq!(hit.title(), "title");
        assert_eq!(hit.description(), "description");
        assert_eq!(hit.score(), 1.5);
        assert_eq!(hit.snippet_count(), 1);
        assert!(hit.snippet(0).is_some());
        assert!(hit.snippet(1).is_none());
    }

    #[test]
    fn a_snippet_answers_its_field_text_and_ranges() {
        let snippet = BookSnippet(snippet());

        assert_eq!(snippet.field(), "content");
        assert_eq!(snippet.text(), "the text");
        assert_eq!(snippet.range_count(), 1);
        assert_eq!(snippet.range_start(0), 4);
        assert_eq!(snippet.range_length(0), 4);
        assert_eq!(snippet.range_start(1), 0);
        assert_eq!(snippet.range_length(1), 0);
    }

    #[test]
    fn a_book_that_is_not_there_cannot_be_opened_or_located() {
        let path = std::env::temp_dir().join("rola-book-that-is-not-there.book");

        assert!(open(&path).is_err());
        assert!(locate(&path).is_none());
    }
}

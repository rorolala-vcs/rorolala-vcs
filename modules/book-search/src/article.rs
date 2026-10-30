//! Articles, the fields they are searched by, and the answers a search gives back.

use std::borrow::Cow;

/// One field of an article that a query is matched against.
///
/// The fields are kept apart for two reasons: a hit in the title is worth more than a hit in the
/// body, and a fragment is read from the field it was found in so that what is shown beside a hit
/// is the line the word is on rather than the whole article.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// The article's name: its path under `book/`, without the extension.
    Name,
    /// The article's title, from its front matter.
    Title,
    /// The article's description, from its front matter.
    Description,
    /// The article's keywords, from its front matter.
    Keywords,
    /// The article's text, with the Markdown syntax taken away.
    Content,
}

impl Field {
    /// Every field, in the order a book's index keeps the postings for them.
    pub const ALL: [Self; 5] = [
        Self::Name,
        Self::Title,
        Self::Description,
        Self::Keywords,
        Self::Content,
    ];

    /// The fields a fragment is read from, the one that reads best first.
    ///
    /// A query that names only a title has no content to cut a fragment from, so the fragment is
    /// taken from the field the word was actually in. Content comes first because a fragment of
    /// body text is what a hit is usually shown with.
    pub(crate) const SNIPPET_ORDER: [Self; 5] = [
        Self::Content,
        Self::Description,
        Self::Keywords,
        Self::Title,
        Self::Name,
    ];

    /// The position of the field in [`Self::ALL`], which is the order the index is written in.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The field's name, when it is written out for a reader or a caller.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Title => "title",
            Self::Description => "description",
            Self::Keywords => "keywords",
            Self::Content => "content",
        }
    }

    /// How much a hit in this field is worth against a hit in the body.
    ///
    /// The weights are relative to content, which is 1: a word in a title says more about what an
    /// article is than the same word somewhere in its body, and the name and keywords say almost as
    /// much. The numbers are not tuned against a corpus; they order the fields the way a reader
    /// looking for an article would.
    #[must_use]
    pub(crate) const fn weight(self) -> f64 {
        match self {
            Self::Name | Self::Description => 2.5,
            Self::Title => 4.0,
            Self::Keywords => 3.0,
            Self::Content => 1.0,
        }
    }

    /// The field's text from the parts of an article.
    pub(crate) fn text<'a>(
        self,
        name: &'a str,
        title: &'a str,
        description: &'a str,
        keywords: &'a [String],
        plain: &'a str,
    ) -> Cow<'a, str> {
        match self {
            Self::Name => Cow::Borrowed(name),
            Self::Title => Cow::Borrowed(title),
            Self::Description => Cow::Borrowed(description),
            Self::Keywords => Cow::Owned(keywords.join(" ")),
            Self::Content => Cow::Borrowed(plain),
        }
    }
}

/// An article as a build tool hands it over to be written into a book.
///
/// This is the input side of the crate: the dev tool reads a Markdown file, takes its front matter
/// apart and turns the body into plain text, and hands the result here. What a reader gets back is
/// [`Article`], which is the same facts in the shape the search reads them in.
#[derive(Debug, Clone, Default)]
pub struct SourceArticle {
    /// The article's name: its path under `book/`, with `/` between the directories and the `.md`
    /// extension taken off.
    pub name: String,
    /// The title a hit is shown by.
    pub title: String,
    /// The description a hit is shown with.
    pub description: String,
    /// The keywords the article is found by.
    pub keywords: Vec<String>,
    /// The article as it was written, Markdown and all.
    pub markdown: String,
    /// The article as search reads it: Markdown syntax taken away, whitespace collapsed.
    pub plain: String,
}

/// One article of a book that has been read.
///
/// The fields are the article's own facts; the token counts and averages the ranking needs belong
/// to [`BookSearch`](crate::BookSearch), which is where they were worked out.
#[derive(Debug, Clone)]
pub struct Article {
    /// The article's name.
    name: String,
    /// The article's title.
    title: String,
    /// The article's description.
    description: String,
    /// The article's keywords.
    keywords: Vec<String>,
    /// The article as it was written.
    markdown: String,
    /// The article as search reads it.
    plain: String,
}

impl Article {
    /// Makes an article from the facts a book file was read as.
    pub(crate) const fn new(
        name: String,
        title: String,
        description: String,
        keywords: Vec<String>,
        markdown: String,
        plain: String,
    ) -> Self {
        Self {
            name,
            title,
            description,
            keywords,
            markdown,
            plain,
        }
    }

    /// The article's name: its path under `book/`, without the extension.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The title a hit is shown by.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The description a hit is shown with.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The keywords the article is found by.
    #[must_use]
    pub fn keywords(&self) -> &[String] {
        &self.keywords
    }

    /// The article as it was written, Markdown and all.
    #[must_use]
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// The article as search reads it: Markdown syntax taken away, whitespace collapsed.
    #[must_use]
    pub fn plain(&self) -> &str {
        &self.plain
    }

    /// The field's text, as a fragment is cut from it.
    pub(crate) fn text(&self, field: Field) -> Cow<'_, str> {
        field.text(
            &self.name,
            &self.title,
            &self.description,
            &self.keywords,
            &self.plain,
        )
    }
}

/// One range of characters in a piece of text.
///
/// Both numbers count characters rather than bytes, so a range can be used to split a string into
/// runs whatever encoding the caller holds it in. The start is included and the end is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Range {
    /// The first character the range covers.
    pub start: usize,
    /// One past the last character the range covers.
    pub end: usize,
}

impl Range {
    /// How many characters the range covers.
    #[must_use]
    pub const fn length(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// A fragment of one field, and the words in it the query matched.
///
/// The text is cut from the field around the match, with `…` where it was cut, and the ranges are
/// the places inside that text to draw as matched. A range counts characters of the text that is
/// returned, so a caller splits the text at the ranges and styles the pieces it was told to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// The field the fragment was cut from.
    pub field: Field,
    /// The fragment itself.
    pub text: String,
    /// The matched words inside [`Self::text`], in character positions.
    pub ranges: Vec<Range>,
}

/// One article a query found, with what a list of hits draws beside it.
#[derive(Debug, Clone)]
pub struct Hit {
    /// The article's name, which names it to [`BookSearch::article`](crate::BookSearch::article).
    pub name: String,
    /// The article's title.
    pub title: String,
    /// The article's description.
    pub description: String,
    /// How well the query matched the article, relative to the other hits of the same search.
    pub score: f64,
    /// The fragments the query matched in the article, at most
    /// [`MAX_SNIPPETS_PER_HIT`](crate::MAX_SNIPPETS_PER_HIT).
    pub snippets: Vec<Snippet>,
}

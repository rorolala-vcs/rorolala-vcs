//! What reading the book's articles fails with.

use std::fmt;
use std::path::PathBuf;

use rorolala_book_search::BookError;

/// What a build of the book failed with.
#[derive(Debug)]
pub enum Error {
    /// A directory or a file could not be read, or the book could not be written.
    Io {
        /// What was being done, for a reader who has to know where it stopped.
        doing: &'static str,
        /// What the operating system answered.
        detail: String,
    },
    /// An article's file is not UTF-8.
    Utf8 {
        /// The article's file.
        path: PathBuf,
        /// What the decoder answered.
        detail: String,
    },
    /// An article's front matter is not the YAML it is read as.
    FrontMatter {
        /// The article's file.
        path: PathBuf,
        /// What the YAML parser answered.
        detail: String,
    },
    /// The book directory is not there, or holds no articles.
    NoArticles {
        /// The directory that was walked.
        path: PathBuf,
    },
    /// The book format refused what the articles were written as.
    Book(BookError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { doing, detail } => write!(formatter, "{doing}: {detail}"),
            Self::Utf8 { path, detail } => {
                write!(
                    formatter,
                    "{}: the article is not UTF-8: {detail}",
                    path.display()
                )
            }
            Self::FrontMatter { path, detail } => {
                write!(
                    formatter,
                    "{}: the front matter is not YAML: {detail}",
                    path.display()
                )
            }
            Self::NoArticles { path } => write!(
                formatter,
                "{}: no articles under here: the book is the `*.md` files under `{}`",
                path.display(),
                crate::BOOK_DIR
            ),
            Self::Book(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<BookError> for Error {
    fn from(source: BookError) -> Self {
        Self::Book(source)
    }
}

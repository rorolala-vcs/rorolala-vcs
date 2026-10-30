#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod article;
mod error;
mod ffi;
pub(crate) mod format;
mod index;
pub(crate) mod snippet;
mod token;

pub use article::{Article, Field, Hit, Range, Snippet, SourceArticle};
pub use error::BookError;
pub use ffi::*;
pub use format::{FORMAT_VERSION, MAGIC, encode};
pub use index::BookSearch;
pub use snippet::MAX_SNIPPETS_PER_HIT;
pub use token::Term;

/// The file the book is read from, where it sits in a build or an install.
///
/// It is one file for the whole book: the articles, the text each of them is searched by, and the
/// index the search walks. The path is relative to the directory the documentation is laid in,
/// which is the parent of the directory the programs are laid in — see [`BookSearch::locate`].
pub const BOOK_FILE_NAME: &str = "builtin.book";

/// The path the book is read from, relative to the root a program is installed under.
///
/// A program at `bin/rola` is answered by walking to `docs/builtin.book`; the Desktop program at
/// `bin/desktop/RorolalaDesktop.exe` by walking one more directory up. Both are the same walk,
/// which is why the file is placed beside `bin/` rather than beside either program.
pub const BOOK_RELATIVE_PATH: &str = "docs/builtin.book";

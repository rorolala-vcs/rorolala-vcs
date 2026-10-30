//! `book-index`: the built-in book, read from `book/` and written as one searchable file.
//!
//! The articles are Markdown files with a YAML front matter block. This program walks them, takes
//! the title, description and keywords out of the block, turns the body into the plain text the
//! search reads, and writes the lot — articles and index — as one file with
//! [`rorolala_book_search`]'s format. The file is what a program opens; nothing here is read at
//! run time.
//!
//! ```sh
//! cargo dev book
//! ```
//!
//! What it writes is `build/docs/builtin.book`, which is where `cargo dev export` lays the file a
//! package carries, and where the command line program and the Desktop program find it beside
//! themselves. The two paths are stated once in this crate — [`BOOK_DIR`] and [`BOOK_IN_BUILD`] —
//! and [`workspace_paths`] turns them into the ones a build uses, so the tool and the export cannot
//! disagree about where the file goes.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod article;
mod error;
mod markdown;

use std::path::{Path, PathBuf};

pub use error::Error;

/// The directory the articles are read from, relative to the workspace root.
pub const BOOK_DIR: &str = "book";

/// The directory an export is laid out in when the build names no other one.
///
/// The variable `BUILD_DIR` overrides it, the way it overrides every other build step's output
/// directory, so the file lands in the export that was laid out rather than beside a name this
/// tool chose.
pub const BUILD_DIR: &str = "build";

/// The file the book is written to, relative to the export's own directory.
///
/// It is laid beside the programs rather than inside either of them: `bin/` and `docs/` are
/// siblings, so a program two directories under the root finds it by walking up.
pub const BOOK_IN_BUILD: &str = "docs/builtin.book";

/// What one build of the book was made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The file written.
    pub output: PathBuf,
    /// How many articles were written into it.
    pub articles: usize,
    /// How large the file is.
    pub bytes: u64,
}

/// Builds the book under `book` into `output`.
///
/// The articles are the `*.md` files under `book`, walked recursively, with a file or directory
/// whose name begins with `__` left out — that is where this tree keeps a draft. An article's name
/// is its path under `book` with `/` between the directories and the `.md` extension taken off, so
/// `getting-started/install.md` is the article `getting-started/install`.
///
/// # Errors
///
/// Reports [`Error`] when the directory cannot be walked, a file is not UTF-8, a front matter block
/// is not YAML, or there are no articles to write.
pub fn build(book: &Path, output: &Path) -> Result<Report, Error> {
    let sources = article::read(book)?;
    let bytes = rorolala_book_search::encode(&sources).map_err(Error::Book)?;

    if let Some(directory) = output.parent() {
        std::fs::create_dir_all(directory).map_err(|error| Error::Io {
            doing: "creating the book's directory",
            detail: format!("{}: {error}", directory.display()),
        })?;
    }

    write(output, &bytes)?;

    Ok(Report {
        output: output.to_owned(),
        articles: sources.len(),
        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

/// Writes `bytes` whole, with the file before a cut-short write left as it was.
///
/// A book is one file, so a write that stopped halfway would leave a file that opens as garbage;
/// writing beside it and moving the finished one into place is what makes the file that is there
/// always a file that was written whole.
fn write(output: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temporary = output.with_extension("book.tmp");

    std::fs::write(&temporary, bytes).map_err(|error| Error::Io {
        doing: "writing the book",
        detail: format!("{}: {error}", temporary.display()),
    })?;

    std::fs::remove_file(output).ok();
    std::fs::rename(&temporary, output).map_err(|error| Error::Io {
        doing: "moving the book into place",
        detail: format!("{}: {error}", output.display()),
    })
}

/// The book directory and the output file a workspace's own build uses.
///
/// The output follows `BUILD_DIR`, the variable every build step reads its output directory from,
/// so the file is written into the export that was laid out rather than beside one this tool named.
#[must_use]
pub fn workspace_paths(root: &Path) -> (PathBuf, PathBuf) {
    let build =
        std::env::var_os("BUILD_DIR").map_or_else(|| PathBuf::from(BUILD_DIR), PathBuf::from);

    (root.join(BOOK_DIR), root.join(build).join(BOOK_IN_BUILD))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rorolala_book_search::BookSearch;

    #[test]
    fn the_workspace_book_builds_and_answers_a_query() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temporary = std::env::temp_dir().join(format!("rola-dev-book-{}", std::process::id()));
        let output = temporary.join("builtin.book");

        let report = crate::build(&root.join(crate::BOOK_DIR), &output).expect("the book builds");
        assert!(
            report.articles >= 1,
            "the tree's book should hold the example article"
        );

        let book = BookSearch::open(&output).expect("the book opens");
        let hits = book.search("Article Example", 10);
        assert_eq!(hits[0].title, "Article Example");
        assert!(
            hits[0].description.contains("sample"),
            "a hit should carry the description the article declares"
        );

        std::fs::remove_dir_all(&temporary).ok();
    }
}

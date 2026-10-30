//! The `rorolala-dev-book-index` entry point: the book written as one file.
//!
//! Run with no arguments it builds the checkout's own book: `book/` beside the crate's manifest is
//! read, and `build/docs/builtin.book` under the same root is written. A caller that lays a build
//! out somewhere else — `cargo dev export` with `BUILD_DIR` set — names the two paths instead:
//!
//! ```text
//! rorolala-dev-book-index [<book-dir> [<output-file>]]
//! ```
//!
//! The program is deliberately one job with two paths rather than a command tree: what it is for
//! is stated by its name, and the two paths are what the build that runs it already knows.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Where the checkout this program was built in keeps its book.
const BOOK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../book");

/// The checkout this program was built in, as the crate's own manifest names it.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// What a run was told.
struct Asked {
    /// The directory the articles are read from.
    book: PathBuf,
    /// The file the book is written to.
    output: PathBuf,
}

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);

    let (default_book, default_output) = rorolala_dev_book_index::workspace_paths(Path::new(ROOT));

    let book = arguments.next().map_or(default_book, PathBuf::from);
    let output = arguments.next().map_or(default_output, PathBuf::from);

    if arguments.next().is_some() {
        eprintln!("book-index: at most two paths, the book directory and the output file");
        return ExitCode::from(2);
    }

    let asked = Asked { book, output };

    match build(&asked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("book-index: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Builds the book and says what was written.
fn build(asked: &Asked) -> Result<(), rorolala_dev_book_index::Error> {
    let report = rorolala_dev_book_index::build(&asked.book, &asked.output)?;

    println!(
        "{}: {} article{}, {} bytes",
        report.output.display(),
        report.articles,
        if report.articles == 1 { "" } else { "s" },
        report.bytes
    );

    Ok(())
}

/// The book directory the crate's own manifest sits under.
///
/// It is read at compile time, so a program run from anywhere answers about the tree it was built
/// from rather than about wherever the shell happens to be.
#[must_use]
pub fn default_book() -> &'static Path {
    Path::new(BOOK)
}

//! `book`: the built-in book written as the one file the programs search.
//!
//! The writing is `rorolala-dev-book-index`'s: it owns the two paths the file is read from and
//! written to, and it reads `BUILD_DIR` for the second so that the file lands in the export this
//! run laid out. What this step does is run it, which is also what an export does — a person who
//! runs `cargo dev book` and a package that carries the result are made by the same command.

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::paths::Workspace;
use crate::run_step;
use crate::toolchain::Tool;

/// Writes the built-in book's search file.
#[command(node = "book")]
pub fn book() -> StateBook {
    StateBook
}

/// The state the command starts in: it names nothing, since the book is the whole of what is made.
#[derive(Grouped)]
pub struct StateBook;

/// Writes the book into the export's own directory.
///
/// The tool is run from the workspace root, so the workspace it belongs to is the one found from
/// there rather than the one this process happened to be started in, and `BUILD_DIR` is inherited
/// by it: the file goes into the directory this run is laying out, whether that is `build` or the
/// one an override names.
///
/// # Errors
///
/// Reports whatever stopped the build — the tool failing to compile or to start, and the failure
/// of the book itself, which says which article and what about it.
pub fn build_book(workspace: &Workspace) -> Result<(), crate::Error> {
    Tool::cargo().run_in(
        workspace.root(),
        ["run", "--quiet", "-p", "rorolala-dev-book-index"],
    )
}

/// Writes the book.
#[chain]
pub fn handle_book(_state: StateBook, ec: &mut ResExitCode) -> Next {
    run_step(ec, || {
        let workspace = Workspace::discover()?;

        build_book(&workspace)
    });

    empty_result!()
}

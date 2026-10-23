//! `chunker`: the chunking harness, run over a corpus.
//!
//! The options this takes are the harness's own — its corpus, its weights, its kinds of report — and
//! they are printed by the harness itself. Re-describing them here would be a second copy of a
//! surface that already exists, and the copy is the one that would drift, so the command is a
//! pass-through: [`passthrough`] hands the words after `chunker` to the harness unchanged.

use crate::Error;
use crate::toolchain::Tool;

/// How much of a run is the command's own name before the harness's options start.
const COMMAND: &str = "chunker";

/// The package the harness is built from.
const HARNESS: &str = "rorolala-dev-chunker-harness";

/// Runs the harness with the options after `chunker`, when the run is that command.
///
/// Returns the status the program exits with when the run was the harness's, and nothing when it was
/// not: a pass-through that is not taken must leave the run to the program's own command tree.
pub fn passthrough(arguments: &[String]) -> Option<i32> {
    if arguments.first().map(String::as_str) != Some(COMMAND) {
        return None;
    }

    let options = &arguments[1..];

    let mut words: Vec<String> = vec![
        "run".to_owned(),
        "--quiet".to_owned(),
        "--release".to_owned(),
        "-p".to_owned(),
        HARNESS.to_owned(),
        "--".to_owned(),
    ];
    words.extend(options.iter().cloned());

    Some(match Tool::cargo().run(words) {
        Ok(()) => 0,
        Err(error @ Error::Status { code, .. }) => {
            eprintln!("{error}");
            code
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    })
}

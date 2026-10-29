//! Scores chunking strategies against frozen general-purpose yardsticks.
//!
//! This is the measuring half of the chunking work. A strategy is a function from content to the
//! ranges it is cut into; what a strategy is *worth* is how much of a version chain the store ends up
//! holding because of it, how much an edit costs, and how fast the cut is. This program answers those
//! for every chunker it is given, over a corpus of version chains, and prints the comparison.
//!
//! Three things are deliberate:
//!
//! - **The yardsticks are the frozen general chunkers.** `whole`, `fixed-*`, `cdc-*`, `text` and
//!   `zip` are the ones the store falls back on, and they are not tuned. A strategy's score is its
//!   distance from the best of them *on the same chain*, so the number keeps its meaning as the
//!   strategies change.
//! - **Time is not scored.** A weighted timing term would make a laptop and a build machine disagree
//!   about a strategy, which is the one thing a score that accumulates cannot afford. Speed is a
//!   floor and a column.
//! - **Correctness is a veto.** A chunker whose ranges do not cover its input, or that cuts the same
//!   input two ways, gets no score at all.
//!
//! What is measured is what ships: the same `Chunking` values the store uses, from the same crate.
//! This is a development tool: it is crude on purpose, and it is what runs, not the interface.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

mod baseline;
mod corpus;
mod generate;
mod measure;
mod peak;
mod report;
mod score;
mod scratch;
mod store;

#[global_allocator]
static ALLOCATOR: peak::Counting = peak::Counting;

use std::path::PathBuf;
use std::process::ExitCode;

use arg_picker::consts::UNMATCHED;
use arg_picker::value::Flag;
use arg_picker::{Picker, PickerArg};

use crate::corpus::Corpus;
use crate::score::Weights;

/// Where the corpora are read from when no other place is named.
///
/// It is the chunking crate's own `corpus/`, beside the policies the corpora belong to: a policy and
/// the chains that judge it are one contribution, and a report that had to be told where they live
/// would be a report that can measure the wrong ones. The path is tied to this crate rather than to
/// the working directory, so the program answers the same from anywhere.
const CORPUS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../modules/chunking/corpus");

/// The file the weights and the floors are read from.
const WEIGHTS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/score.toml");

/// What the program is told, and what it says about itself.
///
/// Written out rather than derived: `arg-picker` picks arguments and does not render help, and what a
/// development tool needs to say about its five options is one paragraph.
const USAGE: &str = "\
usage: chunker [options]

  --corpus <dir>     read the chains from <dir> (default: the chunking crate's corpus/)
  --generate <name>  write a synthetic chain set under <dir>/<name>, then measure it
  --check            validate the corpus and cut nothing
  --score            write each policy's SCORE.md beside its corpus
  --store            also measure through a real store, and compare it with the model
  -h, --help         this text

A corpus is <dir>/<policy>/<pattern>/[<lane>/]<version>.<ext>, where `<version>` is a number
starting at 000 and counting up by one, and what a chain is made of is the order those numbers
name. A pattern with two lanes is a content shared between two paths: it is measured as one chain,
so what the two have in common is counted as deduplicated.
";

/// The directory the chains are read from.
///
/// Built through the constructor rather than with `arg!`, as `rola-edit` does: the macro resolves its
/// paths through whichever crate `arg-picker` was compiled for, and in a whole-workspace build that is
/// mingling, which this program does not depend on.
const ARG_CORPUS: PickerArg<'static, Option<PathBuf>> = PickerArg::new(&["corpus"], None, false);

/// The policy name a synthetic corpus is written under.
const ARG_GENERATE: PickerArg<'static, Option<String>> = PickerArg::new(&["generate"], None, false);

/// Read the corpus and say what it holds, cutting nothing.
const ARG_CHECK: PickerArg<'static, Flag> = PickerArg::new(&["check"], None, false);

/// Write each policy's `SCORE.md` beside its corpus, from the numbers this run measured.
const ARG_SCORE: PickerArg<'static, Flag> = PickerArg::new(&["score"], None, false);

/// Measure through a real store as well as through the model, and say whether they agree.
const ARG_STORE: PickerArg<'static, Flag> = PickerArg::new(&["store"], None, false);

/// Say what the options are and stop.
const ARG_HELP: PickerArg<'static, Flag> = PickerArg::new(&["help"], Some('h'), false);

/// The options that take a value, so that one written last can be refused.
///
/// `arg-picker` consumes the option's own word and then finds nothing after it, which answers "not
/// given" — and a run that meant to measure one corpus would measure the default one instead. A
/// measurement of something other than what was asked for is the one thing this tool must not do, so
/// the words are looked at as well as the picks.
const WITH_VALUE: [&str; 2] = ["--corpus", "--generate"];

#[tokio::main]
async fn main() -> ExitCode {
    // UNWRAP: picking cannot fail — a positional or flag that is absent answers `None`/`Inactive`
    // rather than an error, and a word no option claimed lands in `UNMATCHED` rather than in `Err`.
    let (corpus, generate, check, writing, through_store, help, unmatched) = Picker::from_args()
        .pick_or_default(ARG_CORPUS)
        .pick_or_default(ARG_GENERATE)
        .pick_or_default(ARG_CHECK)
        .pick_or_default(ARG_SCORE)
        .pick_or_default(ARG_STORE)
        .pick_or_default(ARG_HELP)
        .pick(&UNMATCHED)
        .unwrap();

    if *help {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    // A word no option claimed is the caller's mistake, and one that is passed over quietly is a
    // measurement of something other than what was asked for.
    if !unmatched.is_empty() {
        for word in &unmatched {
            eprintln!("chunker: no such argument: `{}`", word.raw);
        }
        eprintln!("\n{USAGE}");

        return ExitCode::FAILURE;
    }

    if let Some(option) = dangling(unmatched.original()) {
        eprintln!("chunker: `{option}` needs a value\n\n{USAGE}");

        return ExitCode::FAILURE;
    }

    let root = corpus.unwrap_or_else(|| PathBuf::from(CORPUS_DIR));

    let asked = Asked {
        generate: generate.as_deref(),
        checking: *check,
        scoring: *writing,
        through_store: *through_store,
    };

    match run(&root, &asked).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("chunker: {error}");
            ExitCode::FAILURE
        }
    }
}

/// What the run was asked to do.
struct Asked<'a> {
    /// The policy name a synthetic corpus is written under.
    generate: Option<&'a str>,
    /// Validate the corpus and cut nothing.
    checking: bool,
    /// Write each policy's `SCORE.md` beside its corpus.
    scoring: bool,
    /// Measure through a real store as well.
    through_store: bool,
}

/// Does what the arguments say, and says what it found.
async fn run(root: &std::path::Path, asked: &Asked<'_>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(name) = asked.generate {
        crate::generate::synthetic(root, name, crate::generate::Scale::Full)?;
        println!(
            "wrote a synthetic chain set to {}",
            root.join(name).display()
        );
    }

    let corpus = Corpus::read(root)?;

    if asked.checking {
        println!(
            "{} corpus, {} pattern{}",
            corpus.policies.len(),
            corpus.patterns(),
            if corpus.patterns() == 1 { "" } else { "s" },
        );
        return Ok(());
    }

    // The policy directories are carried while their chains are not (see
    // `modules/chunking/corpus/.gitignore`), so a checkout with no captured chains has policies and
    // no patterns. Nothing can be measured over it, and `--score` must not replace a committed score
    // with a record of no measurement.
    if corpus.patterns() == 0 {
        println!(
            "no chains under {}: a policy's chains are put beside its corpus by whoever captured \
             them, or named with `--corpus <dir>`, and `--generate synthetic` writes a chain set for \
             the yardsticks alone",
            root.display()
        );
        return Ok(());
    }

    let weights = Weights::read(std::path::Path::new(WEIGHTS_FILE))?;
    report::print(&corpus, &weights, asked.through_store, asked.scoring).await?;

    Ok(())
}

/// The option `words` ends with, when it is one that takes a value and did not get one.
fn dangling(words: &[String]) -> Option<&str> {
    let last = words.last()?;

    WITH_VALUE.contains(&last.as_str()).then_some(last.as_str())
}

#[cfg(test)]
mod tests {
    use super::dangling;

    #[test]
    fn an_option_without_its_value_is_named() {
        // `arg-picker` answers "not given" for both of these, so the words are what tell them apart.
        assert_eq!(dangling(&["--corpus".to_owned()]), Some("--corpus"));
        assert_eq!(dangling(&["--corpus".to_owned(), "here".to_owned()]), None);
        assert_eq!(dangling(&["--check".to_owned()]), None);
        assert_eq!(dangling(&[]), None);
    }
}

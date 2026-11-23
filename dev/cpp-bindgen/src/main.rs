//! Command line for the C++ binding generator: the header to read, then the directory to fill.
//!
//! It is a program rather than only a library because `cargo dev export` hands it paths and asks
//! nothing else; the compile check it runs is part of the deliverable, not of the caller's build.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rororala_dev_cpp_bindgen::{Config, generate};

const USAGE: &str =
    "usage: cpp-bindgen <header.h> <gen-dir> [--expected-abi <n>] [--umbrella <file>]";

/// Runs the generator over the header and the directory it is given.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match arguments(&args) {
        Ok(arguments) => match generate(&arguments.config()) {
            Ok(counts) => {
                println!(
                    "{} functions, {} handles, {} outcomes, {} error families, {} constants",
                    counts.functions,
                    counts.handles,
                    counts.outcomes,
                    counts.error_families,
                    counts.constants
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// What the command line names, kept alive for the borrows [`Config`] holds.
struct Arguments {
    /// The header to read.
    header: PathBuf,
    /// The directory the headers are written to.
    gen_dir: PathBuf,
    /// The ABI major to assert, when it was named.
    expected_abi: Option<u32>,
    /// The umbrella header to write, when it was named.
    umbrella: Option<PathBuf>,
}

impl Arguments {
    /// The generator's view of what was named.
    fn config(&self) -> Config<'_> {
        Config {
            header: &self.header,
            gen_dir: &self.gen_dir,
            expected_abi: self.expected_abi,
            umbrella: self.umbrella.as_deref(),
        }
    }
}

/// Reads the header, the directory and the options after them.
fn arguments(args: &[String]) -> Result<Arguments, String> {
    let mut positional: Vec<&str> = Vec::new();
    let mut expected_abi = None;
    let mut umbrella = None;
    let mut at = 0;

    while at < args.len() {
        match args[at].as_str() {
            "--expected-abi" => {
                at += 1;
                let value = value(args, at, "--expected-abi")?;
                expected_abi = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| format!("`{value}` is not an ABI major\n{USAGE}"))?,
                );
            }
            "--umbrella" => {
                at += 1;
                umbrella = Some(Path::new(value(args, at, "--umbrella")?).to_path_buf());
            }
            "--help" | "-h" => return Err(USAGE.to_owned()),
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}`\n{USAGE}"));
            }
            other => positional.push(other),
        }

        at += 1;
    }

    let [header, gen_dir] = positional.as_slice() else {
        return Err(USAGE.to_owned());
    };

    Ok(Arguments {
        header: Path::new(header).to_path_buf(),
        gen_dir: Path::new(gen_dir).to_path_buf(),
        expected_abi,
        umbrella,
    })
}

/// The value after an option, or the usage when there is none.
fn value<'args>(args: &'args [String], at: usize, option: &str) -> Result<&'args str, String> {
    args.get(at)
        .map(String::as_str)
        .ok_or_else(|| format!("`{option}` needs a value\n{USAGE}"))
}

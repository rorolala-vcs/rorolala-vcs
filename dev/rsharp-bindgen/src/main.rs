//! Command line for the C# binding generator: the header to read, then the file to write.
//!
//! A C# project runs this on every build (`RolaSharp` does), which is why it is a program rather than
//! only a library: the build hands it the paths and the names it wants, and asks nothing else.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rorolala_dev_rsharp_bindgen::{Config, generate};

const USAGE: &str = "\
usage: rsharp-bindgen <header.h> <binding.cs> [--namespace <ns>] [--class <name>] [--policies <file>]";

/// Runs the generator over the header and the binding it is given.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match arguments(&args) {
        Ok(arguments) => match generate(&arguments.config()) {
            Ok(_) => ExitCode::SUCCESS,
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
    /// The binding to write.
    binding: PathBuf,
    /// The namespace the binding lives in.
    namespace: String,
    /// The native class the functions are members of.
    class: String,
    /// The policy enum to write, when it was named.
    policies: Option<PathBuf>,
}

impl Arguments {
    /// The generator's view of what was named.
    fn config(&self) -> Config<'_> {
        Config {
            header: &self.header,
            binding: &self.binding,
            namespace: &self.namespace,
            class: &self.class,
            policies: self.policies.as_deref(),
        }
    }
}

/// Reads the header, the binding and the options after them.
fn arguments(args: &[String]) -> Result<Arguments, String> {
    let mut positional: Vec<&str> = Vec::new();
    let mut namespace = "RolaSharp".to_owned();
    let mut class = "RorolalaBinding".to_owned();
    let mut policies: Option<PathBuf> = None;
    let mut at = 0;

    while at < args.len() {
        match args[at].as_str() {
            "--namespace" => {
                at += 1;
                value(args, at, "--namespace")?.clone_into(&mut namespace);
            }
            "--class" => {
                at += 1;
                value(args, at, "--class")?.clone_into(&mut class);
            }
            "--policies" => {
                at += 1;
                policies = Some(Path::new(value(args, at, "--policies")?).to_path_buf());
            }
            "--help" | "-h" => return Err(USAGE.to_owned()),
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}`\n{USAGE}"));
            }
            other => positional.push(other),
        }

        at += 1;
    }

    let [header, binding] = positional.as_slice() else {
        return Err(USAGE.to_owned());
    };

    Ok(Arguments {
        header: Path::new(header).to_path_buf(),
        binding: Path::new(binding).to_path_buf(),
        namespace,
        class,
        policies,
    })
}

/// The value after an option, or the usage when there is none.
fn value<'args>(args: &'args [String], at: usize, option: &str) -> Result<&'args str, String> {
    args.get(at)
        .map(String::as_str)
        .ok_or_else(|| format!("`{option}` needs a value\n{USAGE}"))
}

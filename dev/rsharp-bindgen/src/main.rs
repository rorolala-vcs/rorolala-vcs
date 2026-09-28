//! Command line for the C# binding generator: the header to read, then the file to write.
//!
//! A C# project runs this on every build (`RolaSharp` does), which is why it is a program rather than
//! only a library: the build hands it the two paths and asks nothing else.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::path::Path;
use std::process::ExitCode;

use rorolala_dev_rsharp_bindgen::{Config, generate};

/// Runs the generator over the header and the binding it is given.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let [header, binding] = args.as_slice() else {
        eprintln!("usage: rsharp-bindgen <header.h> <binding.cs>");
        return ExitCode::FAILURE;
    };

    let config = Config {
        header: Path::new(header),
        binding: Path::new(binding),
    };

    match generate(&config) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

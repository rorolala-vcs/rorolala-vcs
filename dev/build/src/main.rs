//! The `cargo dev` entry point: the commands that produce this workspace's artifacts.
//!
//! Everything the program can do is declared in the library beside this file, one command to one
//! module; this is only what starts it. `DefaultSetup` is the arrangement the rest of the tree's
//! programs use — help, quiet, verbose, the exit code a task asks for, and the environment
//! resources — so a build step exits the way it reports.
//!
//! One command is answered before the tree is entered: `chunker` hands its words to the harness
//! unchanged, because what it takes is that program's own options and a second description of them
//! here is the copy that would drift.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use mingling::setup::DefaultSetup;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = rola_build::chunker_passthrough(&arguments) {
        std::process::exit(code);
    }

    let mut program = rola_build::ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.exec_and_exit();
}

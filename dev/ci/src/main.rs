//! The `cargo ci` entry point: the checks rola is held to.
//!
//! Everything the program can do is declared in the library beside this file, one check to one
//! module; this is only what starts it. `DefaultSetup` is the arrangement the rest of the tree's
//! programs use — help, quiet, verbose, the exit code a check asks for, and the environment
//! resources — so a check exits the way it reports.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use mingling::setup::DefaultSetup;

fn main() {
    let mut program = rola_ci::ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.exec_and_exit();
}

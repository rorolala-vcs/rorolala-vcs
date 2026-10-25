//! The `cargo changelog` entry point: the editor for what changed between releases.
//!
//! Everything the program can do is declared in the library beside this file, one command to one
//! module; this is only what starts it. `DefaultSetup` is the arrangement the rest of the tree's
//! programs use — help, the exit code a command asks for, and the environment resources — and
//! [`rola_changelog::AllowDirtySetup`] consumes `--allow-dirty` before any command sees its words.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use mingling::setup::DefaultSetup;

fn main() {
    let mut program = rola_changelog::ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.with_setup(rola_changelog::AllowDirtySetup);
    program.exec_and_exit();
}

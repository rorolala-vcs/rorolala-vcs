#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]

rust_i18n::i18n!("i18n", fallback = "en");

use mingling::{
    macros::{chain, gen_program},
    setup::DefaultSetup,
};
use rorolala_cli_setups::RorolalaSetup;

use crate::cmd_listen::EntryListen;

mod cmd_listen;
mod exit_codes;

fn main() {
    let mut program = ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.with_setup(RorolalaSetup);
    program.exec_and_exit();
}

#[chain]
pub(crate) fn handle_fallback(args: EntryFallback) -> EntryListen {
    // When no subcommand matches, automatically route to listen
    EntryListen::from(args.0)
}

gen_program!();

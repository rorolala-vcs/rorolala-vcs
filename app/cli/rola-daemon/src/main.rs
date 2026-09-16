#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]

rust_i18n::i18n!("../../../i18n/rola-daemon", fallback = "en");

use mingling::{
    macros::{buffer, gen_program, r_println, renderer},
    setup::DefaultSetup,
};
use rorolala_cli_setups::RorolalaSetup;

mod cmd_listen;

fn main() {
    let mut program = ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.with_setup(RorolalaSetup);
    program.exec_and_exit();
}

#[renderer(buffer)]
pub(crate) fn render_fallback(args: EntryFallback) {
    r_println!("Command not found: {}", args.join(" "));
}

gen_program!();

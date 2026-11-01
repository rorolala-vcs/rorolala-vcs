//! The bare `rola layout` command: the Layout a run works in.
//!
//! Naming no subcommand asks the same thing as `rola layout ls`, so it is the same listing: the one
//! Layout a Workspace works in, or the one a Vault keeps.

use mingling::{
    Suggest,
    macros::{buffer, command, completion, help, metadata, r_eprintln, suggest},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::exit_codes::EC_HELP;
use crate::format::ResFormat;
use crate::layout::cmd_layout_ls::{DEFAULT_FORMAT, StateLayoutLs};

#[help(buffer)]
pub fn help_layout(_: EntryLayout, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayout)]
pub fn desc_layout() -> Description {
    t!("cmd_layout.description").to_string().into()
}

/// Completes what `rola layout` can be given next.
///
/// The namespace names nothing of its own and its subcommands are put to the line by the
/// dispatcher, so there is nothing here to offer.
#[completion(EntryLayout)]
pub fn complete_layout() -> Suggest {
    suggest!()
}

/// Lists the Layout a run works in
///
/// The same as `rola layout ls`: the one Layout a Workspace works in, or the one a Vault keeps.
#[command(node = "layout")]
pub fn layout(format: &mut ResFormat) -> StateLayoutLs {
    format.default_template(DEFAULT_FORMAT);
    StateLayoutLs
}

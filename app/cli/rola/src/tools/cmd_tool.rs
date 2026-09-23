//! The `rola tool` namespace: the low-level tools that are not version control.
//!
//! `tool` is a namespace rather than one thing to do — a file is put in, a hash is taken out, a pack
//! is looked at — and what it holds is what sits under version control rather than part of it: the
//! store the content is kept in, the keys an account is proved with, and the words two ends exchange
//! before either moves anything. Each of its commands is therefore two words deep, `tool write-file`
//! for one, so that adding another is adding another word rather than reshaping this one.
//!
//! Reaching `tool` with none of those words is a question about the namespace rather than about one
//! of its commands, so it is answered with what the namespace holds — see [`handle_tool`] — and the
//! same words answer `rola tool -h`.

use mingling::{
    Grouped,
    macros::{buffer, chain, dispatcher, help, metadata, r_eprintln, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;

dispatcher!("tool", EntryTools);

#[help(buffer)]
pub fn help_tool(_: EntryTools, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTools)]
pub fn desc_tool() -> Description {
    t!("tool.cmd_tool_description").to_string().into()
}

/// Names what `tool` holds.
///
/// A command the namespace has is matched before this is reached, so what arrives here is a run that
/// named none of them — or one it does not have. Either is a question about the namespace rather than
/// about a command of it, and both are answered the way [`help_tool`] answers.
#[chain]
pub fn handle_tool(_: EntryTools) -> Next {
    ResultToolHelp.into()
}

/// Result: what `tool` holds was named.
///
/// The same answer [`help_tool`] gives, since a run that reached `tool` with nothing and a run that
/// asked it for help are asking the same thing. What it says is written out twice rather than shared,
/// because what puts it on the screen — `r_eprintln!` — is a buffer only the attributes open.
#[derive(Grouped)]
pub struct ResultToolHelp;

#[renderer(buffer)]
pub fn render_result_tool_help(_: ResultToolHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool.help")).trim());
    ec.exit_code = EC_HELP;
}

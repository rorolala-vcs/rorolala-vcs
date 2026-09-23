//! The `rola storage` namespace: what a store holds, read and written by hand.
//!
//! `storage` is a namespace rather than one thing to do — a file is put in, a hash is taken out, a
//! pack is looked at — and what it holds is what sits under version control rather than part of it:
//! the store the content is kept in. Each of its commands is therefore two words deep,
//! `storage write-file` for one, so that adding another is adding another word rather than
//! reshaping this one.
//!
//! Reaching `storage` with none of those words is a question about the namespace rather than about
//! one of its commands, so it is answered with what the namespace holds — see [`handle_storage`] —
//! and the same words answer `rola storage -h`.

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

dispatcher!("storage", EntryStorage);

#[help(buffer)]
pub fn help_storage(_: EntryStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorage)]
pub fn desc_storage() -> Description {
    t!("storage.cmd_storage_description").to_string().into()
}

/// Names what `storage` holds.
///
/// A command the namespace has is matched before this is reached, so what arrives here is a run
/// that named none of them — or one it does not have. Either is a question about the namespace
/// rather than about a command of it, and both are answered the way [`help_storage`] answers.
#[chain]
pub fn handle_storage(_: EntryStorage) -> Next {
    ResultStorageHelp.into()
}

/// Result: what `storage` holds was named.
///
/// The same answer [`help_storage`] gives, since a run that reached `storage` with nothing and a
/// run that asked it for help are asking the same thing. What it says is written out twice rather
/// than shared, because what puts it on the screen — `r_eprintln!` — is a buffer only the
/// attributes open.
#[derive(Grouped)]
pub struct ResultStorageHelp;

#[renderer(buffer)]
pub fn render_result_storage_help(_: ResultStorageHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage.help")).trim());
    ec.exit_code = EC_HELP;
}

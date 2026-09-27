//! The `rola vcs-index` namespace: the version control index, read and written.
//!
//! The index holds four kinds of object — a Variant and a Version, a Creator and a Message — under
//! the hashes they are named by, and every command here speaks in those hashes: what a listing
//! prints is a hash to hand to another command, and what a write prints is the hash of what it
//! stored.
//!
//! Each command is a module of its own beside this one — see the `cmd_vcs_index_*` files — so that
//! adding one is adding a file rather than reshaping this one. What stands here is what they share:
//! the failures any of them can reach for, the hash of what a write stored, and the little that
//! reading a hash and driving the index takes.

pub mod cmd_vcs_index_create_rootver;
pub mod cmd_vcs_index_lookback;
pub mod cmd_vcs_index_ls_remote_str;
pub mod cmd_vcs_index_ls_remote_variants;
pub mod cmd_vcs_index_ls_remote_versions;
pub mod cmd_vcs_index_ls_str;
pub mod cmd_vcs_index_ls_variants;
pub mod cmd_vcs_index_ls_versions;
pub mod cmd_vcs_index_my_creator_hash;
pub mod cmd_vcs_index_print_rootver;
pub mod cmd_vcs_index_read;
pub mod cmd_vcs_index_read_remote;
pub mod cmd_vcs_index_sync_all;
pub mod cmd_vcs_index_write_creator;
pub mod cmd_vcs_index_write_msg;
pub mod cmd_vcs_index_write_variant;
pub mod cmd_vcs_index_write_version;

use librorolala::auth::Account;
use librorolala::daemon::{action_list_remote_index_async, action_read_remote_index_async};
use librorolala::protocol::ActionError;
use librorolala::storage::Key;
use librorolala::vcs::{VCSIndex, VCSIndexObject};
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, StructuralData,
    macros::{buffer, chain, dispatcher, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{Colorize as _, err_line, help_line, trd};
use rorolala_utils_progress::Progress;
use rust_i18n::t;
use serde::Serialize;
use std::str::FromStr as _;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_VCS_INDEX_ARGUMENT, EC_ERR_VCS_INDEX_NO_INDEX, EC_ERR_VCS_INDEX_NOT_FOUND,
    EC_ERR_VCS_INDEX_READ, EC_ERR_VCS_INDEX_WRITE, EC_HELP,
};
use crate::failure::failure;

dispatcher!("vcs-index", EntryVcsIndex);

#[help(buffer)]
pub fn help_vcs_index(_: EntryVcsIndex, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndex)]
pub fn desc_vcs_index() -> Description {
    t!("vcs_index.cmd_vcs_index_description").to_string().into()
}

/// Names what `vcs-index` holds.
///
/// A command the namespace has is matched before this is reached, so what arrives here is a run
/// that named none of them — or one it does not have. Either is a question about the namespace
/// rather than about a command of it, and both are answered the way [`help_vcs_index`] answers.
#[chain]
pub fn handle_vcs_index(_: EntryVcsIndex) -> Next {
    ResultVcsIndexHelp.into()
}

/// Result: what `vcs-index` holds was named.
///
/// The same answer [`help_vcs_index`] gives, since a run that reached `vcs-index` with nothing and
/// a run that asked it for help are asking the same thing.
#[derive(Grouped)]
pub struct ResultVcsIndexHelp;

#[renderer(buffer)]
pub fn render_result_vcs_index_help(_: ResultVcsIndexHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index.help")).trim());
    ec.exit_code = EC_HELP;
}

/// The hash as the hex a listing prints and a write answers with.
pub fn hex(digest: &[u8; 32]) -> String {
    Key::new(*digest).hex()
}

/// The `(store:..)` and, when it is a merge, `(join:..)` a variant is listed with.
///
/// What a variant points at — the storage entry its content sits in — and what it merges in are
/// both hashes the reader may want next, so both are named beside the variant itself.
pub fn variant_tail(storage: &[u8; 32], join: Option<&[u8; 32]>) -> String {
    let mut tail = String::new();

    for (label, hash) in [("store", Some(storage)), ("join", join)] {
        let Some(hash) = hash else {
            continue;
        };

        tail.push(' ');
        tail.push_str(
            &format!("({label}:{})", hex(hash))
                .bright_yellow()
                .bold()
                .to_string(),
        );
    }

    tail
}

/// The hash `text` names, if it names one.
pub fn parse_hash(text: &str) -> Option<Key> {
    Key::from_str(text).ok()
}

/// The runtime a command drives the index with, or the failure to make one.
pub fn runtime() -> Result<tokio::runtime::Runtime, ErrorVcsIndexRead> {
    tokio::runtime::Runtime::new().map_err(|error| ErrorVcsIndexRead {
        cause: error.to_string(),
    })
}

/// Rebuilds the inverse index from the objects `index` holds
///
/// A write leaves the records describing an index that is one object older than the one now there,
/// so rebuilding them is what `--rebuild` asks a write to do besides writing. It is done
/// deliberately rather than by the index itself: the records are derived data, and a run that does
/// not ask for them kept in step pays nothing for them.
pub fn rebuild_inverse_index(index: &VCSIndex) -> Result<(), ErrorVcsIndexWrite> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| ErrorVcsIndexWrite {
        cause: error.to_string(),
    })?;
    let inverse = librorolala::inverse_index::InverseIndex::at(index.clone());

    runtime
        .block_on(inverse.rebuild())
        .map(|_report| ())
        .map_err(|error| ErrorVcsIndexWrite {
            cause: error.reason(),
        })
}

/// Rebuilds the inverse index of the Vault or Workspace the run is inside
///
/// The counterpart of [`rebuild_inverse_index`] for a command that
/// works through a Workspace rather than an index it holds: the index is the one the run's own
/// directory is inside, found the way every other command finds it.
pub fn rebuild_inverse_index_here() -> Result<(), ErrorVcsIndexWrite> {
    let cwd = std::env::current_dir().map_err(|error| ErrorVcsIndexWrite {
        cause: error.to_string(),
    })?;
    let Some(inverse) = librorolala::inverse_index::InverseIndex::locate(&cwd) else {
        return Err(ErrorVcsIndexWrite {
            cause: "this run is nowhere a version control index is".to_owned(),
        });
    };

    let runtime = tokio::runtime::Runtime::new().map_err(|error| ErrorVcsIndexWrite {
        cause: error.to_string(),
    })?;

    runtime
        .block_on(inverse.rebuild())
        .map(|_report| ())
        .map_err(|error| ErrorVcsIndexWrite {
            cause: error.reason(),
        })
}

/// Error: the run is nowhere an index is.
#[derive(Grouped)]
pub struct ErrorVcsIndexNoIndex;

impl Failure for ErrorVcsIndexNoIndex {
    fn name(&self) -> &'static str {
        "error_vcs_index_no_index"
    }

    fn reason(&self) -> String {
        t!("vcs_index_ls_str.err_no_index").trim().to_string()
    }
}

failure!(ErrorVcsIndexNoIndex);

#[renderer(buffer)]
pub fn render_error_vcs_index_no_index(error: ErrorVcsIndexNoIndex, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("vcs_index_ls_str.err_no_index_help").trim())
    );
    ec.exit_code = EC_ERR_VCS_INDEX_NO_INDEX;
}

/// Error: the index could not be read.
#[derive(Grouped)]
pub struct ErrorVcsIndexRead {
    /// Why the index would not read.
    cause: String,
}

impl Failure for ErrorVcsIndexRead {
    fn name(&self) -> &'static str {
        "error_vcs_index_read"
    }

    fn reason(&self) -> String {
        t!("vcs_index_ls_str.err_read", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorVcsIndexRead);

#[renderer(buffer)]
pub fn render_error_vcs_index_read(error: ErrorVcsIndexRead, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("vcs_index_ls_str.err_read_help").trim())
    );
    ec.exit_code = EC_ERR_VCS_INDEX_READ;
}

/// Error: the index could not be written.
#[derive(Grouped)]
pub struct ErrorVcsIndexWrite {
    /// Why the index would not write.
    cause: String,
}

impl Failure for ErrorVcsIndexWrite {
    fn name(&self) -> &'static str {
        "error_vcs_index_write"
    }

    fn reason(&self) -> String {
        t!("vcs_index.err_write", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorVcsIndexWrite);

#[renderer(buffer)]
pub fn render_error_vcs_index_write(error: ErrorVcsIndexWrite, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("vcs_index.err_write_help").trim()));
    ec.exit_code = EC_ERR_VCS_INDEX_WRITE;
}

/// Error: an argument the command needs was not given.
#[derive(Grouped)]
pub struct ErrorVcsIndexArgument {
    /// The argument that was missing.
    argument: String,
}

impl Failure for ErrorVcsIndexArgument {
    fn name(&self) -> &'static str {
        "error_vcs_index_argument"
    }

    fn reason(&self) -> String {
        t!("vcs_index.err_argument", argument = self.argument)
            .trim()
            .to_string()
    }
}

failure!(ErrorVcsIndexArgument);

#[renderer(buffer)]
pub fn render_error_vcs_index_argument(error: ErrorVcsIndexArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("vcs_index.err_argument_help").trim()));
    ec.exit_code = EC_ERR_VCS_INDEX_ARGUMENT;
}

/// Error: an argument does not read as a hash.
#[derive(Grouped)]
pub struct ErrorVcsIndexHash {
    /// What was given instead of a hash.
    hash: String,
}

impl Failure for ErrorVcsIndexHash {
    fn name(&self) -> &'static str {
        "error_vcs_index_hash"
    }

    fn reason(&self) -> String {
        t!("vcs_index.err_bad_hash", hash = self.hash)
            .trim()
            .to_string()
    }
}

failure!(ErrorVcsIndexHash);

#[renderer(buffer)]
pub fn render_error_vcs_index_hash(error: ErrorVcsIndexHash, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("vcs_index.err_bad_hash_help").trim()));
    ec.exit_code = EC_ERR_VCS_INDEX_ARGUMENT;
}

/// Error: what was asked for is not in the index.
///
/// One object is named and it is not there. It is told apart from [`ErrorVcsIndexRead`], which is a
/// read that could not be finished: this is one that was finished, and had nothing to answer.
#[derive(Grouped)]
pub struct ErrorVcsIndexNotFound {
    /// What was looked for, as hex.
    hash: String,
}

impl Failure for ErrorVcsIndexNotFound {
    fn name(&self) -> &'static str {
        "error_vcs_index_not_found"
    }

    fn reason(&self) -> String {
        t!("vcs_index.err_not_found", hash = self.hash)
            .trim()
            .to_string()
    }
}

failure!(ErrorVcsIndexNotFound);

#[renderer(buffer)]
pub fn render_error_vcs_index_not_found(error: ErrorVcsIndexNotFound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("vcs_index.err_not_found_help").trim()));
    ec.exit_code = EC_ERR_VCS_INDEX_NOT_FOUND;
}

/// Result: the hash of what was written.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexHash {
    /// The hash, as hex.
    hash: String,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_hash(result: ResultVcsIndexHash) {
    r_println!("{}", result.hash);
}

/// Result: the hashes a listing named.
///
/// A remote listing names hashes and nothing else — what each one holds is read with
/// `rola vcs-index read-remote` — so the same result answers every one of them, and a hash read
/// here is one to hand straight on. The field is the same one
/// [`ls-str`](crate::vcs_index::cmd_vcs_index_ls_str) writes, since what is held is the same thing:
/// a list of hashes, each as hex.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultHashes {
    /// The hashes, as hex.
    string_hashes: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_hashes(result: ResultHashes) {
    for hash in &result.string_hashes {
        r_println!("{hash}");
    }
}

/// The hashes a listing printed, one a line.
pub fn keys_of(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Lists what the other end's index holds, by the kind `which` names.
///
/// It is the requesting half of [`ActionListRemoteIndex`](librorolala::daemon::ActionListRemoteIndex):
/// the Vault is asked and answers, and nothing but the listing moves. What comes back is named the
/// way the local `ls-*` names it, one hash a line.
///
/// # Errors
///
/// Returns an [`ActionError`] when the exchange fails or the runtime cannot be built.
pub fn list_remote_hashes(
    workspace: &Workspace,
    account: &Account,
    target: &str,
    which: &str,
) -> Result<ResultHashes, ActionError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| ActionError::Io(error.into()))?;
    let listing = runtime.block_on(action_list_remote_index_async(
        workspace,
        account,
        target.to_owned(),
        which.to_owned(),
        Progress::silent(),
    ))?;

    Ok(ResultHashes {
        string_hashes: keys_of(&listing),
    })
}

/// Reads one object from the other end's index.
///
/// It is the requesting half of [`ActionReadRemoteIndex`](librorolala::daemon::ActionReadRemoteIndex):
/// the Vault reads the object under `hash` from its own index and answers with it. A Version comes
/// back with the number the Vault's index traced it to, since the chain is there and not here.
///
/// # Errors
///
/// Returns an [`ActionError`] when the exchange fails or the runtime cannot be built.
pub fn read_remote(
    workspace: &Workspace,
    account: &Account,
    target: &str,
    hash: &str,
) -> Result<VCSIndexObject, ActionError> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| ActionError::Io(error.into()))?;

    runtime.block_on(action_read_remote_index_async(
        workspace,
        account,
        target.to_owned(),
        hash.to_owned(),
        Progress::silent(),
    ))
}

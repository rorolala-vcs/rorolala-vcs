//! The `rola inv-idx` namespace: the inverse index, built and asked.
//!
//! The version control index answers "what is stored under this hash"; the inverse index answers
//! the other way — which objects depend on a key. It is derived data, so a rebuild is what makes
//! it, and a read falls back to the objects when it does not describe the index as it stands.
//!
//! Each command is a module of its own beside this one — see the `cmd_inv_idx_*` files. What stands
//! here is what they share: the failures they can reach for, the hash-list result they hand back,
//! and the little that reading a hash and driving the index takes.

pub mod cmd_inv_idx_creator_dep;
pub mod cmd_inv_idx_msg_dep;
pub mod cmd_inv_idx_rebuild;
pub mod cmd_inv_idx_store_dep;
pub mod cmd_inv_idx_variant_dep;
pub mod cmd_inv_idx_version_dep;
pub mod cmd_inv_idx_version_num;

use librorolala::inverse_index::InverseIndexReadingError;
use librorolala::storage::Key;
use mingling::{
    Grouped, StructuralData,
    macros::{buffer, chain, dispatcher, help, metadata, r_eprintln, r_print, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_FORMAT, EC_ERR_INV_IDX_ARGUMENT, EC_ERR_INV_IDX_NO_INDEX, EC_ERR_INV_IDX_NOT_FOUND,
    EC_ERR_INV_IDX_READ, EC_HELP,
};
use crate::failure::failure;
use crate::format::ResFormat;
use crate::hash::{HashMiss, resolve};

/// How a listing of hashes is drawn when no template is named.
///
/// One hash a line, which is what the `inv-idx` listings have always printed. The hashes are the
/// one field of the result, `hashes`.
pub const DEFAULT_FORMAT_HASHES: &str = "{{ hashes }}";

dispatcher!("inv-idx", EntryInvIdx);

#[help(buffer)]
pub fn help_inv_idx(_: EntryInvIdx, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInvIdx)]
pub fn desc_inv_idx() -> Description {
    t!("inv_idx.cmd_inv_idx_description").to_string().into()
}

/// Names what `inv-idx` holds.
///
/// A command the namespace has is matched before this is reached, so what arrives here is a run
/// that named none of them — or one it does not have — and both are answered the way
/// [`help_inv_idx`] answers.
#[chain]
pub fn handle_inv_idx(_: EntryInvIdx) -> Next {
    ResultInvIdxHelp.into()
}

/// Result: what `inv-idx` holds was named.
#[derive(Grouped)]
pub struct ResultInvIdxHelp;

#[renderer(buffer)]
pub fn render_result_inv_idx_help(_: ResultInvIdxHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx.help")).trim());
    ec.exit_code = EC_HELP;
}

/// The hash `text` names, or the failure for a word that names no one.
///
/// A whole hash is taken as it is; only the head of one is resolved, and then among `candidates`.
pub fn hash_of(text: &str, candidates: impl FnOnce() -> Vec<Key>) -> Result<Key, ErrorInvIdxHash> {
    resolve(text, candidates).map_err(|miss| ErrorInvIdxHash {
        hash: text.to_owned(),
        miss,
    })
}

/// The runtime a command drives the index with, or the failure to make one.
pub fn runtime() -> Result<tokio::runtime::Runtime, ErrorInvIdxRead> {
    tokio::runtime::Runtime::new().map_err(|error| ErrorInvIdxRead {
        cause: error.to_string(),
    })
}

/// What a read through the inverse index failed with, told the way this namespace reports one.
///
/// Nothing being stored under the key is told apart from a read that could not be made: the first
/// is an answer that there is nothing, the second a question that was not answered.
pub fn reading_error(error: InverseIndexReadingError) -> Next {
    match error {
        InverseIndexReadingError::NotFound { key } => ErrorInvIdxNotFound { hash: key }.into(),
        other => ErrorInvIdxRead {
            cause: other.reason(),
        }
        .into(),
    }
}

/// Result: the hashes an answer names.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultInvIdxHashes {
    /// The hashes, as hex.
    hashes: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_inv_idx_hashes(
    result: ResultInvIdxHashes,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!(
                    "{}",
                    err_line!(t!("format.err_format", reason = error).trim())
                );
                ec.exit_code = EC_ERR_FORMAT;
            }
        }
    } else {
        for hash in &result.hashes {
            r_println!("{hash}");
        }
    }
}

/// Error: the run is nowhere an index is.
#[derive(Grouped)]
pub struct ErrorInvIdxNoIndex;

impl Failure for ErrorInvIdxNoIndex {
    fn name(&self) -> &'static str {
        "error_inv_idx_no_index"
    }

    fn reason(&self) -> String {
        t!("inv_idx.err_no_index").trim().to_string()
    }
}

failure!(ErrorInvIdxNoIndex);

#[renderer(buffer)]
pub fn render_error_inv_idx_no_index(error: ErrorInvIdxNoIndex, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("inv_idx.err_no_index_help").trim()));
    ec.exit_code = EC_ERR_INV_IDX_NO_INDEX;
}

/// Error: the index could not be read, or the inverse index could not be built.
#[derive(Grouped)]
pub struct ErrorInvIdxRead {
    /// Why it would not read.
    cause: String,
}

impl Failure for ErrorInvIdxRead {
    fn name(&self) -> &'static str {
        "error_inv_idx_read"
    }

    fn reason(&self) -> String {
        t!("inv_idx.err_read", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorInvIdxRead);

#[renderer(buffer)]
pub fn render_error_inv_idx_read(error: ErrorInvIdxRead, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("inv_idx.err_read_help").trim()));
    ec.exit_code = EC_ERR_INV_IDX_READ;
}

/// Error: an argument the command needs was not given.
#[derive(Grouped)]
pub struct ErrorInvIdxArgument {
    /// The argument that was missing.
    argument: String,
}

impl Failure for ErrorInvIdxArgument {
    fn name(&self) -> &'static str {
        "error_inv_idx_argument"
    }

    fn reason(&self) -> String {
        t!("inv_idx.err_argument", argument = self.argument)
            .trim()
            .to_string()
    }
}

failure!(ErrorInvIdxArgument);

#[renderer(buffer)]
pub fn render_error_inv_idx_argument(error: ErrorInvIdxArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("inv_idx.err_argument_help").trim()));
    ec.exit_code = EC_ERR_INV_IDX_ARGUMENT;
}

/// Error: an argument does not read as a hash, or names no one object.
#[derive(Grouped)]
pub struct ErrorInvIdxHash {
    /// What was given instead of a hash.
    hash: String,
    /// Why it named no one hash.
    miss: HashMiss,
}

impl Failure for ErrorInvIdxHash {
    fn name(&self) -> &'static str {
        "error_inv_idx_hash"
    }

    fn reason(&self) -> String {
        self.miss.reason(&self.hash, || {
            t!("inv_idx.err_bad_hash", hash = self.hash)
                .trim()
                .to_string()
        })
    }
}

failure!(ErrorInvIdxHash);

#[renderer(buffer)]
pub fn render_error_inv_idx_hash(error: ErrorInvIdxHash, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    let malformed = t!("inv_idx.err_bad_hash_help").trim().to_string();
    r_eprintln!("{}", help_line!(error.miss.help(&malformed)));
    ec.exit_code = EC_ERR_INV_IDX_ARGUMENT;
}

/// Error: nothing is stored under the key that was asked about.
#[derive(Grouped)]
pub struct ErrorInvIdxNotFound {
    /// The key nothing is stored under, as hex.
    hash: String,
}

impl Failure for ErrorInvIdxNotFound {
    fn name(&self) -> &'static str {
        "error_inv_idx_not_found"
    }

    fn reason(&self) -> String {
        t!("inv_idx.err_not_found", hash = self.hash)
            .trim()
            .to_string()
    }
}

failure!(ErrorInvIdxNotFound);

#[renderer(buffer)]
pub fn render_error_inv_idx_not_found(error: ErrorInvIdxNotFound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("inv_idx.err_not_found_help").trim()));
    ec.exit_code = EC_ERR_INV_IDX_NOT_FOUND;
}

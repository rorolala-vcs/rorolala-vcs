//! The `rola storage extract-file` command: take content back out of the store.
//!
//! It is the other half of [`storage_write_file`](crate::storage::cmd_storage_write_file): the hash that
//! command printed is the whole of what is needed to ask for the content back, and what comes
//! back is the content byte for byte.

use std::fs;
use std::path::PathBuf;

use librorolala::storage::{Error as StorageError, Key, StorageBackend as _};
use mingling::{
    Grouped, LazyRes,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResRorolalaStorage;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_STORAGE_EXTRACT_FILE_ARGUMENT, EC_ERR_STORAGE_EXTRACT_FILE_BAD_HASH,
    EC_ERR_STORAGE_EXTRACT_FILE_EXISTS, EC_ERR_STORAGE_EXTRACT_FILE_FAILED,
    EC_ERR_STORAGE_EXTRACT_FILE_NO_STORAGE, EC_HELP,
};
use crate::failure::failure;

#[help(buffer)]
pub fn help_storage_extract_file(_: EntryStorageExtractFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage_extract_file.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorageExtractFile)]
pub fn desc_storage_extract_file() -> Description {
    t!("storage_extract_file.cmd_storage_extract_file_description")
        .to_string()
        .into()
}

/// Takes the content stored under `HASH` out of the store, into `DIR`.
///
/// `HASH` is what [`rola storage write-file`](crate::storage::cmd_storage_write_file) printed: written
/// as a digest in hex, with or without the hash's name in front of it. The content is written
/// under that digest, in `DIR` — or in the current directory when none is named.
///
/// `DIR` is made if it is not there yet, so a path into somewhere new is a request to write
/// there rather than a mistake. A file already under the name — the same content written twice
/// into one place — is *not* written over: it is reported, so that what is there is never lost
/// to what was asked for.
///
/// # Errors
///
/// Renders [`ErrorHashMissing`] when no hash was named, [`ErrorExtractNoStorage`] when the run is
/// nowhere a store is, [`ErrorBadHash`] when what was named does not read as a hash,
/// [`ErrorTargetExists`] when the file is already there, and [`ErrorExtractFailed`] when the
/// store cannot produce the content or nothing could be written where it was asked for.
#[command(node = "storage.extract-file")]
pub fn storage_extract_file(args: EntryStorageExtractFile) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || ErrorHashMissing.into())
        .pick(&arg![Option<PathBuf>])
        .to_result();
    let (hash, dir) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateStorageExtractFile { hash, dir }.into()
}

/// The state of taking content back out of the store.
///
/// What is asked for is a hash, and where it is asked to land is a directory — or the current
/// one, when none is named.
#[derive(Grouped)]
pub struct StateStorageExtractFile {
    /// The hash whose content is asked for.
    hash: String,
    /// The directory the content goes into, or nothing when the current one is meant.
    dir: Option<PathBuf>,
}

#[chain(routeify)]
pub fn handle_storage_extract_file(
    state: StateStorageExtractFile,
    storage: &mut LazyRes<ResRorolalaStorage>,
) -> Next {
    let StateStorageExtractFile { hash, dir } = state;

    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorExtractNoStorage.into();
    };

    let Ok(key) = hash.parse::<Key>() else {
        return ErrorBadHash { hash }.into();
    };

    // The content goes into the directory named, or the current one: what is written is the
    // digest, which is the name the content is known by wherever it is.
    let dir = dir.unwrap_or_else(|| PathBuf::from("."));
    let path = dir.join(key.hex());

    // A file already under the name is not something to write over: the same content written
    // twice into one place is a mistake worth being told about, not a silent replacement.
    if path.exists() {
        return ErrorTargetExists { path }.into();
    }

    // A directory that is not there yet is made rather than complained about: naming a path into
    // somewhere new is a request to write there.
    if let Err(error) = fs::create_dir_all(&dir) {
        return ErrorExtractFailed {
            hash,
            cause: error.to_string(),
        }
        .into();
    }

    // The store is asynchronous and a command is not, so the two meet here — see `storage_write_file`.
    let extracted = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime.block_on(store.extract_file(&key, &path)),
        Err(error) => Err(StorageError::Io(error)),
    };

    match extracted {
        Ok(()) => ResultExtracted { path }.into(),
        Err(error) => ErrorExtractFailed {
            hash,
            cause: error.to_string(),
        }
        .into(),
    }
}

/// Result: the content was written out.
#[derive(Grouped)]
pub struct ResultExtracted {
    /// Where the content was written.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_result_extracted(result: ResultExtracted) {
    r_println!("{}", result.path.display());
}

/// Error: no hash was named.
#[derive(Grouped)]
pub struct ErrorHashMissing;

impl Failure for ErrorHashMissing {
    fn name(&self) -> &'static str {
        "error_hash_missing"
    }

    fn reason(&self) -> String {
        t!("storage_extract_file.err_hash_missing")
            .trim()
            .to_string()
    }
}

failure!(ErrorHashMissing);

#[renderer(buffer)]
pub fn render_error_hash_missing(error: ErrorHashMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_extract_file.err_hash_missing_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_EXTRACT_FILE_ARGUMENT;
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorExtractNoStorage;

impl Failure for ErrorExtractNoStorage {
    fn name(&self) -> &'static str {
        "error_extract_no_storage"
    }

    fn reason(&self) -> String {
        t!("storage_extract_file.err_no_storage").trim().to_string()
    }
}

failure!(ErrorExtractNoStorage);

#[renderer(buffer)]
pub fn render_error_extract_no_storage(error: ErrorExtractNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_extract_file.err_no_storage_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_EXTRACT_FILE_NO_STORAGE;
}

/// Error: what was named does not read as a hash.
#[derive(Grouped)]
pub struct ErrorBadHash {
    /// What was named.
    hash: String,
}

impl Failure for ErrorBadHash {
    fn name(&self) -> &'static str {
        "error_bad_hash"
    }

    fn reason(&self) -> String {
        t!("storage_extract_file.err_bad_hash", hash = self.hash)
            .trim()
            .to_string()
    }
}

failure!(ErrorBadHash);

#[renderer(buffer)]
pub fn render_error_bad_hash(err: ErrorBadHash, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(err.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_extract_file.err_bad_hash_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_EXTRACT_FILE_BAD_HASH;
}

/// Error: a file is already under the name the content would be written as.
#[derive(Grouped)]
pub struct ErrorTargetExists {
    /// The file that is already there.
    path: PathBuf,
}

impl Failure for ErrorTargetExists {
    fn name(&self) -> &'static str {
        "error_target_exists"
    }

    fn reason(&self) -> String {
        t!(
            "storage_extract_file.err_target_exists",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorTargetExists);

#[renderer(buffer)]
pub fn render_error_target_exists(err: ErrorTargetExists, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(err.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_extract_file.err_target_exists_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_EXTRACT_FILE_EXISTS;
}

/// Error: the content could not be produced, or could not be written where it was asked for.
#[derive(Grouped)]
pub struct ErrorExtractFailed {
    /// The hash whose content was being asked for.
    hash: String,
    /// Why it could not be produced or written.
    cause: String,
}

impl Failure for ErrorExtractFailed {
    fn name(&self) -> &'static str {
        "error_extract_failed"
    }

    fn reason(&self) -> String {
        t!(
            "storage_extract_file.err_extract_failed",
            hash = self.hash,
            reason = self.cause
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorExtractFailed);

#[renderer(buffer)]
pub fn render_error_extract_failed(err: ErrorExtractFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(err.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_extract_file.err_extract_failed_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_EXTRACT_FILE_FAILED;
}

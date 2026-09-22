//! The `rola tool-extract-file` command: take content back out of the store.
//!
//! It is the other half of [`tool_write_file`](crate::tools::cmd_tool_write_file): the hash that
//! command printed is the whole of what is needed to ask for the content back, and what comes
//! back is the content byte for byte.

use std::fs;
use std::path::PathBuf;

use librorolala::storage::{Error as StorageError, Key, StorageBackend as _};
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResRorolalaStorage;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_TOOL_EXTRACT_FILE_ARGUMENT, EC_ERR_TOOL_EXTRACT_FILE_BAD_HASH,
    EC_ERR_TOOL_EXTRACT_FILE_EXISTS, EC_ERR_TOOL_EXTRACT_FILE_FAILED,
    EC_ERR_TOOL_EXTRACT_FILE_NO_STORAGE, EC_HELP,
};

#[help(buffer)]
pub fn help_tool_extract_file(_: EntryToolExtractFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool_extract_file.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryToolExtractFile)]
pub fn desc_tool_extract_file() -> Description {
    t!("tool_extract_file.cmd_tool_extract_file_description")
        .to_string()
        .into()
}

/// Takes the content stored under `HASH` out of the store, into `DIR`.
///
/// `HASH` is what [`rola tool-write-file`](crate::tools::cmd_tool_write_file) printed: written
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
#[command(node = "tool-extract-file")]
pub fn tool_extract_file(
    args: EntryToolExtractFile,
    storage: &mut LazyRes<ResRorolalaStorage>,
) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || ErrorHashMissing.into())
        .pick(&arg![Option<PathBuf>])
        .to_result();
    let (hash, dir) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

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
            reason: error.to_string(),
        }
        .into();
    }

    // The store is asynchronous and a command is not, so the two meet here — see `tool_write_file`.
    let extracted = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime.block_on(store.extract_file(&key, &path)),
        Err(error) => Err(StorageError::Io(error)),
    };

    match extracted {
        Ok(()) => ResultExtracted { path }.into(),
        Err(error) => ErrorExtractFailed {
            hash,
            reason: error.to_string(),
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

#[renderer(buffer)]
pub fn render_error_hash_missing(_: ErrorHashMissing, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("tool_extract_file.err_hash_missing").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_extract_file.err_hash_missing_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_EXTRACT_FILE_ARGUMENT;
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorExtractNoStorage;

#[renderer(buffer)]
pub fn render_error_extract_no_storage(_: ErrorExtractNoStorage, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("tool_extract_file.err_no_storage").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_extract_file.err_no_storage_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_EXTRACT_FILE_NO_STORAGE;
}

/// Error: what was named does not read as a hash.
#[derive(Grouped)]
pub struct ErrorBadHash {
    /// What was named.
    hash: String,
}

#[renderer(buffer)]
pub fn render_error_bad_hash(err: ErrorBadHash, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("tool_extract_file.err_bad_hash", hash = err.hash).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_extract_file.err_bad_hash_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_EXTRACT_FILE_BAD_HASH;
}

/// Error: a file is already under the name the content would be written as.
#[derive(Grouped)]
pub struct ErrorTargetExists {
    /// The file that is already there.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_error_target_exists(err: ErrorTargetExists, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!(
            "tool_extract_file.err_target_exists",
            path = err.path.display()
        ))
        .trim()
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_extract_file.err_target_exists_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_EXTRACT_FILE_EXISTS;
}

/// Error: the content could not be produced, or could not be written where it was asked for.
#[derive(Grouped)]
pub struct ErrorExtractFailed {
    /// The hash whose content was being asked for.
    hash: String,
    /// Why it could not be produced or written.
    reason: String,
}

#[renderer(buffer)]
pub fn render_error_extract_failed(err: ErrorExtractFailed, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(
            t!(
                "tool_extract_file.err_extract_failed",
                hash = err.hash,
                reason = err.reason
            )
            .trim()
        )
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_extract_file.err_extract_failed_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_EXTRACT_FILE_FAILED;
}

//! The `rola storage write-file` command: put a file's content into the store.
//!
//! It is a write with nothing else around it: a path is named, its content goes into the store a
//! run works on, and the hash it is kept under is printed. What that hash is worth printing is
//! that it is the whole of what is needed to ask for the content back — see
//! [`storage_extract_file`](crate::storage::cmd_storage_extract_file).

use std::path::PathBuf;

use librorolala::storage::{Key, store_file};
use mingling::{
    Grouped, LazyRes, StructuralData, Wrap,
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
use serde::Serialize;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_STORAGE_WRITE_FILE_ARGUMENT, EC_ERR_STORAGE_WRITE_FILE_FAILED,
    EC_ERR_STORAGE_WRITE_FILE_NO_STORAGE, EC_ERR_STORAGE_WRITE_FILE_NOT_A_FILE, EC_HELP,
};
use crate::failure::failure;

#[help(buffer)]
pub fn help_storage_write_file(_: EntryStorageWriteFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage_write_file.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorageWriteFile)]
pub fn desc_storage_write_file() -> Description {
    t!("storage_write_file.cmd_storage_write_file_description")
        .to_string()
        .into()
}

/// Stores the content of `FILE`, and prints the hash it is kept under.
///
/// `FILE` names a file, not a directory: what is stored is content, and a directory is not
/// content. How the content is kept — whether it is compressed, whether it is cut into chunks —
/// is the store's own decision, made from the content itself, so what a caller says is a path
/// and nothing else.
///
/// What is printed is the hash, on a line of its own: the same hash `rola storage extract-file`
/// takes, so what one command answers is what the other is asked. Content the store cuts is named
/// as `manifest:<digest>` rather than `blake3:<digest>`, since how it is kept is part of what the
/// line says; either name is read back by `storage extract-file`.
///
/// The run has to be somewhere a store can be found — inside one, or inside a Vault or Workspace
/// that keeps one.
///
/// # Errors
///
/// Renders [`ErrorFileMissing`] when no path was named, [`ErrorWriteNoStorage`] when the run is
/// nowhere a store is, [`ErrorNotAFile`] when the path is not a file, and [`ErrorWriteFailed`]
/// when the store would not take the content.
#[command(node = "storage.write-file")]
pub fn storage_write_file(args: EntryStorageWriteFile) -> Next {
    let picked = args
        .pick_or_route(&arg![PathBuf], || ErrorFileMissing.into())
        .to_result();
    let file = match picked {
        Ok(file) => file,
        Err(next) => return next,
    };

    StateStorageWriteFile::from(file).into()
}

/// The state of storing a file's content.
///
/// A path is the whole of what a write is told: how the content is kept is the store's own
/// decision, made from the content itself.
#[derive(Grouped, Wrap)]
pub struct StateStorageWriteFile {
    /// The file whose content is stored.
    path: PathBuf,
}

#[chain(routeify)]
pub fn handle_storage_write_file(
    state: StateStorageWriteFile,
    storage: &mut LazyRes<ResRorolalaStorage>,
) -> Next {
    let file = state.path;

    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorWriteNoStorage.into();
    };

    // What is stored is a file's content, so a path that is not a file — one that is not there,
    // or one that names a directory — is not something there is content to take.
    if !file.is_file() {
        return ErrorNotAFile { path: file }.into();
    }

    // The store is asynchronous and a command is not, so the two meet here: the content is read,
    // encoded, cut and written by the store, and a runtime of this run's own is what waits for it.
    // The same runtime is kept for the look at how the content was kept, so the write and that look
    // meet one store rather than two.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorWriteFailed {
                path: file,
                cause: error.to_string(),
            }
            .into();
        }
    };

    let hash = match runtime.block_on(store_file(store, &file)) {
        Ok(hash) => hash,
        Err(error) => {
            return ErrorWriteFailed {
                path: file,
                cause: error.to_string(),
            }
            .into();
        }
    };

    // Whether the content was cut is not something the hash says, so the store is asked: content kept
    // as a manifest of chunks is named as one, so that how it is kept is part of what was answered.
    match runtime.block_on(store.holds_manifest(&hash)) {
        Ok(chunked) => ResultBlake3Hash { hash, chunked }.into(),
        Err(error) => ErrorWriteFailed {
            path: file,
            cause: error.to_string(),
        }
        .into(),
    }
}

/// Result: the content was stored, under the hash it is named by.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultBlake3Hash {
    /// The hash the content is kept under.
    hash: Key,
    /// Whether the content is kept as a manifest of chunks rather than one object.
    chunked: bool,
}

#[renderer(buffer)]
pub fn render_result_blake3_hash(result: ResultBlake3Hash) {
    // What is printed says how the content is kept as well as what it is: content cut into chunks is
    // named as a manifest, so a reader can tell the two apart at a glance. A key is the same key
    // either way, so `storage extract-file` reads either name back.
    if result.chunked {
        r_println!("manifest:{}", result.hash.hex());
    } else {
        r_println!("{}", result.hash);
    }
}

/// Error: no path was named.
#[derive(Grouped)]
pub struct ErrorFileMissing;

impl Failure for ErrorFileMissing {
    fn name(&self) -> &'static str {
        "error_file_missing"
    }

    fn reason(&self) -> String {
        t!("storage_write_file.err_file_missing").trim().to_string()
    }
}

failure!(ErrorFileMissing);

#[renderer(buffer)]
pub fn render_error_file_missing(error: ErrorFileMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_write_file.err_file_missing_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_WRITE_FILE_ARGUMENT;
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorWriteNoStorage;

impl Failure for ErrorWriteNoStorage {
    fn name(&self) -> &'static str {
        "error_write_no_storage"
    }

    fn reason(&self) -> String {
        t!("storage_write_file.err_no_storage").trim().to_string()
    }
}

failure!(ErrorWriteNoStorage);

#[renderer(buffer)]
pub fn render_error_write_no_storage(error: ErrorWriteNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_write_file.err_no_storage_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_WRITE_FILE_NO_STORAGE;
}

/// Error: the path named is not a file.
#[derive(Grouped)]
pub struct ErrorNotAFile {
    /// The path that is not a file.
    path: PathBuf,
}

impl Failure for ErrorNotAFile {
    fn name(&self) -> &'static str {
        "error_not_a_file"
    }

    fn reason(&self) -> String {
        t!(
            "storage_write_file.err_not_a_file",
            path = self.path.display().to_string()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorNotAFile);

#[renderer(buffer)]
pub fn render_error_not_a_file(err: ErrorNotAFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(err.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_write_file.err_not_a_file_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_WRITE_FILE_NOT_A_FILE;
}

/// Error: the store would not take the content.
#[derive(Grouped)]
pub struct ErrorWriteFailed {
    /// The file whose content was being stored.
    path: PathBuf,
    /// Why the store would not take it.
    cause: String,
}

impl Failure for ErrorWriteFailed {
    fn name(&self) -> &'static str {
        "error_write_failed"
    }

    fn reason(&self) -> String {
        t!(
            "storage_write_file.err_write_failed",
            path = self.path.display().to_string(),
            reason = self.cause
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorWriteFailed);

#[renderer(buffer)]
pub fn render_error_write_failed(err: ErrorWriteFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(err.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_write_file.err_write_failed_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_WRITE_FILE_FAILED;
}

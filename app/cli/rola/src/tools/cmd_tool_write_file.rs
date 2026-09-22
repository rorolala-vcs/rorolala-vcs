//! The `rola tool-write-file` command: put a file's content into the store.
//!
//! It is a write with nothing else around it: a path is named, its content goes into the store a
//! run works on, and the hash it is kept under is printed. What that hash is worth printing is
//! that it is the whole of what is needed to ask for the content back — see
//! [`tool_extract_file`](crate::tools::cmd_tool_extract_file).

use std::path::PathBuf;

use librorolala::storage::{Key, store_file};
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
    EC_ERR_TOOL_WRITE_FILE_ARGUMENT, EC_ERR_TOOL_WRITE_FILE_FAILED,
    EC_ERR_TOOL_WRITE_FILE_NO_STORAGE, EC_ERR_TOOL_WRITE_FILE_NOT_A_FILE, EC_HELP,
};

#[help(buffer)]
pub fn help_tool_write_file(_: EntryToolWriteFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool_write_file.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryToolWriteFile)]
pub fn desc_tool_write_file() -> Description {
    t!("tool_write_file.cmd_tool_write_file_description")
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
/// What is printed is the hash, on a line of its own: the same hash `rola tool-extract-file`
/// takes, so what one command answers is what the other is asked. Content the store cuts is named
/// as `manifest:<digest>` rather than `blake3:<digest>`, since how it is kept is part of what the
/// line says; either name is read back by `tool-extract-file`.
///
/// The run has to be somewhere a store can be found — inside one, or inside a Vault or Workspace
/// that keeps one.
///
/// # Errors
///
/// Renders [`ErrorFileMissing`] when no path was named, [`ErrorWriteNoStorage`] when the run is
/// nowhere a store is, [`ErrorNotAFile`] when the path is not a file, and [`ErrorWriteFailed`]
/// when the store would not take the content.
#[command(node = "tool-write-file")]
pub fn tool_write_file(
    args: EntryToolWriteFile,
    storage: &mut LazyRes<ResRorolalaStorage>,
) -> Next {
    let picked = args
        .pick_or_route(&arg![PathBuf], || ErrorFileMissing.into())
        .to_result();
    let file = match picked {
        Ok(file) => file,
        Err(next) => return next,
    };

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
                reason: error.to_string(),
            }
            .into();
        }
    };

    let hash = match runtime.block_on(store_file(store, &file)) {
        Ok(hash) => hash,
        Err(error) => {
            return ErrorWriteFailed {
                path: file,
                reason: error.to_string(),
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
            reason: error.to_string(),
        }
        .into(),
    }
}

/// Result: the content was stored, under the hash it is named by.
#[derive(Grouped)]
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
    // either way, so `tool-extract-file` reads either name back.
    if result.chunked {
        r_println!("manifest:{}", result.hash.hex());
    } else {
        r_println!("{}", result.hash);
    }
}

/// Error: no path was named.
#[derive(Grouped)]
pub struct ErrorFileMissing;

#[renderer(buffer)]
pub fn render_error_file_missing(_: ErrorFileMissing, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("tool_write_file.err_file_missing").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_write_file.err_file_missing_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_WRITE_FILE_ARGUMENT;
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorWriteNoStorage;

#[renderer(buffer)]
pub fn render_error_write_no_storage(_: ErrorWriteNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tool_write_file.err_no_storage").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_write_file.err_no_storage_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_WRITE_FILE_NO_STORAGE;
}

/// Error: the path named is not a file.
#[derive(Grouped)]
pub struct ErrorNotAFile {
    /// The path that is not a file.
    path: PathBuf,
}

#[renderer(buffer)]
pub fn render_error_not_a_file(err: ErrorNotAFile, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!(
            "tool_write_file.err_not_a_file",
            path = err.path.display()
        ))
        .trim()
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_write_file.err_not_a_file_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_WRITE_FILE_NOT_A_FILE;
}

/// Error: the store would not take the content.
#[derive(Grouped)]
pub struct ErrorWriteFailed {
    /// The file whose content was being stored.
    path: PathBuf,
    /// Why the store would not take it.
    reason: String,
}

#[renderer(buffer)]
pub fn render_error_write_failed(err: ErrorWriteFailed, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!(
            "tool_write_file.err_write_failed",
            path = err.path.display().to_string(),
            reason = err.reason
        ))
        .trim()
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_write_file.err_write_failed_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_WRITE_FILE_FAILED;
}

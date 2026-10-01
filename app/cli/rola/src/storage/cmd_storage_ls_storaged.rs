//! The `rola storage ls-storaged` command: list the objects a store holds.
//!
//! It is the listing half of the store: what [`storage_write_file`](crate::storage::cmd_storage_write_file)
//! puts in and [`storage_extract_file`](crate::storage::cmd_storage_extract_file) takes out is named here,
//! one key per line. What is printed is exactly what those two speak in, so a line read here is a
//! hash to hand to `storage extract-file`.

use librorolala::storage::{Key, StorageBackend as _};
use mingling::{
    Grouped, LazyRes, StructuralData, Suggest,
    macros::{
        buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, routeify, suggest,
    },
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResRorolalaStorage;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_FORMAT, EC_ERR_STORAGE_LS_STORAGED_FAILED, EC_ERR_STORAGE_LS_STORAGED_NO_STORAGE,
    EC_HELP,
};
use crate::failure::failure;
use crate::format::ResFormat;

/// How a listing of the store's objects is drawn when no template is named.
///
/// One key a line, which is what the command has always printed: the template is the same shape
/// `--json` writes, so `keys` is the one field of the result.
pub const DEFAULT_FORMAT: &str = "{{ keys }}";

#[help(buffer)]
pub fn help_storage_ls_storaged(_: EntryStorageLsStoraged, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("storage_ls_storaged.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStorageLsStoraged)]
pub fn desc_storage_ls_storaged() -> Description {
    t!("storage_ls_storaged.cmd_storage_ls_storaged_description")
        .to_string()
        .into()
}

/// Completes what `rola storage ls-storaged` can be given next.
///
/// The listing names nothing, so there is nothing to offer.
#[completion(EntryStorageLsStoraged)]
pub fn complete_storage_ls_storaged() -> Suggest {
    suggest!()
}

/// Lists every object the store holds, one key per line.
///
/// What is printed is the keys the store answers for — the same keys
/// [`rola storage extract-file`](crate::storage::cmd_storage_extract_file) takes — so a line read here is
/// a hash to hand to that command. Only what the store *holds* is listed: a manifest whose chunks
/// have gone is not named, since it is not something there is content to read.
///
/// The run has to be somewhere a store can be found — inside one, or inside a Vault or Workspace
/// that keeps one.
///
/// # Errors
///
/// Renders [`ErrorLsNoStorage`] when the run is nowhere a store is, and [`ErrorLsFailed`] when the
/// store's objects could not be listed.
#[command(node = "storage.ls-storaged")]
pub fn storage_ls_storaged(format: &mut ResFormat) -> StateStorageLsStoraged {
    // The default is named as the command is reached, so it holds even when the listing then fails
    // and the failure is what is drawn. A run that asked for `--json` keeps it: the default is left
    // unwritten and the framework's own output answers.
    format.default_template(DEFAULT_FORMAT);
    StateStorageLsStoraged
}

/// The state a listing of the store's objects starts in.
///
/// A listing names nothing: what is listed is whatever the run's store holds.
#[derive(Grouped)]
pub struct StateStorageLsStoraged;

#[chain(routeify)]
pub fn handle_storage_ls_storaged(
    _state: StateStorageLsStoraged,
    storage: &mut LazyRes<ResRorolalaStorage>,
    format: &mut ResFormat,
) -> Next {
    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorLsNoStorage.into();
    };

    // The store is asynchronous and a command is not, so the two meet here — see
    // `cmd_storage_write_file`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorLsFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    match runtime.block_on(store.list_exist_keys()) {
        Ok(keys) => {
            // Published under the name `--json` writes the list by, so a template and a JSON
            // reader name it the same way.
            format.set(
                "keys",
                keys.iter().map(|key| serde_json::json!(key)).collect(),
            );
            ResultLs { keys }.into()
        }
        Err(error) => ErrorLsFailed {
            cause: error.to_string(),
        }
        .into(),
    }
}

/// Result: what the store holds was listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLs {
    /// The keys the store holds.
    pub(crate) keys: Vec<Key>,
}

#[renderer(buffer)]
pub fn render_result_ls(result: ResultLs, format: &ResFormat, ec: &mut ResExitCode) {
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
        for key in &result.keys {
            r_println!("{key}");
        }
    }
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorLsNoStorage;

impl Failure for ErrorLsNoStorage {
    fn name(&self) -> &'static str {
        "error_ls_no_storage"
    }

    fn reason(&self) -> String {
        t!("storage_ls_storaged.err_no_storage").trim().to_string()
    }
}

failure!(ErrorLsNoStorage);

#[renderer(buffer)]
pub fn render_error_ls_no_storage(error: ErrorLsNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_ls_storaged.err_no_storage_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_LS_STORAGED_NO_STORAGE;
}

/// Error: the store's objects could not be listed.
#[derive(Grouped)]
pub struct ErrorLsFailed {
    /// Why the store would not list.
    cause: String,
}

impl Failure for ErrorLsFailed {
    fn name(&self) -> &'static str {
        "error_ls_failed"
    }

    fn reason(&self) -> String {
        t!("storage_ls_storaged.err_ls_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorLsFailed);

#[renderer(buffer)]
pub fn render_error_ls_failed(error: ErrorLsFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("storage_ls_storaged.err_ls_failed_help").trim())
    );
    ec.exit_code = EC_ERR_STORAGE_LS_STORAGED_FAILED;
}

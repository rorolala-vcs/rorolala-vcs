//! The `rola tool ls-storaged` command: list the objects a store holds.
//!
//! It is the listing half of the tools: what [`tool_write_file`](crate::tools::cmd_tool_write_file)
//! puts in and [`tool_extract_file`](crate::tools::cmd_tool_extract_file) takes out is named here,
//! one key per line. What is printed is exactly what those two speak in, so a line read here is a
//! hash to hand to `tool extract-file`.

use librorolala::storage::{Key, StorageBackend as _};
use mingling::{
    Grouped, LazyRes,
    macros::{buffer, command, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResRorolalaStorage;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_TOOL_LS_STORAGED_FAILED, EC_ERR_TOOL_LS_STORAGED_NO_STORAGE, EC_HELP,
};

#[help(buffer)]
pub fn help_tool_ls_storaged(_: EntryToolLsStoraged, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool_ls_storaged.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryToolLsStoraged)]
pub fn desc_tool_ls_storaged() -> Description {
    t!("tool_ls_storaged.cmd_tool_ls_storaged_description")
        .to_string()
        .into()
}

/// Lists every object the store holds, one key per line.
///
/// What is printed is the keys the store answers for — the same keys
/// [`rola tool extract-file`](crate::tools::cmd_tool_extract_file) takes — so a line read here is
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
#[command(node = "tool.ls-storaged")]
pub fn tool_ls_storaged(storage: &mut LazyRes<ResRorolalaStorage>) -> Next {
    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorLsNoStorage.into();
    };

    // The store is asynchronous and a command is not, so the two meet here — see
    // `cmd_tool_write_file`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorLsFailed {
                reason: error.to_string(),
            }
            .into();
        }
    };

    match runtime.block_on(store.list_exist_keys()) {
        Ok(keys) => ResultLs { keys }.into(),
        Err(error) => ErrorLsFailed {
            reason: error.to_string(),
        }
        .into(),
    }
}

/// Result: what the store holds was listed.
#[derive(Grouped)]
pub struct ResultLs {
    /// The keys the store holds.
    keys: Vec<Key>,
}

#[renderer(buffer)]
pub fn render_result_ls(result: ResultLs) {
    for key in &result.keys {
        r_println!("{key}");
    }
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorLsNoStorage;

#[renderer(buffer)]
pub fn render_error_ls_no_storage(_: ErrorLsNoStorage, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("tool_ls_storaged.err_no_storage").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_ls_storaged.err_no_storage_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_LS_STORAGED_NO_STORAGE;
}

/// Error: the store's objects could not be listed.
#[derive(Grouped)]
pub struct ErrorLsFailed {
    /// Why the store would not list.
    reason: String,
}

#[renderer(buffer)]
pub fn render_error_ls_failed(error: ErrorLsFailed, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("tool_ls_storaged.err_ls_failed", reason = error.reason).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_ls_storaged.err_ls_failed_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_LS_STORAGED_FAILED;
}

//! The `rola tool ls-manifests` command: list the content the store keeps cut.
//!
//! It is the listing for what [`tool_write_file`](crate::tools::cmd_tool_write_file) names as a
//! manifest: where that command says how one content was kept, this says which contents were kept
//! that way. What is printed is named `manifest:<digest>`, so a line read here is what
//! [`tool_extract_file`](crate::tools::cmd_tool_extract_file) takes back.

use librorolala::storage::Key;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify},
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
    EC_ERR_TOOL_LS_MANIFESTS_FAILED, EC_ERR_TOOL_LS_MANIFESTS_NO_STORAGE, EC_HELP,
};
use crate::failure::failure;

#[help(buffer)]
pub fn help_tool_ls_manifests(_: EntryToolLsManifests, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool_ls_manifests.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryToolLsManifests)]
pub fn desc_tool_ls_manifests() -> Description {
    t!("tool_ls_manifests.cmd_tool_ls_manifests_description")
        .to_string()
        .into()
}

/// Lists every content the store keeps as a manifest of chunks, one key per line.
///
/// A manifest is what a cut content has; a whole one has none. What is printed is named
/// `manifest:<digest>` — the way [`rola tool write-file`](crate::tools::cmd_tool_write_file) names
/// content it cut — and the same name is read back by
/// [`rola tool extract-file`](crate::tools::cmd_tool_extract_file), so a line read here can be
/// handed straight to it. A manifest whose chunks have gone is still listed: it is still what the
/// store was told to cut, and what is missing is found on the read that asks for it.
///
/// The run has to be somewhere a store can be found — inside one, or inside a Vault or Workspace
/// that keeps one.
///
/// # Errors
///
/// Renders [`ErrorManifestsNoStorage`] when the run is nowhere a store is, and
/// [`ErrorManifestsFailed`] when the store's manifests could not be listed.
#[command(node = "tool.ls-manifests")]
pub fn tool_ls_manifests() -> StateToolLsManifests {
    StateToolLsManifests
}

/// The state a listing of the store's manifests starts in.
///
/// A listing names nothing: what is listed is whatever the run's store keeps cut.
#[derive(Grouped)]
pub struct StateToolLsManifests;

#[chain(routeify)]
pub fn handle_tool_ls_manifests(
    _state: StateToolLsManifests,
    storage: &mut LazyRes<ResRorolalaStorage>,
) -> Next {
    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorManifestsNoStorage.into();
    };

    // The store is asynchronous and a command is not, so the two meet here — see
    // `cmd_tool_write_file`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorManifestsFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    match runtime.block_on(store.list_manifest_keys()) {
        Ok(keys) => ResultManifests { keys }.into(),
        Err(error) => ErrorManifestsFailed {
            cause: error.to_string(),
        }
        .into(),
    }
}

/// Result: the manifests the store keeps were listed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultManifests {
    /// The keys whose content is kept as a manifest.
    keys: Vec<Key>,
}

#[renderer(buffer)]
pub fn render_result_manifests(result: ResultManifests) {
    // A key is written as the name the content is kept under, `manifest:`, so that a listing says
    // both what the content is and how it is kept.
    for key in &result.keys {
        r_println!("manifest:{}", key.hex());
    }
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorManifestsNoStorage;

impl Failure for ErrorManifestsNoStorage {
    fn name(&self) -> &'static str {
        "error_manifests_no_storage"
    }

    fn reason(&self) -> String {
        t!("tool_ls_manifests.err_no_storage").trim().to_string()
    }
}

failure!(ErrorManifestsNoStorage);

#[renderer(buffer)]
pub fn render_error_manifests_no_storage(error: ErrorManifestsNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_ls_manifests.err_no_storage_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_LS_MANIFESTS_NO_STORAGE;
}

/// Error: the store's manifests could not be listed.
#[derive(Grouped)]
pub struct ErrorManifestsFailed {
    /// Why the store would not list.
    cause: String,
}

impl Failure for ErrorManifestsFailed {
    fn name(&self) -> &'static str {
        "error_manifests_failed"
    }

    fn reason(&self) -> String {
        t!("tool_ls_manifests.err_ls_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorManifestsFailed);

#[renderer(buffer)]
pub fn render_error_manifests_failed(error: ErrorManifestsFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_ls_manifests.err_ls_failed_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_LS_MANIFESTS_FAILED;
}

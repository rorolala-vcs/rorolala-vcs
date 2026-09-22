//! The `rola pack` command: gather a store's loose objects into a pack.
//!
//! It is packing on its own, with nothing else around it: every object the store in hand holds is
//! looked at, the ones still sitting loose are gathered into a pack, and what is answered is
//! whether that changed anything. What each object *is* does not change — a pack holds the entries
//! exactly as they sat loose — so this is a way to keep the same objects in fewer files, not a
//! step a reader has to know happened.

use librorolala::storage::StorageBackend as _;
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
use crate::exit_codes::{EC_ERR_PACK_FAILED, EC_ERR_PACK_NO_STORAGE, EC_HELP};

#[help(buffer)]
pub fn help_pack(_: EntryPack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("pack.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryPack)]
pub fn desc_pack() -> Description {
    t!("pack.cmd_pack_description").to_string().into()
}

/// Gathers the store's loose objects into a pack.
///
/// Every object a store holds sits on its own until it is packed. This looks at all of them and
/// gathers the ones still loose into a pack — one file with an index beside it — so that fewer
/// files carry the same objects. What each object *is* does not change: a pack holds its entries
/// exactly as they sat loose, so every read that worked before works after.
///
/// An object a pack already names is left where it is, so packing a store that is already packed
/// changes nothing; so does packing one with nothing loose.
///
/// The run has to be somewhere a store can be found — inside one, or inside a Vault or Workspace
/// that keeps one.
///
/// # Errors
///
/// Renders [`ErrorPackNoStorage`] when the run is nowhere a store is, and [`ErrorPackFailed`] when
/// the store's objects could not be read or the pack could not be written.
#[command(node = "pack")]
pub fn pack(storage: &mut LazyRes<ResRorolalaStorage>) -> Next {
    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorPackNoStorage.into();
    };

    // The store is asynchronous and a command is not, so the two meet here: the keys are read and
    // the pack written by the store, and a runtime of this run's own is what waits for it — see
    // `cmd_tool_write_file`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorPackFailed {
                reason: error.to_string(),
            }
            .into();
        }
    };

    // Which objects there are to pack is the store's to say: every key it holds is handed over,
    // and packing passes over the ones a pack already names, so what is loose is what is packed.
    let keys = match runtime.block_on(store.list_exist_keys()) {
        Ok(keys) => keys,
        Err(error) => {
            return ErrorPackFailed {
                reason: error.to_string(),
            }
            .into();
        }
    };

    match runtime.block_on(store.pack(&keys)) {
        Ok(packed) => ResultPacked { packed }.into(),
        Err(error) => ErrorPackFailed {
            reason: error.to_string(),
        }
        .into(),
    }
}

/// Result: the store was looked at, and answered whether a pack was made.
#[derive(Grouped)]
pub struct ResultPacked {
    /// Whether anything was loose to pack.
    packed: bool,
}

#[renderer(buffer)]
pub fn render_result_packed(result: ResultPacked) {
    if result.packed {
        r_println!("{}", t!("pack.result_packed").trim());
    } else {
        r_println!("{}", t!("pack.result_nothing").trim());
    }
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorPackNoStorage;

#[renderer(buffer)]
pub fn render_error_pack_no_storage(_: ErrorPackNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("pack.err_no_storage").trim()));
    r_eprintln!("{}", help_line!(t!("pack.err_no_storage_help").trim()));
    ec.exit_code = EC_ERR_PACK_NO_STORAGE;
}

/// Error: the store's objects could not be read, or the pack could not be written.
#[derive(Grouped)]
pub struct ErrorPackFailed {
    /// Why the store would not pack.
    reason: String,
}

#[renderer(buffer)]
pub fn render_error_pack_failed(error: ErrorPackFailed, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("pack.err_pack_failed", reason = error.reason).trim())
    );
    r_eprintln!("{}", help_line!(t!("pack.err_pack_failed_help").trim()));
    ec.exit_code = EC_ERR_PACK_FAILED;
}

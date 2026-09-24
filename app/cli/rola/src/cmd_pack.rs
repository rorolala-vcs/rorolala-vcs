//! The `rola pack` command: lay a store's objects out in as few packs as its size limit allows.
//!
//! It is packing on its own, with nothing else around it: everything the store in hand holds — the
//! entries of every pack, and every loose object — is written again, so that what was many packs and
//! a scattering of loose files becomes as few packs as the store's `max_pack_size` allows. What each
//! object *is* does not change — a pack holds the entries exactly as they sat loose — so this is a way
//! to keep the same objects in fewer files, not a step a reader has to know happened.

use librorolala::storage::Lockable as _;
use mingling::{
    Grouped, LazyRes,
    macros::{buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResRorolalaStorage;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{EC_ERR_PACK_FAILED, EC_ERR_PACK_LOCKED, EC_ERR_PACK_NO_STORAGE, EC_HELP};
use crate::failure::failure;

#[help(buffer)]
pub fn help_pack(_: EntryPack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("pack.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryPack)]
pub fn desc_pack() -> Description {
    t!("pack.cmd_pack_description").to_string().into()
}

/// Lays the store's objects out in as few packs as its size limit allows.
///
/// Everything the store holds is written again: the entries of every pack are gathered up with the
/// loose objects, and the whole of it is laid down as one pack — or as several, once one would pass
/// the store's `max_pack_size`, which is `[storage] max_pack_size` in the store's configuration and
/// two gibibytes when that says nothing. What each object *is* does not change: a pack holds its
/// entries exactly as they sat loose, so every read that worked before works after.
///
/// A store already laid out the way this would lay it out is left alone, so running this twice in a
/// row changes nothing the second time. What is printed says which of the two happened.
///
/// The store is locked for the whole of the work, so a run packing one is the only run packing it.
/// A store another run holds the lock of is not waited for — the run is told, and nothing is
/// changed.
///
/// The run has to be somewhere a store can be found — inside one, or inside a Vault or Workspace
/// that keeps one.
///
/// # Errors
///
/// Renders [`ErrorPackNoStorage`] when the run is nowhere a store is, [`ErrorPackLocked`] when the
/// store is locked by another run, and [`ErrorPackFailed`] when the store's objects could not be
/// read or a pack could not be written.
#[command(node = "pack")]
pub fn pack() -> StatePack {
    StatePack
}

/// The state a packing run starts in.
///
/// A packing names nothing: what is laid out is whatever store the run works on, so the whole
/// of what it is told is already in where the run happens to be.
#[derive(Grouped)]
pub struct StatePack;

#[chain(routeify)]
pub fn handle_pack(_state: StatePack, storage: &mut LazyRes<ResRorolalaStorage>) -> Next {
    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorPackNoStorage.into();
    };

    // The store is asynchronous and a command is not, so the two meet here: the objects are read and
    // the packs written by the store, and a runtime of this run's own is what waits for it — see
    // `cmd_storage_write_file`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorPackFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    // Which objects there are and how they are laid out is the store's to say: this hands over
    // nothing and asks for the whole of it to be laid out afresh — under the store's lock, so that
    // nothing else is packing the same store while it happens.
    let guard = match runtime.block_on(store.lock()) {
        Ok(guard) => guard,
        Err(error) => {
            return ErrorPackLocked {
                cause: error.to_string(),
            }
            .into();
        }
    };

    match runtime.block_on(guard.repack()) {
        Ok(changed) => ResultPacked { changed }.into(),
        Err(error) => ErrorPackFailed {
            cause: error.to_string(),
        }
        .into(),
    }
}

/// Result: the store was laid out afresh, or already was.
#[derive(Grouped)]
pub struct ResultPacked {
    /// Whether anything was changed.
    changed: bool,
}

#[renderer(buffer)]
pub fn render_result_packed(result: ResultPacked) {
    if result.changed {
        r_println!("{}", t!("pack.result_packed").trim());
    } else {
        r_println!("{}", t!("pack.result_nothing").trim());
    }
}

/// Error: the run is nowhere a store is.
#[derive(Grouped)]
pub struct ErrorPackNoStorage;

impl Failure for ErrorPackNoStorage {
    fn name(&self) -> &'static str {
        "error_pack_no_storage"
    }

    fn reason(&self) -> String {
        t!("pack.err_no_storage").trim().to_string()
    }
}

failure!(ErrorPackNoStorage);

#[renderer(buffer)]
pub fn render_error_pack_no_storage(error: ErrorPackNoStorage, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_no_storage_help").trim()));
    ec.exit_code = EC_ERR_PACK_NO_STORAGE;
}

/// Error: the store's objects could not be read, or a pack could not be written.
#[derive(Grouped)]
pub struct ErrorPackFailed {
    /// Why the store would not pack.
    cause: String,
}

impl Failure for ErrorPackFailed {
    fn name(&self) -> &'static str {
        "error_pack_failed"
    }

    fn reason(&self) -> String {
        t!("pack.err_pack_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorPackFailed);

#[renderer(buffer)]
pub fn render_error_pack_failed(error: ErrorPackFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_pack_failed_help").trim()));
    ec.exit_code = EC_ERR_PACK_FAILED;
}

/// Error: another run holds the store's lock.
#[derive(Grouped)]
pub struct ErrorPackLocked {
    /// Why the store would not lock.
    cause: String,
}

impl Failure for ErrorPackLocked {
    fn name(&self) -> &'static str {
        "error_pack_locked"
    }

    fn reason(&self) -> String {
        t!("pack.err_pack_locked", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorPackLocked);

#[renderer(buffer)]
pub fn render_error_pack_locked(error: ErrorPackLocked, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_pack_locked_help").trim()));
    ec.exit_code = EC_ERR_PACK_LOCKED;
}

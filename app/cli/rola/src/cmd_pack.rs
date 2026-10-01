//! The `rola pack` command: lay a store's objects out in as few packs as its size limit allows.
//!
//! It is packing on its own, with nothing else around it: everything the store in hand holds — the
//! entries of every pack, and every loose object — is written again, so that what was many packs and
//! a scattering of loose files becomes as few packs as the store's `max_pack_size` allows. What each
//! object *is* does not change — a pack holds the entries exactly as they sat loose — so this is a way
//! to keep the same objects in fewer files, not a step a reader has to know happened.

use librorolala::storage::Lockable as _;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{
    EC_ERR_PACK_FAILED, EC_ERR_PACK_LOCKED, EC_ERR_PACK_NO_STORAGE, EC_ERR_PACK_NOTHING_TO_PACK,
    EC_HELP,
};
use crate::failure::failure;

/// The flags `rola pack` takes.
///
/// What is packed by default is everything the run can reach, store and index alike; each flag
/// leaves one of the two out.
#[derive(Pickable)]
struct PackFlags {
    /// Lay the index's objects out too. Absent means the index is left as it is.
    #[arg(long)]
    no_index: Flag,
    /// Lay the store's objects out too. Absent means the store is left as it is.
    #[arg(long)]
    no_storage: Flag,
}

#[help(buffer)]
pub fn help_pack(_: EntryPack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("pack.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryPack)]
pub fn desc_pack() -> Description {
    t!("pack.cmd_pack_description").to_string().into()
}

/// Completes what `rola pack` can be given next.
///
/// A packing names nothing: what is laid out is whatever store and index the run works on, and the
/// only thing a caller chooses is which of the two to leave alone.
#[completion(EntryPack)]
pub fn complete_pack(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                "--no-index": t!("pack.complete.no_index"),
                "--no-storage": t!("pack.complete.no_storage"),
            },
        );
    }

    suggest!()
}

/// Lays the objects a store and an index hold out in packs.
///
/// Everything the store holds is written again: the entries of every pack are gathered up with the
/// loose objects, and the whole of it is laid down as one pack — or as several, once one would pass
/// the store's `max_pack_size`, which is `[storage] max_pack_size` in the store's configuration and
/// two gibibytes when that says nothing. The index the run is inside is laid out the same way, as
/// one pack. What each object *is* does not change: a pack holds its entries exactly as they sat
/// loose, so every read that worked before works after.
///
/// Both are packed by default. `--no-index` leaves the index as it is and `--no-storage` leaves the
/// store as it is, so a run that wants only one of the two says so; with both, there is nothing to
/// pack and the run is told so.
///
/// A store or an index already laid out the way this would lay it out is left alone, so running
/// this twice in a row changes nothing the second time. What is printed says which of the two
/// happened.
///
/// Each is locked for the whole of the work, so a run packing one is the only run packing it. One
/// another run holds the lock of is not waited for — the run is told, and nothing is changed.
///
/// The run has to be somewhere a store or an index can be found — inside one, or inside a Vault or
/// Workspace that keeps one.
///
/// # Errors
///
/// Renders [`ErrorNothingToPack`] when both `--no-index` and `--no-storage` were written, which
/// leaves nothing to lay out, [`ErrorPackNoStorage`] when the run is nowhere a store or an index
/// is, [`ErrorPackLocked`] when one is locked by another run, and [`ErrorPackFailed`] when the
/// objects could not be read or a pack could not be written.
#[command(node = "pack", entry = EntryPack)]
pub fn pack(args: EntryPack) -> StatePack {
    // Picking flags cannot fail: a flag that is absent is `Inactive`, not an error.
    let flags = args.pick(&arg![PackFlags]).unwrap();

    StatePack {
        storage: matches!(flags.no_storage, Flag::Inactive),
        index: matches!(flags.no_index, Flag::Inactive),
    }
}

/// The state a packing run starts in.
///
/// A packing names nothing: what is laid out is whatever store and index the run works on, so the
/// whole of what it is told is already in where the run happens to be and which of the two flags
/// were written.
#[derive(Grouped, Clone, Copy)]
pub struct StatePack {
    /// Whether the store is to be laid out.
    storage: bool,
    /// Whether the index is to be laid out.
    index: bool,
}

#[chain(routeify)]
pub fn handle_pack(
    state: StatePack,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    // Both flags together leave nothing to lay out, which is a run that would do nothing rather
    // than one that happens to find nothing: it is said apart from `ErrorPackNoStorage`.
    if !state.storage && !state.index {
        return ErrorNothingToPack.into();
    }

    let store = if state.storage {
        storage.get_ref().as_ref()
    } else {
        None
    };
    let vcs_index = if state.index {
        index.get_ref().as_ref()
    } else {
        None
    };

    if store.is_none() && vcs_index.is_none() {
        return ErrorPackNoStorage.into();
    }

    // The store and the index are asynchronous and a command is not, so the two meet here: the
    // objects are read and the packs written by each, and a runtime of this run's own is what waits
    // for them — see `cmd_storage_write_file`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorPackFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let mut changed = false;

    // Which objects there are and how they are laid out is each side's own to say: this hands over
    // nothing and asks for the whole of it to be laid out afresh — under the lock of that side, so
    // that nothing else is packing it while it happens.
    if let Some(store) = store {
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
            Ok(was) => changed |= was,
            Err(error) => {
                return ErrorPackFailed {
                    cause: error.to_string(),
                }
                .into();
            }
        }
    }

    if let Some(store) = vcs_index {
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
            Ok(was) => changed |= was,
            Err(error) => {
                return ErrorPackFailed {
                    cause: error.to_string(),
                }
                .into();
            }
        }
    }

    ResultPacked { changed }.into()
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

/// Error: the run was told to pack neither the store nor the index.
///
/// `--no-storage` and `--no-index` together leave nothing to lay out. That is a run that would do
/// nothing rather than one that has nothing, so it is said apart from [`ErrorPackNoStorage`].
#[derive(Grouped)]
pub struct ErrorNothingToPack;

impl Failure for ErrorNothingToPack {
    fn name(&self) -> &'static str {
        "error_pack_nothing_to_pack"
    }

    fn reason(&self) -> String {
        t!("pack.err_nothing_to_pack").trim().to_string()
    }
}

failure!(ErrorNothingToPack);

#[renderer(buffer)]
pub fn render_error_pack_nothing_to_pack(error: ErrorNothingToPack, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("pack.err_nothing_to_pack_help").trim()));
    ec.exit_code = EC_ERR_PACK_NOTHING_TO_PACK;
}

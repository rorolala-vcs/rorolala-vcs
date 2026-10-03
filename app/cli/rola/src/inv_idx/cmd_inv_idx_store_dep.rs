//! The `rola inv-idx store-dep` command: list the variants made from a stored entry.

use librorolala::inverse_index::InverseIndex;
use librorolala::storage::Key;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{offer, positional, store_key_set, store_keys, typing_flag};
use crate::exit_codes::EC_HELP;
use crate::format::ResFormat;
use crate::inv_idx::{
    DEFAULT_FORMAT_HASHES, ErrorInvIdxArgument, ErrorInvIdxNoIndex, ResultInvIdxHashes, hash_of,
    reading_error, runtime,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex};

#[help(buffer)]
pub fn help_inv_idx_store_dep(_: EntryInvIdxStoreDep, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx_store_dep.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInvIdxStoreDep)]
pub fn desc_inv_idx_store_dep() -> Description {
    t!("inv_idx_store_dep.description").to_string().into()
}

/// Lists the variants made from the content stored under a key
///
/// # Errors
///
/// Renders [`ErrorInvIdxNoIndex`] when the run is nowhere an index is, [`ErrorInvIdxArgument`] when
/// the argument is missing, [`ErrorInvIdxHash`](crate::inv_idx::ErrorInvIdxHash) when it does not read as a hash, and the reading
/// failure when the index could not be read.
#[command(node = "inv-idx.store-dep", entry = EntryInvIdxStoreDep)]
pub fn inv_idx_store_dep(args: EntryInvIdxStoreDep, format: &mut ResFormat) -> Next {
    let hash = match args
        .pick_or_route(&arg![String], || {
            ErrorInvIdxArgument {
                argument: "STORE".to_owned(),
            }
            .into()
        })
        .to_result()
    {
        Ok(hash) => hash,
        Err(next) => return next,
    };

    format.default_template(DEFAULT_FORMAT_HASHES);
    StateInvIdxStoreDep { hash }.into()
}

/// The state a listing starts in: the key it is about.
#[derive(Grouped)]
pub struct StateInvIdxStoreDep {
    /// The storage address.
    hash: String,
}

#[chain(routeify)]
pub fn handle_inv_idx_store_dep(
    state: StateInvIdxStoreDep,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    format: &mut ResFormat,
) -> Next {
    let StateInvIdxStoreDep { hash } = state;
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorInvIdxNoIndex.into();
    };
    let store = store_key_set(storage.get_ref().as_ref());
    let key = match hash_of(&hash, || store) {
        Ok(key) => key,
        Err(error) => return error.into(),
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let inverse = InverseIndex::at(index.clone());
    match runtime.block_on(inverse.store_dependents(key)) {
        Ok(hashes) => {
            let hashes: Vec<String> = hashes.iter().map(Key::hex).collect();
            format.set(
                "hashes",
                hashes.iter().map(|hash| serde_json::json!(hash)).collect(),
            );
            ResultInvIdxHashes { hashes }.into()
        }
        Err(error) => reading_error(error),
    }
}

/// Completes what `rola inv-idx store-dep` can be given next.
///
/// The hash names a key the store holds, so those are what is offered.
#[completion(EntryInvIdxStoreDep)]
pub fn complete_inv_idx_store_dep(
    ctx: ShellContext,
    storage: &mut LazyRes<ResRorolalaStorage>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "store-dep") != 0 {
        return suggest!();
    }

    offer(&ctx, store_keys(storage.get_ref().as_ref()))
}

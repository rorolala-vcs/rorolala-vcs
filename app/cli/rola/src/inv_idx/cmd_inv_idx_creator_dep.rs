//! The `rola inv-idx creator-dep` command: list the variants made by a creator.

use librorolala::inverse_index::InverseIndex;
use librorolala::storage::Key;
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::inv_idx::{
    ErrorInvIdxArgument, ErrorInvIdxHash, ErrorInvIdxNoIndex, ResultInvIdxHashes, parse_hash,
    reading_error, runtime,
};

#[help(buffer)]
pub fn help_inv_idx_creator_dep(_: EntryInvIdxCreatorDep, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx_creator_dep.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInvIdxCreatorDep)]
pub fn desc_inv_idx_creator_dep() -> Description {
    t!("inv_idx_creator_dep.description").to_string().into()
}

/// Lists the variants made by a creator
///
/// # Errors
///
/// Renders [`ErrorInvIdxNoIndex`] when the run is nowhere an index is, [`ErrorInvIdxArgument`] when
/// the argument is missing, [`ErrorInvIdxHash`] when it does not read as a hash, and the reading
/// failure when the index could not be read.
#[command(node = "inv-idx.creator-dep", entry = EntryInvIdxCreatorDep)]
pub fn inv_idx_creator_dep(args: EntryInvIdxCreatorDep) -> Next {
    let hash = match args
        .pick_or_route(&arg![String], || {
            ErrorInvIdxArgument {
                argument: "HASH".to_owned(),
            }
            .into()
        })
        .to_result()
    {
        Ok(hash) => hash,
        Err(next) => return next,
    };

    StateInvIdxCreatorDep { hash }.into()
}

/// The state a listing starts in: the creator it is about.
#[derive(Grouped)]
pub struct StateInvIdxCreatorDep {
    /// The creator's hash.
    hash: String,
}

#[chain(routeify)]
pub fn handle_inv_idx_creator_dep(
    state: StateInvIdxCreatorDep,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(key) = parse_hash(&state.hash) else {
        return ErrorInvIdxHash { hash: state.hash }.into();
    };
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorInvIdxNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let inverse = InverseIndex::at(index.clone());
    match runtime.block_on(inverse.creator_dependents(key)) {
        Ok(hashes) => ResultInvIdxHashes {
            hashes: hashes.iter().map(Key::hex).collect(),
        }
        .into(),
        Err(error) => reading_error(error),
    }
}

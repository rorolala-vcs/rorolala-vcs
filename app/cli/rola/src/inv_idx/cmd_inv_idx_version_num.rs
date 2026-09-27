//! The `rola inv-idx version-num` command: print a version's number.

use librorolala::inverse_index::InverseIndex;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::inv_idx::{
    ErrorInvIdxArgument, ErrorInvIdxHash, ErrorInvIdxNoIndex, parse_hash, reading_error, runtime,
};

#[help(buffer)]
pub fn help_inv_idx_version_num(_: EntryInvIdxVersionNum, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx_version_num.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInvIdxVersionNum)]
pub fn desc_inv_idx_version_num() -> Description {
    t!("inv_idx_version_num.description").to_string().into()
}

/// Prints a version's number
///
/// The number is derived rather than stored, so what answers is the records when they hold it and
/// the chain otherwise. The root version is numbered `u64::MAX`, one before the first.
///
/// # Errors
///
/// Renders [`ErrorInvIdxNoIndex`] when the run is nowhere an index is, [`ErrorInvIdxArgument`] when
/// the argument is missing, [`ErrorInvIdxHash`] when it does not read as a hash, and the reading
/// failure when nothing is stored under the hash or it is not a version.
#[command(node = "inv-idx.version-num", entry = EntryInvIdxVersionNum)]
pub fn inv_idx_version_num(args: EntryInvIdxVersionNum) -> Next {
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

    StateInvIdxVersionNum { hash }.into()
}

/// The state a number is asked in: the version it is about.
#[derive(Grouped)]
pub struct StateInvIdxVersionNum {
    /// The version's hash.
    hash: String,
}

#[chain(routeify)]
pub fn handle_inv_idx_version_num(
    state: StateInvIdxVersionNum,
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
    match runtime.block_on(inverse.version_num(key)) {
        Ok(number) => ResultInvIdxNumber { number }.into(),
        Err(error) => reading_error(error),
    }
}

/// Result: a version's number.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultInvIdxNumber {
    /// The number.
    number: u64,
}

#[renderer(buffer)]
pub fn render_result_inv_idx_number(result: ResultInvIdxNumber) {
    r_println!("{}", result.number);
}

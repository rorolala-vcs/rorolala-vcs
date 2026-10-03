//! The `rola inv-idx version-num` command: print a version's number.

use librorolala::inverse_index::InverseIndex;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_utils_cli_theme::{err_line, trd};
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::complete::{IndexObject, index_hashes, index_keys, offer, positional, typing_flag};
use crate::exit_codes::{EC_ERR_FORMAT, EC_HELP};
use crate::format::ResFormat;
use crate::inv_idx::{ErrorInvIdxArgument, ErrorInvIdxNoIndex, hash_of, reading_error, runtime};

/// How a version's number is drawn when no template is named.
///
/// The one field of the result, `number`.
const DEFAULT_FORMAT: &str = "{{ number }}";

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
/// the argument is missing, [`ErrorInvIdxHash`](crate::inv_idx::ErrorInvIdxHash) when it does not read as a hash, and the reading
/// failure when nothing is stored under the hash or it is not a version.
#[command(node = "inv-idx.version-num", entry = EntryInvIdxVersionNum)]
pub fn inv_idx_version_num(args: EntryInvIdxVersionNum, format: &mut ResFormat) -> Next {
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

    format.default_template(DEFAULT_FORMAT);
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
    format: &mut ResFormat,
) -> Next {
    let StateInvIdxVersionNum { hash } = state;
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorInvIdxNoIndex.into();
    };
    let key = match hash_of(&hash, || index_keys(Some(index), IndexObject::Version)) {
        Ok(key) => key,
        Err(error) => return error.into(),
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let inverse = InverseIndex::at(index.clone());
    match runtime.block_on(inverse.version_num(key)) {
        Ok(number) => {
            format.set("number", vec![serde_json::json!(number)]);
            ResultInvIdxNumber { number }.into()
        }
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
pub fn render_result_inv_idx_number(
    result: ResultInvIdxNumber,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
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
        r_println!("{}", result.number);
    }
}

/// Completes what `rola inv-idx version-num` can be given next.
///
/// The hash names a version the index holds, so those are what is offered.
#[completion(EntryInvIdxVersionNum)]
pub fn complete_inv_idx_version_num(
    ctx: ShellContext,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "version-num") != 0 {
        return suggest!();
    }

    offer(
        &ctx,
        index_hashes(index.get_ref().as_ref(), IndexObject::Version),
    )
}

//! The `rola inv-idx variant-dep` command: list the versions that fix a variant.

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
use rorolala_cli_setups::ResVCSIndex;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{IndexObject, index_hashes, index_keys, offer, positional, typing_flag};
use crate::exit_codes::EC_HELP;
use crate::format::ResFormat;
use crate::inv_idx::{
    DEFAULT_FORMAT_HASHES, ErrorInvIdxArgument, ErrorInvIdxNoIndex, ResultInvIdxHashes, hash_of,
    reading_error, runtime,
};

#[help(buffer)]
pub fn help_inv_idx_variant_dep(_: EntryInvIdxVariantDep, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("inv_idx_variant_dep.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryInvIdxVariantDep)]
pub fn desc_inv_idx_variant_dep() -> Description {
    t!("inv_idx_variant_dep.description").to_string().into()
}

/// Lists the versions that fix a variant
///
/// # Errors
///
/// Renders [`ErrorInvIdxNoIndex`] when the run is nowhere an index is, [`ErrorInvIdxArgument`] when
/// the argument is missing, [`ErrorInvIdxHash`](crate::inv_idx::ErrorInvIdxHash) when it does not read as a hash, and the reading
/// failure when the index could not be read.
#[command(node = "inv-idx.variant-dep", entry = EntryInvIdxVariantDep)]
pub fn inv_idx_variant_dep(args: EntryInvIdxVariantDep, format: &mut ResFormat) -> Next {
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

    format.default_template(DEFAULT_FORMAT_HASHES);
    StateInvIdxVariantDep { hash }.into()
}

/// The state a listing starts in: the variant it is about.
#[derive(Grouped)]
pub struct StateInvIdxVariantDep {
    /// The variant's hash.
    hash: String,
}

#[chain(routeify)]
pub fn handle_inv_idx_variant_dep(
    state: StateInvIdxVariantDep,
    index: &mut LazyRes<ResVCSIndex>,
    format: &mut ResFormat,
) -> Next {
    let StateInvIdxVariantDep { hash } = state;
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorInvIdxNoIndex.into();
    };
    let key = match hash_of(&hash, || index_keys(Some(index), IndexObject::Variant)) {
        Ok(key) => key,
        Err(error) => return error.into(),
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let inverse = InverseIndex::at(index.clone());
    match runtime.block_on(inverse.variant_dependents(key)) {
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

/// Completes what `rola inv-idx variant-dep` can be given next.
///
/// The hash names a variant the index holds, so those are what is offered.
#[completion(EntryInvIdxVariantDep)]
pub fn complete_inv_idx_variant_dep(
    ctx: ShellContext,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "variant-dep") != 0 {
        return suggest!();
    }

    offer(
        &ctx,
        index_hashes(index.get_ref().as_ref(), IndexObject::Variant),
    )
}

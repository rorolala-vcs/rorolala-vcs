//! The `rola vcs-index write-variant` command: write a Variant and answer with its hash.
//!
//! Nothing is checked to be there — the base version, the storage entry, the creator and the
//! message are taken as the hashes they are given. `--join` makes it a merge, carrying the variant
//! it brings in.

use librorolala::storage::Key;
use librorolala::vcs::{UNKNOWN_VERSION, Variant};
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex};
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{
    IndexObject, filling_flag, index_hashes, index_keyed, offer, positional, store_key_set,
    store_keys, strip_written, typing_flag,
};
use crate::exit_codes::EC_HELP;
use crate::rebuild::ResRebuildInverseIndex;
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexNoIndex, ErrorVcsIndexWrite, ResultVcsIndexHash, hash_of,
    rebuild_inverse_index, runtime,
};

/// The variant this one merges in.
const ARG_JOIN: PickerArg<'static, Option<String>> = arg![join: Option<String>];

#[help(buffer)]
pub fn help_vcs_index_write_variant(_: EntryVcsIndexWriteVariant, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_write_variant.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexWriteVariant)]
pub fn desc_vcs_index_write_variant() -> Description {
    t!("vcs_index_write_variant.description").to_string().into()
}

/// Completes what `rola vcs-index write-variant` can be given next.
///
/// Each word names an object the run already has: the base version, the creator and the message are
/// index objects, the storage entry is a key the store holds, and `--join` names another variant.
#[completion(EntryVcsIndexWriteVariant)]
pub fn complete_vcs_index_write_variant(
    ctx: ShellContext,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
) -> Suggest {
    if filling_flag(&ctx, &ARG_JOIN) {
        return offer(
            &ctx,
            index_hashes(index.get_ref().as_ref(), IndexObject::Variant),
        );
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_JOIN: t!("vcs_index_write_variant.complete.join"),
            },
        );
    }

    let held = index.get_ref().as_ref();
    match positional(&ctx, "write-variant") {
        0 => offer(&ctx, index_hashes(held, IndexObject::Version)),
        1 => offer(&ctx, store_keys(storage.get_ref().as_ref())),
        2 => offer(&ctx, index_hashes(held, IndexObject::Creator)),
        3 => offer(&ctx, index_hashes(held, IndexObject::Message)),
        _ => suggest!(),
    }
}

/// Writes a Variant into the index, answering with its hash
///
/// Nothing is checked to be there: the base version and the storage entry are taken as the hashes
/// they are given, and the number the chain would work out is left unknown.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexArgument`] when an argument is missing, [`ErrorVcsIndexHash`](crate::vcs_index::ErrorVcsIndexHash) when one does not
/// read as a hash, and [`ErrorVcsIndexWrite`] when the index cannot be written to.
#[command(node = "vcs-index.write-variant", entry = EntryVcsIndexWriteVariant)]
pub fn vcs_index_write_variant(args: EntryVcsIndexWriteVariant) -> Next {
    let picked = args
        .pick(&ARG_JOIN)
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "BASE_VERSION".to_owned(),
            }
            .into()
        })
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "STORAGE".to_owned(),
            }
            .into()
        })
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "CREATOR_HASH".to_owned(),
            }
            .into()
        })
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "MESSAGE_HASH".to_owned(),
            }
            .into()
        })
        .to_result();
    let (join, base_version, storage, creator, message) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateVcsIndexWriteVariant {
        base_version,
        storage,
        creator,
        message,
        join,
    }
    .into()
}

/// The state a Variant write starts in.
#[derive(Grouped)]
pub struct StateVcsIndexWriteVariant {
    /// The base version's hash.
    base_version: String,
    /// The storage entry's hash.
    storage: String,
    /// The creator's hash.
    creator: String,
    /// The message's hash.
    message: String,
    /// The joined variant's hash, when this is a merge.
    join: Option<String>,
}

#[chain(routeify)]
pub fn handle_vcs_index_write_variant(
    state: StateVcsIndexWriteVariant,
    index: &mut LazyRes<ResVCSIndex>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    rebuild: &ResRebuildInverseIndex,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };

    // The index is read whole once: four kinds were asked for, which is four filters over one
    // listing rather than four listings.
    let held = index_keyed(Some(index));
    let keys_of = |kind: IndexObject| -> Vec<Key> {
        held.iter()
            .filter(|(_, object)| kind.holds(object))
            .map(|(key, _)| *key)
            .collect()
    };

    let base_version = match hash_of(&state.base_version, || keys_of(IndexObject::Version)) {
        Ok(key) => key,
        Err(error) => return error.into(),
    };
    let store = store_key_set(storage.get_ref().as_ref());
    let storage = match hash_of(&state.storage, || store) {
        Ok(key) => key,
        Err(error) => return error.into(),
    };
    let creator = match hash_of(&state.creator, || keys_of(IndexObject::Creator)) {
        Ok(key) => key,
        Err(error) => return error.into(),
    };
    let message = match hash_of(&state.message, || keys_of(IndexObject::Message)) {
        Ok(key) => key,
        Err(error) => return error.into(),
    };
    let join = match state.join {
        Some(join) => match hash_of(&join, || keys_of(IndexObject::Variant)) {
            Ok(join) => Some(*join.digest()),
            Err(error) => return error.into(),
        },
        None => None,
    };

    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let variant = Variant::new_bare_variant(
        *storage.digest(),
        *base_version.digest(),
        join,
        *creator.digest(),
        *message.digest(),
        UNKNOWN_VERSION,
    );
    match runtime.block_on(index.write(variant)) {
        Ok(key) => {
            if **rebuild && let Err(error) = rebuild_inverse_index(index) {
                return error.into();
            }
            ResultVcsIndexHash { hash: key.hex() }.into()
        }
        Err(error) => ErrorVcsIndexWrite {
            cause: error.reason(),
        }
        .into(),
    }
}

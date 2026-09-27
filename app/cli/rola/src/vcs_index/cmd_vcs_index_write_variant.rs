//! The `rola vcs-index write-variant` command: write a Variant and answer with its hash.
//!
//! Nothing is checked to be there — the base version, the storage entry, the creator and the
//! message are taken as the hashes they are given. `--join` makes it a merge, carrying the variant
//! it brings in.

use librorolala::vcs::{UNKNOWN_VERSION, Variant};
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    picker::{EntryPicker, Pickable},
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::rebuild::ResRebuildInverseIndex;
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexHash, ErrorVcsIndexNoIndex, ErrorVcsIndexWrite,
    ResultVcsIndexHash, parse_hash, rebuild_inverse_index, runtime,
};

/// The flags `rola vcs-index write-variant` takes.
#[derive(Pickable)]
struct WriteVariantFlags {
    /// The variant this one merges in.
    #[arg(long)]
    join: Option<String>,
}

#[help(buffer)]
pub fn help_vcs_index_write_variant(_: EntryVcsIndexWriteVariant, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_write_variant.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexWriteVariant)]
pub fn desc_vcs_index_write_variant() -> Description {
    t!("vcs_index_write_variant.description").to_string().into()
}

/// Writes a Variant into the index, answering with its hash
///
/// Nothing is checked to be there: the base version and the storage entry are taken as the hashes
/// they are given, and the number the chain would work out is left unknown.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexArgument`] when an argument is missing, [`ErrorVcsIndexHash`] when one does not
/// read as a hash, and [`ErrorVcsIndexWrite`] when the index cannot be written to.
#[command(node = "vcs-index.write-variant", entry = EntryVcsIndexWriteVariant)]
pub fn vcs_index_write_variant(args: EntryVcsIndexWriteVariant) -> Next {
    let picked = args
        .pick(&arg![WriteVariantFlags])
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
    let (flags, base_version, storage, creator, message) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateVcsIndexWriteVariant {
        base_version,
        storage,
        creator,
        message,
        join: flags.join,
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
    rebuild: &ResRebuildInverseIndex,
) -> Next {
    let Some(base_version) = parse_hash(&state.base_version) else {
        return ErrorVcsIndexHash {
            hash: state.base_version,
        }
        .into();
    };
    let Some(storage) = parse_hash(&state.storage) else {
        return ErrorVcsIndexHash {
            hash: state.storage,
        }
        .into();
    };
    let Some(creator) = parse_hash(&state.creator) else {
        return ErrorVcsIndexHash {
            hash: state.creator,
        }
        .into();
    };
    let Some(message) = parse_hash(&state.message) else {
        return ErrorVcsIndexHash {
            hash: state.message,
        }
        .into();
    };
    let join = match state.join {
        Some(join) => match parse_hash(&join) {
            Some(join) => Some(*join.digest()),
            None => return ErrorVcsIndexHash { hash: join }.into(),
        },
        None => None,
    };

    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
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

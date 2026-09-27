//! The `rola vcs-index write-version` command: write a Version and answer with its hash.
//!
//! Nothing is checked to be there — the variant is taken as the hash it is given, and the number
//! the chain would work out is left unknown.

use librorolala::vcs::{UNKNOWN_VERSION, Version};
use mingling::{
    Grouped, LazyRes,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexHash, ErrorVcsIndexNoIndex, ErrorVcsIndexWrite,
    ResultVcsIndexHash, parse_hash, runtime,
};

#[help(buffer)]
pub fn help_vcs_index_write_version(_: EntryVcsIndexWriteVersion, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_write_version.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexWriteVersion)]
pub fn desc_vcs_index_write_version() -> Description {
    t!("vcs_index_write_version.description").to_string().into()
}

/// Writes a Version into the index, answering with its hash
///
/// Nothing is checked to be there: the variant, the creator and the message are taken as the
/// hashes they are given, and the number the chain would work out is left unknown.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexArgument`] when an argument is missing, [`ErrorVcsIndexHash`] when one does not
/// read as a hash, and [`ErrorVcsIndexWrite`] when the index cannot be written to.
#[command(node = "vcs-index.write-version", entry = EntryVcsIndexWriteVersion)]
pub fn vcs_index_write_version(args: EntryVcsIndexWriteVersion) -> Next {
    let picked = args
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "VARIANT".to_owned(),
            }
            .into()
        })
        .to_result();
    let variant = match picked {
        Ok(variant) => variant,
        Err(next) => return next,
    };

    StateVcsIndexWriteVersion { variant }.into()
}

/// The state a Version write starts in.
#[derive(Grouped)]
pub struct StateVcsIndexWriteVersion {
    /// The variant's hash.
    variant: String,
}

#[chain(routeify)]
pub fn handle_vcs_index_write_version(
    state: StateVcsIndexWriteVersion,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(variant) = parse_hash(&state.variant) else {
        return ErrorVcsIndexHash {
            hash: state.variant,
        }
        .into();
    };

    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let version = Version::new_bare_version(*variant.digest(), UNKNOWN_VERSION);
    match runtime.block_on(index.write(version)) {
        Ok(key) => ResultVcsIndexHash { hash: key.hex() }.into(),
        Err(error) => ErrorVcsIndexWrite {
            cause: error.reason(),
        }
        .into(),
    }
}

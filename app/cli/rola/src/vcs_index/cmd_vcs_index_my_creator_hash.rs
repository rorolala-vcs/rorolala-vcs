//! The `rola vcs-index my-creator-hash` command: the hash of the Creator the work acts as.
//!
//! Who made a change belongs to the variant, so a variant carries a Creator's hash — the object the
//! account the work acts as is recorded under. The Creator is written into the index, and what is
//! printed is the hash to hand to `write-variant` as `CREATOR_HASH`. The same name is the same
//! object, so it is created once and read back whole on every run after.

use librorolala::vcs::Creator;
use mingling::{
    Grouped, LazyRes,
    macros::{buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::EC_HELP;
use crate::format::ResFormat;
use crate::vcs_index::{
    DEFAULT_FORMAT_HASH, ErrorVcsIndexNoIndex, ErrorVcsIndexWrite, ResultVcsIndexHash, runtime,
};

#[help(buffer)]
pub fn help_vcs_index_my_creator_hash(_: EntryVcsIndexMyCreatorHash, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_my_creator_hash.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexMyCreatorHash)]
pub fn desc_vcs_index_my_creator_hash() -> Description {
    t!("vcs_index_my_creator_hash.description")
        .to_string()
        .into()
}

/// Prints the hash of the Creator the work acts as
///
/// The account the work acts as names the Creator: the same name is the same object, so writing it
/// where it already is changes nothing. What comes back is what a `write-variant` takes for
/// `CREATOR_HASH`.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorNoAccount`](crate::account::ErrorNoAccount) when the work acts as no account,
/// [`ErrorVcsIndexWrite`] when the name is too long or the index cannot be written to.
#[command(node = "vcs-index.my-creator-hash")]
pub fn vcs_index_my_creator_hash(format: &mut ResFormat) -> StateVcsIndexMyCreatorHash {
    format.default_template(DEFAULT_FORMAT_HASH);
    StateVcsIndexMyCreatorHash
}

/// The state a creator hash read starts in.
///
/// It names nothing: whose hash it is comes from the account the work acts as.
#[derive(Grouped)]
pub struct StateVcsIndexMyCreatorHash;

#[chain(routeify)]
pub fn handle_vcs_index_my_creator_hash(
    _state: StateVcsIndexMyCreatorHash,
    index: &mut LazyRes<ResVCSIndex>,
    account: &mut LazyRes<ResCurrentAccount>,
    format: &mut ResFormat,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let name = match account.get_ref().must_bind() {
        Ok(name) => name,
        Err(error) => return error.into(),
    };
    let creator = match Creator::try_from(name) {
        Ok(creator) => creator,
        Err(error) => {
            return ErrorVcsIndexWrite {
                cause: error.to_string(),
            }
            .into();
        }
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    match runtime.block_on(index.write(creator)) {
        Ok(key) => {
            let hash = key.hex();
            format.set("hash", vec![serde_json::json!(hash)]);
            ResultVcsIndexHash { hash }.into()
        }
        Err(error) => ErrorVcsIndexWrite {
            cause: error.reason(),
        }
        .into(),
    }
}

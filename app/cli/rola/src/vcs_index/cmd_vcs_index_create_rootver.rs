//! The `rola vcs-index create-rootver` command: put the root version into the index.
//!
//! The root version is the version before the first — numbered `u64::MAX` — and every chain starts
//! from it: a file's first variant is based on it. It is the same object in every index, so this
//! names nothing and, written when it is already there, changes nothing.

use librorolala::vcs::Version;
use mingling::{
    Grouped, LazyRes, Suggest,
    macros::{buffer, chain, command, completion, help, metadata, r_eprintln, routeify, suggest},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::vcs_index::{ErrorVcsIndexNoIndex, ErrorVcsIndexWrite, ResultVcsIndexHash, runtime};

#[help(buffer)]
pub fn help_vcs_index_create_rootver(_: EntryVcsIndexCreateRootver, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_create_rootver.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexCreateRootver)]
pub fn desc_vcs_index_create_rootver() -> Description {
    t!("vcs_index_create_rootver.description")
        .to_string()
        .into()
}

/// Puts the root version into the index, answering with its hash
///
/// The root version is the version before the first — `u64::MAX` as a number — and every chain
/// starts from it. It is the same object in every index, so writing it where it already is changes
/// nothing; what comes back is what a first `write-variant` takes for `BASE_VERSION`.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is, and
/// [`ErrorVcsIndexWrite`] when the index cannot be written to.
#[command(node = "vcs-index.create-rootver")]
pub fn vcs_index_create_rootver() -> StateVcsIndexCreateRootver {
    StateVcsIndexCreateRootver
}

/// The state a root version creation starts in.
///
/// It names nothing: the root version is one object for the whole index.
#[derive(Grouped)]
pub struct StateVcsIndexCreateRootver;

#[chain(routeify)]
pub fn handle_vcs_index_create_rootver(
    _state: StateVcsIndexCreateRootver,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    match runtime.block_on(index.write(Version::root())) {
        Ok(key) => ResultVcsIndexHash { hash: key.hex() }.into(),
        Err(error) => ErrorVcsIndexWrite {
            cause: error.reason(),
        }
        .into(),
    }
}

/// Completes what `rola vcs-index create-rootver` can be given next.
///
/// The listing names nothing, so there is nothing to offer.
#[completion(EntryVcsIndexCreateRootver)]
pub fn complete_vcs_index_create_rootver() -> Suggest {
    suggest!()
}

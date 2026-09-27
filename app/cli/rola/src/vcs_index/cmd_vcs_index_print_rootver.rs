//! The `rola vcs-index print-rootver` command: print the root version's hash.
//!
//! The root version is the version before the first — numbered `u64::MAX` — and every chain starts
//! from it. Its hash is the same in every index, and it is what a first `write-variant` takes for
//! `BASE_VERSION`. An index that does not hold it is told so rather than answered with the hash it
//! would have had: a chain whose root is not stored cannot be read back.

use librorolala::vcs::{VCSIndexReadingError, VCSWrite as _, Version};
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
use crate::exit_codes::EC_HELP;
use crate::vcs_index::{
    ErrorVcsIndexNoIndex, ErrorVcsIndexNotFound, ErrorVcsIndexRead, ResultVcsIndexHash, runtime,
};

#[help(buffer)]
pub fn help_vcs_index_print_rootver(_: EntryVcsIndexPrintRootver, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_print_rootver.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexPrintRootver)]
pub fn desc_vcs_index_print_rootver() -> Description {
    t!("vcs_index_print_rootver.description").to_string().into()
}

/// Prints the root version's hash
///
/// The root version is the version before the first — `u64::MAX` as a number — and every chain
/// starts from it. What is printed is what a first `write-variant` takes for `BASE_VERSION`; an
/// index that does not hold the root version is told so, since nothing read back from it could.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexNotFound`] when the index does not hold the root version, and
/// [`ErrorVcsIndexRead`] when the index could not be read.
#[command(node = "vcs-index.print-rootver")]
pub fn vcs_index_print_rootver() -> StateVcsIndexPrintRootver {
    StateVcsIndexPrintRootver
}

/// The state printing the root version starts in.
///
/// It names nothing: the root version is one object for the whole index.
#[derive(Grouped)]
pub struct StateVcsIndexPrintRootver;

#[chain(routeify)]
pub fn handle_vcs_index_print_rootver(
    _state: StateVcsIndexPrintRootver,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let root = Version::root();
    let hash = root.hash().hex();

    match runtime.block_on(index.read(root.hash())) {
        Ok(_) => ResultVcsIndexHash { hash }.into(),
        Err(VCSIndexReadingError::NotFound { .. }) => ErrorVcsIndexNotFound { hash }.into(),
        Err(error) => ErrorVcsIndexRead {
            cause: error.reason(),
        }
        .into(),
    }
}

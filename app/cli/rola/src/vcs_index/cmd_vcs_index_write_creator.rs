//! The `rola vcs-index write-creator` command: write a Creator and answer with its hash.
//!
//! A Creator is the name beside a change: the text object a Variant carries the hash of. What is
//! answered with is that hash, which is what a `write-variant` takes for its `CREATOR_HASH`.

use librorolala::vcs::Creator;
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
use crate::rebuild::ResRebuildInverseIndex;
use crate::vcs_index::{
    ErrorVcsIndexArgument, ErrorVcsIndexNoIndex, ErrorVcsIndexWrite, ResultVcsIndexHash,
    rebuild_inverse_index, runtime,
};

#[help(buffer)]
pub fn help_vcs_index_write_creator(_: EntryVcsIndexWriteCreator, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_write_creator.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexWriteCreator)]
pub fn desc_vcs_index_write_creator() -> Description {
    t!("vcs_index_write_creator.description").to_string().into()
}

/// Writes a Creator into the index, answering with its hash
///
/// The name is stored as the text object it is; the hash answered with is what a Version carries.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexArgument`] when the name is missing, [`ErrorVcsIndexWrite`] when the name is too
/// long or the index cannot be written to.
#[command(node = "vcs-index.write-creator", entry = EntryVcsIndexWriteCreator)]
pub fn vcs_index_write_creator(args: EntryVcsIndexWriteCreator) -> Next {
    let name = match args
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "CREATOR".to_owned(),
            }
            .into()
        })
        .to_result()
    {
        Ok(name) => name,
        Err(next) => return next,
    };

    StateVcsIndexWriteCreator { name }.into()
}

/// The state a Creator write starts in.
#[derive(Grouped)]
pub struct StateVcsIndexWriteCreator {
    /// The name to write.
    name: String,
}

#[chain(routeify)]
pub fn handle_vcs_index_write_creator(
    state: StateVcsIndexWriteCreator,
    index: &mut LazyRes<ResVCSIndex>,
    rebuild: &ResRebuildInverseIndex,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let creator = match Creator::try_from(state.name) {
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

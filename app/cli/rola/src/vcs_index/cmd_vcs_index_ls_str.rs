//! The `rola vcs-index ls-str` command: list the hashes of the text objects an index holds.
//!
//! A Creator and a Message are the index's text, held as objects so that a Version can say who made
//! it and what was done. This lists their hashes and nothing else; the text itself is read with
//! [`vcs_index_read`](crate::vcs_index::cmd_vcs_index_read).

use librorolala::vcs::VCSIndexObject;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_cli_setups::ResVCSIndex;
use rorolala_errors::Failure as _;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::exit_codes::EC_HELP;
use crate::vcs_index::{ErrorVcsIndexNoIndex, ErrorVcsIndexRead};

#[help(buffer)]
pub fn help_vcs_index_ls_str(_: EntryVcsIndexLsStr, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_ls_str.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexLsStr)]
pub fn desc_vcs_index_ls_str() -> Description {
    t!("vcs_index_ls_str.cmd_vcs_index_ls_str_description")
        .to_string()
        .into()
}

/// Lists the hashes of the text objects the index holds
///
/// A Creator and a Message are the index's text: a name and a message, held as objects so that a
/// Version can say who made it and what was done by carrying their hashes. What is listed is those
/// hashes and nothing else, since the text itself is read with `rola vcs-index read`.
///
/// The run has to be somewhere an index can be found — inside a Vault or a Workspace.
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is, and
/// [`ErrorVcsIndexRead`] when the index could not be read.
#[command(node = "vcs-index.ls-str")]
pub fn vcs_index_ls_str() -> StateVcsIndexLsStr {
    StateVcsIndexLsStr
}

/// The state a listing of the text objects starts in.
///
/// A listing names nothing: what is listed is whatever the run's index holds.
#[derive(Grouped)]
pub struct StateVcsIndexLsStr;

#[chain(routeify)]
pub fn handle_vcs_index_ls_str(
    _state: StateVcsIndexLsStr,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };

    // The index is asynchronous and a command is not, so the two meet here — see
    // `cmd_storage_write_file`.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return ErrorVcsIndexRead {
                cause: error.to_string(),
            }
            .into();
        }
    };

    match runtime.block_on(index.read_objects()) {
        Ok(objects) => {
            let mut string_hashes = Vec::new();
            for (key, object) in objects {
                if matches!(
                    object,
                    VCSIndexObject::Creator(_) | VCSIndexObject::Message(_)
                ) {
                    string_hashes.push(key.hex());
                }
            }

            ResultVcsIndexLsStr { string_hashes }.into()
        }
        Err(error) => ErrorVcsIndexRead {
            cause: error.reason(),
        }
        .into(),
    }
}

/// Result: the hashes of the text objects the index holds.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultVcsIndexLsStr {
    /// The hashes, as hex.
    string_hashes: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_vcs_index_ls_str(result: ResultVcsIndexLsStr) {
    for hash in &result.string_hashes {
        r_println!("{hash}");
    }
}

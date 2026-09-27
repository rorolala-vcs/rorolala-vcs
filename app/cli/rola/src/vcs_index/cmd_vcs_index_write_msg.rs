//! The `rola vcs-index write-msg` command: write a Message and answer with its hash.
//!
//! A Message is what was done: the text object a Variant carries the hash of. What is answered with
//! is that hash, which is what a `write-variant` takes for its `MESSAGE_HASH`.

use librorolala::vcs::Message;
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
pub fn help_vcs_index_write_msg(_: EntryVcsIndexWriteMsg, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vcs_index_write_msg.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVcsIndexWriteMsg)]
pub fn desc_vcs_index_write_msg() -> Description {
    t!("vcs_index_write_msg.description").to_string().into()
}

/// Writes a Message into the index, answering with its hash
///
/// # Errors
///
/// Renders [`ErrorVcsIndexNoIndex`] when the run is nowhere an index is,
/// [`ErrorVcsIndexArgument`] when the message is missing, [`ErrorVcsIndexWrite`] when it is too
/// long or the index cannot be written to.
#[command(node = "vcs-index.write-msg", entry = EntryVcsIndexWriteMsg)]
pub fn vcs_index_write_msg(args: EntryVcsIndexWriteMsg) -> Next {
    let message = match args
        .pick_or_route(&arg![String], || {
            ErrorVcsIndexArgument {
                argument: "MESSAGE".to_owned(),
            }
            .into()
        })
        .to_result()
    {
        Ok(message) => message,
        Err(next) => return next,
    };

    StateVcsIndexWriteMsg { message }.into()
}

/// The state a Message write starts in.
#[derive(Grouped)]
pub struct StateVcsIndexWriteMsg {
    /// The message to write.
    message: String,
}

#[chain(routeify)]
pub fn handle_vcs_index_write_msg(
    state: StateVcsIndexWriteMsg,
    index: &mut LazyRes<ResVCSIndex>,
    rebuild: &ResRebuildInverseIndex,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let message = match Message::try_from(state.message) {
        Ok(message) => message,
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

    match runtime.block_on(index.write(message)) {
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

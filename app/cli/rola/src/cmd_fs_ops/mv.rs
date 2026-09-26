//! The `rola fs-ops mv` command: put a path at another.

use std::path::PathBuf;

use mingling::{
    Grouped,
    macros::{arg, buffer, chain, command, help, metadata, r_eprintln, routeify},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::cmd_fs_ops::{self, ErrorFsOpsArguments, ErrorFsOpsFailed, ResultFsOps};
use crate::exit_codes::EC_HELP;

#[help(buffer)]
pub fn help_fs_ops_mv(_: EntryFsOpsMv, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fs_ops.mv_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFsOpsMv)]
pub fn desc_fs_ops_mv() -> Description {
    t!("fs_ops.mv_description").to_string().into()
}

/// Puts `FROM` at `TO`.
///
/// `TO` is the path the thing is to have, not the directory it is to be put in: what a name already
/// taken there means was settled before this was called. Either path may name a file or a directory,
/// and moving a directory moves everything under it.
///
/// # Errors
///
/// Renders [`ErrorFsOpsArguments`] when both paths were not named, and [`ErrorFsOpsFailed`] when the
/// filesystem refused the move.
#[command(node = "fs-ops.mv")]
pub fn fs_ops_mv(args: EntryFsOpsMv) -> Next {
    let picked = args
        .pick_or_route(&arg![PathBuf], || {
            ErrorFsOpsArguments {
                verb: "mv",
                needs_to: true,
            }
            .into()
        })
        .pick_or_route(&arg![PathBuf], || {
            ErrorFsOpsArguments {
                verb: "mv",
                needs_to: true,
            }
            .into()
        })
        .to_result();

    let (from, to) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateFsOpsMv { from, to }.into()
}

/// The state of moving one path to another.
#[derive(Grouped)]
pub struct StateFsOpsMv {
    /// What is being moved.
    from: PathBuf,
    /// Where it is being put.
    to: PathBuf,
}

#[chain(routeify)]
pub fn handle_fs_ops_mv(state: StateFsOpsMv) -> Next {
    // Taken apart rather than borrowed from, so that the state is consumed rather than only read: what the
    // operation needs is the two paths, and keeping the whole of a state alive for them would be keeping
    // more than it is used.
    let StateFsOpsMv { from, to } = state;

    match cmd_fs_ops::move_to(&from, &to) {
        Ok(()) => ResultFsOps {
            verb: "mv",
            from: from.display().to_string(),
            to: Some(to.display().to_string()),
        }
        .into(),
        Err(error) => ErrorFsOpsFailed {
            verb: "mv",
            cause: error.to_string(),
        }
        .into(),
    }
}

//! The `rola fs-ops rm` command: take a path away.

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
pub fn help_fs_ops_rm(_: EntryFsOpsRm, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fs_ops.rm_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFsOpsRm)]
pub fn desc_fs_ops_rm() -> Description {
    t!("fs_ops.rm_description").to_string().into()
}

/// Takes `PATH` away, and everything under it when it is a directory.
///
/// One word for both kinds, because which kind a path is, is read from the path rather than asked of
/// the caller: a removal that had to be told whether it was removing a file or a directory would be a
/// caller keeping a fact the filesystem already has. There is no confirmation and no `-f`; asking
/// whether to remove is the caller's to do, and by the time this runs the answer is yes.
///
/// A symbolic link is unlinked and not followed: removing a link is removing the link.
///
/// # Errors
///
/// Renders [`ErrorFsOpsArguments`] when no path was named, and [`ErrorFsOpsFailed`] when the
/// filesystem refused the removal.
#[command(node = "fs-ops.rm")]
pub fn fs_ops_rm(args: EntryFsOpsRm) -> Next {
    let picked = args
        .pick_or_route(&arg![PathBuf], || {
            ErrorFsOpsArguments {
                verb: "rm",
                needs_to: false,
            }
            .into()
        })
        .to_result();

    let path = match picked {
        Ok(path) => path,
        Err(next) => return next,
    };

    StateFsOpsRm { path }.into()
}

/// The state of removing one path.
#[derive(Grouped)]
pub struct StateFsOpsRm {
    /// What is being removed.
    path: PathBuf,
}

#[chain(routeify)]
pub fn handle_fs_ops_rm(state: StateFsOpsRm) -> Next {
    // Taken apart rather than borrowed from, so that the state is consumed rather than only read.
    let StateFsOpsRm { path } = state;

    match cmd_fs_ops::remove_at(&path) {
        Ok(()) => ResultFsOps {
            verb: "rm",
            from: path.display().to_string(),
            to: None,
        }
        .into(),
        Err(error) => ErrorFsOpsFailed {
            verb: "rm",
            cause: error.to_string(),
        }
        .into(),
    }
}

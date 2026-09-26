//! The `rola fs-ops cp` command: put a copy of a path at another.

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
pub fn help_fs_ops_cp(_: EntryFsOpsCp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fs_ops.cp_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFsOpsCp)]
pub fn desc_fs_ops_cp() -> Description {
    t!("fs_ops.cp_description").to_string().into()
}

/// Copies `FROM` to `TO`, `FROM` being left where it is.
///
/// `TO` is the path the copy is to have, not the directory it is to be put in: what a name already
/// taken there means was settled before this was called. Either path may name a file or a directory,
/// and copying a directory copies everything under it — content and structure, which is all that is
/// claimed: permissions, ownership and times are the destination's own, as the system gives them for
/// anything newly written.
///
/// # Errors
///
/// Renders [`ErrorFsOpsArguments`] when both paths were not named, and [`ErrorFsOpsFailed`] when the
/// filesystem refused the copy.
#[command(node = "fs-ops.cp")]
pub fn fs_ops_cp(args: EntryFsOpsCp) -> Next {
    let picked = args
        .pick_or_route(&arg![PathBuf], || {
            ErrorFsOpsArguments {
                verb: "cp",
                needs_to: true,
            }
            .into()
        })
        .pick_or_route(&arg![PathBuf], || {
            ErrorFsOpsArguments {
                verb: "cp",
                needs_to: true,
            }
            .into()
        })
        .to_result();

    let (from, to) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateFsOpsCp { from, to }.into()
}

/// The state of copying one path to another.
#[derive(Grouped)]
pub struct StateFsOpsCp {
    /// What is being copied.
    from: PathBuf,
    /// Where the copy is being put.
    to: PathBuf,
}

#[chain(routeify)]
pub fn handle_fs_ops_cp(state: StateFsOpsCp) -> Next {
    // Taken apart rather than borrowed from, so that the state is consumed rather than only read.
    let StateFsOpsCp { from, to } = state;

    match cmd_fs_ops::copy_to(&from, &to) {
        Ok(()) => ResultFsOps {
            verb: "cp",
            from: from.display().to_string(),
            to: Some(to.display().to_string()),
        }
        .into(),
        Err(error) => ErrorFsOpsFailed {
            verb: "cp",
            cause: error.to_string(),
        }
        .into(),
    }
}

//! The `rola fs-ops rm` command: take a path away.

use std::path::{Path, PathBuf};

use librorolala::layout::{Layout, LayoutPath};
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::cmd_fs_ops::{
    self, ErrorFsOpsArguments, ErrorFsOpsBlocked, ErrorFsOpsFailed, FsOpsBlock, ResultFsOps,
};
use crate::complete::typing_flag;
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

/// Completes what `rola fs-ops rm` can be given next.
///
/// The path is the filesystem's to answer; there are no flags to offer.
#[completion(EntryFsOpsRm)]
pub fn complete_fs_ops_rm(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    Suggest::file_comp()
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
pub fn handle_fs_ops_rm(state: StateFsOpsRm, account: &mut LazyRes<ResCurrentAccount>) -> Next {
    // Taken apart rather than borrowed from, so that the state is consumed rather than only read.
    let StateFsOpsRm { path } = state;

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return ErrorFsOpsFailed {
                verb: "rm",
                cause: error.to_string(),
            }
            .into();
        }
    };

    // What the removal is about in the Layout being worked in, when there is one. It is the Workspace
    // the path itself sits in, and never the one the run was made in: a path in no Workspace has
    // nothing here to keep in step.
    let me = account.get_ref().must_bind().ok();
    let workspace = cmd_fs_ops::workspace_of(&cwd, &path);
    let plan = workspace
        .as_ref()
        .and_then(|held| plan_rm(held, &cwd, &path).map(|(layout, named)| (held, layout, named)));

    // What is not this run's to change is refused before anything is removed: a removal takes the
    // entry with it, and one over another account's entry is the conflict rather than the fix.
    if let Some((held, layout, named)) = &plan {
        for (path, id) in named {
            if let Some(owner) = cmd_fs_ops::held_elsewhere(held, layout, *id, me.as_deref()) {
                return ErrorFsOpsBlocked {
                    path: path.as_str().to_owned(),
                    kind: FsOpsBlock::Held(owner),
                }
                .into();
            }
        }
    }

    // The filesystem first, since that is what was asked for, and the Layout follows it. A removal
    // that could not be written leaves the path the Layout names unheld, which is what `rola align`
    // settles rather than something this run can put back.
    if let Err(error) = cmd_fs_ops::remove_at(&path) {
        return ErrorFsOpsFailed {
            verb: "rm",
            cause: error.to_string(),
        }
        .into();
    }

    if let Some((_, layout, named)) = plan {
        for (_, id) in named {
            if let Err(error) = layout.remove_entry(id) {
                return crate::layout::failed(&error);
            }
        }
    }

    ResultFsOps {
        verb: "rm",
        from: path.display().to_string(),
        to: None,
    }
    .into()
}

/// What the removal is about in the Layout being worked in, when the Layout names anything it takes.
///
/// Nothing is returned for a run whose Layout names none of it: the filesystem alone is then what the
/// operation is.
fn plan_rm(held: &Workspace, cwd: &Path, path: &Path) -> Option<(Layout, Vec<(LayoutPath, Uuid)>)> {
    let layout = cmd_fs_ops::working(held)?;
    let touched = cmd_fs_ops::touched(&layout, held, cwd, path);
    let named = cmd_fs_ops::named(&layout, &touched);

    (!named.is_empty()).then_some((layout, named))
}

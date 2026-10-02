//! The `rola fs-ops mv` command: put a path at another.

// What a move is about — the two paths, the Workspace each sits in, the entries it renames, and the
// merge a variant file carries with it — is read in one place, so the order the two are kept in step
// reads off the handler rather than being assembled from helpers.
#![allow(clippy::too_many_lines)]

use std::path::{Path, PathBuf};

use librorolala::layout::{Layout, LayoutPath};
use librorolala::workspace::{Merging, Workspace};
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
use rorolala_utils_location::Locate as _;
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
pub fn help_fs_ops_mv(_: EntryFsOpsMv, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fs_ops.mv_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFsOpsMv)]
pub fn desc_fs_ops_mv() -> Description {
    t!("fs_ops.mv_description").to_string().into()
}

/// Completes what `rola fs-ops mv` can be given next.
///
/// Both words name where something sits, so the filesystem answers them; there are no flags to
/// offer.
#[completion(EntryFsOpsMv)]
pub fn complete_fs_ops_mv(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    Suggest::file_comp()
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
pub fn handle_fs_ops_mv(state: StateFsOpsMv, account: &mut LazyRes<ResCurrentAccount>) -> Next {
    // Taken apart rather than borrowed from, so that the state is consumed rather than only read: what the
    // operation needs is the two paths, and keeping the whole of a state alive for them would be keeping
    // more than it is used.
    let StateFsOpsMv { from, to } = state;

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return ErrorFsOpsFailed {
                verb: "mv",
                cause: error.to_string(),
            }
            .into();
        }
    };

    let me = account.get_ref().must_bind().ok();

    // The Workspace each path sits in, looked for from the paths themselves and never from where the
    // run was made: a run made outside any Workspace still keeps the tree its paths are in step. What
    // the move changes is the source's Layout, and the two being the same Workspace is what says the
    // paths stay in it — so the entries are renamed rather than let go of.
    let source = cmd_fs_ops::workspace_of(&cwd, &from);
    let target = cmd_fs_ops::workspace_of(&cwd, &to);
    let same = source
        .as_ref()
        .zip(target.as_ref())
        .is_some_and(|(source, target)| source.get_root() == target.get_root());

    // A variant file is not a path the Layout names, so it is no part of the plan below. Moving one
    // keeps its merge: what is written down is the place it now lies at, relative to the file it is
    // for.
    let variant_place = match &source {
        Some(held) => match plan_variant_mv(held, &cwd, &from, &to, same) {
            Ok(place) => place,
            Err(next) => return next,
        },
        None => None,
    };

    let plan = source
        .as_ref()
        .and_then(|held| plan_mv(held, &cwd, &from, &to, same).map(|plan| (held, plan)));

    // What is not this run's to change is refused before anything moves, and so is a destination that
    // already names an entry: a move that took it would leave two names for one path, which is the
    // replacement the caller settled being made where the Layout cannot hold it.
    if let Some((held, plan)) = &plan {
        for (path, id) in &plan.named {
            if let Some(owner) = cmd_fs_ops::held_elsewhere(held, &plan.layout, *id, me.as_deref())
            {
                return ErrorFsOpsBlocked {
                    path: path.as_str().to_owned(),
                    kind: FsOpsBlock::Held(owner),
                }
                .into();
            }
        }

        if let Some(target) = &plan.target {
            for (path, _) in &plan.named {
                let moved = cmd_fs_ops::moved(&plan.source, target, path);

                if moved != *path && plan.layout.id_of(&moved).is_some() {
                    return ErrorFsOpsBlocked {
                        path: moved.as_str().to_owned(),
                        kind: FsOpsBlock::Named,
                    }
                    .into();
                }
            }
        }
    }

    // The filesystem first, since that is what was asked for, and the Layout follows it. A Layout that
    // could not be written leaves the two disagreeing, which is what `rola align` settles rather than
    // something this run can put back.
    if let Err(error) = cmd_fs_ops::move_to(&from, &to) {
        return ErrorFsOpsFailed {
            verb: "mv",
            cause: error.to_string(),
        }
        .into();
    }

    // A moved variant file is written down where it now lies, so the merge is not left looking for it
    // where the convention used to put it.
    if let Some((waiting, place)) = variant_place
        && let Some(held) = &source
    {
        let mut merging = Merging::read(held.get_root());

        if merging.relocate(waiting, Some(place))
            && let Err(error) = merging.write(held.get_root())
        {
            return ErrorFsOpsFailed {
                verb: "mv",
                cause: error.to_string(),
            }
            .into();
        }
    }

    if let Some((held, plan)) = plan {
        // A file a variant is waiting for takes that variant file with it, since both lie in the
        // same place by the convention; the merge is work in this tree and moves with it.
        let merging = Merging::read(held.get_root());

        for (path, id) in plan.named {
            // A move out of the Workspace leaves the Layout naming a path that is not there, so the
            // entry is let go of instead: what left is not something this Layout still holds.
            let outcome = plan.target.as_ref().map_or_else(
                || plan.layout.remove_entry(id).map(|()| None),
                |target| {
                    let moved = cmd_fs_ops::moved(&plan.source, target, &path);

                    plan.layout.move_path(&path, &moved).map(|()| Some(moved))
                },
            );

            match outcome {
                Ok(Some(moved)) => {
                    if let Err(error) = merging.follow(held.get_root(), id, &path, &moved) {
                        return ErrorFsOpsFailed {
                            verb: "mv",
                            cause: error.to_string(),
                        }
                        .into();
                    }

                    if let Err(next) = remember(&plan.layout, held, &moved) {
                        return next;
                    }
                }
                Ok(None) => {}
                Err(error) => return crate::layout::failed(&error),
            }
        }
    }

    ResultFsOps {
        verb: "mv",
        from: from.display().to_string(),
        to: Some(to.display().to_string()),
    }
    .into()
}

/// Writes down that the moved path holds what the Layout now names there.
fn remember(layout: &Layout, held: &Workspace, path: &LayoutPath) -> Result<(), Next> {
    if let Err(cause) = crate::checkout::remember(layout, held.get_root(), path.as_str()) {
        return Err(ErrorFsOpsFailed { verb: "mv", cause }.into());
    }

    Ok(())
}

/// What the move is about in the Layout being worked in.
struct MovePlan {
    /// The Layout being worked in.
    layout: Layout,
    /// The paths the move takes in, with the entries they name.
    named: Vec<(LayoutPath, Uuid)>,
    /// Where the move starts, as a Layout path.
    source: String,
    /// Where it lands, or nothing when it lands outside the Workspace, which no Layout names.
    target: Option<String>,
}

/// What the move is about in the Layout being worked in, when the Layout names anything it takes.
///
/// Nothing is returned for a run whose Layout names none of it: the filesystem alone is then what the
/// operation is.
fn plan_mv(held: &Workspace, cwd: &Path, from: &Path, to: &Path, same: bool) -> Option<MovePlan> {
    let layout = cmd_fs_ops::working(held)?;
    let touched = cmd_fs_ops::touched(&layout, held, cwd, from);
    let named = cmd_fs_ops::named(&layout, &touched);

    if named.is_empty() {
        return None;
    }

    let source = cmd_fs_ops::inside(held, cwd, from)?;
    let target = same.then(|| cmd_fs_ops::inside(held, cwd, to)).flatten();

    Some(MovePlan {
        layout,
        named,
        source,
        target,
    })
}

/// Where a variant file is being moved to, when the path being moved is one.
///
/// A variant file is not a path the Layout names, so the move plan says nothing about it. What the
/// merge needs is the file it is for and the place it now lies at, relative to that file's
/// directory. A move out of the Workspace is refused rather than allowed: a place is read from the
/// Workspace's own root, and a variant file outside it is one no merge can say where is.
fn plan_variant_mv(
    held: &Workspace,
    cwd: &Path,
    from: &Path,
    to: &Path,
    same: bool,
) -> Result<Option<(Uuid, PathBuf)>, Next> {
    let Some(layout) = cmd_fs_ops::working(held) else {
        return Ok(None);
    };
    let Some(from_text) = cmd_fs_ops::inside(held, cwd, from) else {
        return Ok(None);
    };
    let Ok(from_path) = LayoutPath::new(&from_text) else {
        return Ok(None);
    };

    let merging = Merging::read(held.get_root());
    let Some(pending) = merging.variant_of(&layout, &from_path) else {
        return Ok(None);
    };
    let Some(target) = layout.path_of(pending.target()) else {
        return Ok(None);
    };

    let Some(to_path) = same
        .then(|| cmd_fs_ops::inside(held, cwd, to))
        .flatten()
        .and_then(|text| LayoutPath::new(&text).ok())
    else {
        return Err(ErrorFsOpsBlocked {
            path: from.display().to_string(),
            kind: FsOpsBlock::Variant,
        }
        .into());
    };

    Ok(Some((
        pending.target(),
        Merging::place_of(&target, &to_path),
    )))
}

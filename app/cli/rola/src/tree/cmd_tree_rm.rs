//! The `rola tree rm` command: stage the removal of work from a tree.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use mingling::{
    Grouped, LazyRes, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResForce, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::tree::cmd_tree::ErrorTreeShouldInWorkspace;

/// Stage the removal but leave the work where it is.
const ARG_CACHED: PickerArg<'static, Flag> = arg![cached: Flag];

/// Take a directory and everything under it.
const ARG_RECURSIVE: PickerArg<'static, Flag> = arg![recursive: Flag];

#[help(buffer)]
pub fn help_tree_rm(_: EntryTreeRm, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.rm.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeRm)]
pub fn desc_tree_rm() -> Description {
    t!("tree.rm.description").to_string().into()
}

/// Completes what `rola tree rm` can be given next.
#[completion(EntryTreeRm)]
pub fn complete_tree_rm(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_CACHED: t!("tree.rm.complete.cached"),
                ARG_RECURSIVE: t!("tree.rm.complete.recursive"),
            },
        );
    }

    Suggest::file_comp()
}

/// Stages the removal of work from a tree
///
/// Each `PATH` names a file, a link or a directory the tree holds. What is staged is the removal:
/// the path leaves the staged structure, and `rola track` records a version without it.
///
/// A file is removed from the work as well, unless `--cached` leaves it in place. A directory is
/// only taken with `--recursive`, and a directory holding a tree of its own or the Workspace's data
/// is refused rather than taken with them.
///
/// A file with work that was never staged is refused unless `--force` says to drop it, and a path a
/// merge left in conflict is resolved by removing it, taking the copy beside it as well.
///
/// # Errors
///
/// Renders [`ErrorTreeRmArgument`] when a path cannot be removed, [`ErrorTreeRmDirty`] when it
/// holds work that was never staged, [`ErrorTreeShouldInWorkspace`] when the run is not inside a
/// Workspace, and [`ErrorTreeRmFailed`] when the staging area or the work cannot be written.
#[command(node = "tree.rm", entry = EntryTreeRm)]
pub fn tree_rm(args: EntryTreeRm, force: &ResForce) -> Next {
    let picked = args
        .pick(ARG_CACHED)
        .pick(ARG_RECURSIVE)
        .pick_or_route(arg![Vec<String>], || ErrorTreeRmArgument.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (cached, recursive, paths, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree rm") {
        return next;
    }

    StateTreeRm {
        cached: matches!(cached, Flag::Active),
        recursive: matches!(recursive, Flag::Active),
        force: **force,
        paths,
    }
    .into()
}

/// The state removing work starts in.
#[derive(Grouped)]
pub struct StateTreeRm {
    /// Whether the work is left where it is.
    cached: bool,
    /// Whether a directory is taken with what is under it.
    recursive: bool,
    /// Whether work that was never staged is dropped.
    force: bool,
    /// The paths named, as they were written.
    paths: Vec<String>,
}

#[chain(routeify)]
pub fn handle_tree_rm(state: StateTreeRm, workspace: &mut LazyRes<ResWorkspace>) -> Next {
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let StateTreeRm {
        cached,
        recursive,
        force,
        paths,
    } = state;

    let Some(first) = paths.first() else {
        return ErrorTreeRmArgument.into();
    };
    let Ok(cwd) = std::env::current_dir() else {
        return ErrorTreeRmArgument.into();
    };

    match librorolala::write::tree_remove::remove_paths_for_workspace(
        held,
        &cwd,
        first,
        &paths,
        librorolala::write::tree_remove::RemoveOptions {
            cached,
            recursive,
            force,
        },
    ) {
        Ok(removal) => ResultTreeRm {
            actions: removal
                .paths
                .iter()
                .map(|path| format!("{} {}", t!("tree.rm.deleted").trim(), path))
                .collect(),
            cached: removal.cached,
        }
        .into(),
        Err(librorolala::write::WriteError::InvalidPath { .. }) => ErrorTreeRmArgument.into(),
        Err(librorolala::write::WriteError::Precondition { what, .. }) => {
            ErrorTreeRmDirty { path: what }.into()
        }
        Err(error) => ErrorTreeRmFailed::new(error.to_string()).into(),
    }
}

/// Result: work of a tree was staged as removed.
#[derive(Grouped)]
pub struct ResultTreeRm {
    /// What changed, a line each.
    actions: Vec<String>,
    /// Whether the work was left in place.
    cached: bool,
}

#[renderer(buffer)]
pub fn render_result_tree_rm(result: ResultTreeRm, ec: &mut ResExitCode) {
    for action in &result.actions {
        r_println!("{action}");
    }

    if result.cached {
        r_eprintln!("{}", t!("tree.rm.cached_note").trim());
    }

    let _ = ec;
}

/// Error: a path cannot be removed.
#[derive(Grouped)]
pub struct ErrorTreeRmArgument;

impl Failure for ErrorTreeRmArgument {
    fn name(&self) -> &'static str {
        "error_tree_rm_argument"
    }

    fn reason(&self) -> String {
        t!("tree.rm.err_argument").trim().to_string()
    }
}

failure!(ErrorTreeRmArgument);

#[renderer(buffer)]
pub fn render_error_tree_rm_argument(_: ErrorTreeRmArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.rm.err_argument").trim()));
    r_eprintln!("{}", help_line!(t!("tree.rm.err_argument_help").trim()));
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: a path holds work that was never staged.
#[derive(Grouped)]
pub struct ErrorTreeRmDirty {
    /// The path.
    path: String,
}

impl Failure for ErrorTreeRmDirty {
    fn name(&self) -> &'static str {
        "error_tree_rm_dirty"
    }

    fn reason(&self) -> String {
        t!("tree.rm.err_dirty", path = self.path).trim().to_string()
    }
}

failure!(ErrorTreeRmDirty);

#[renderer(buffer)]
pub fn render_error_tree_rm_dirty(error: ErrorTreeRmDirty, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.rm.err_dirty_help").trim()));
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: staging the removal failed.
#[derive(Grouped)]
pub struct ErrorTreeRmFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeRmFailed {
    /// The failure of staging a removal, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeRmFailed {
    fn name(&self) -> &'static str {
        "error_tree_rm_failed"
    }

    fn reason(&self) -> String {
        t!("tree.rm.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeRmFailed);

#[renderer(buffer)]
pub fn render_error_tree_rm_failed(error: ErrorTreeRmFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.rm.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

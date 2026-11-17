//! The `rola tree add` command: stage work of a tree.

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
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::strip_written;
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::tree::cmd_tree::{ErrorTreeNotATree, ErrorTreeShouldInWorkspace};

#[help(buffer)]
pub fn help_tree_add(_: EntryTreeAdd, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.add.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeAdd)]
pub fn desc_tree_add() -> Description {
    t!("tree.add.description").to_string().into()
}

/// Completes what `rola tree add` can be given next.
#[completion(EntryTreeAdd)]
pub fn complete_tree_add(ctx: ShellContext) -> Suggest {
    if crate::complete::typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                crate::ignore::ARG_INCLUDE_IGNORED: t!("ignore.complete.include_ignored"),
            },
        );
    }

    Suggest::file_comp()
}

/// Stages work of a tree
///
/// Each `PATH` names a file, a symbolic link or a directory inside the tree whose work is to be
/// staged. A directory is taken recursively, with the tree's own `.rola-tree`, the Workspace's
/// `.rola`, conflict copies and nested trees left out, and what is ignored is left out unless
/// `--include-ignored` asks for it.
///
/// A `.` at the tree's root stages every change: what appeared, what changed, and what a recorded
/// path no longer holds. A path a merge left in conflict is resolved by staging it, and the conflict
/// copy beside it is removed.
///
/// Staging is what a commit records: `rola track` records the staged structure, and work that is not
/// staged stays out of the version.
///
/// # Errors
///
/// Renders [`ErrorTreeAddArgument`] when a path is not inside the tree or is ignored,
/// [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorTreeAddFailed`] when the work or the staging area cannot be read or written.
#[command(node = "tree.add", entry = EntryTreeAdd)]
pub fn tree_add(args: EntryTreeAdd) -> Next {
    let picked = args
        .pick(crate::ignore::ARG_INCLUDE_IGNORED)
        .pick_or_route(arg![Vec<String>], || ErrorTreeAddArgument.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (include_ignored, paths, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree add") {
        return next;
    }

    StateTreeAdd {
        include_ignored: matches!(include_ignored, mingling::picker::value::Flag::Active),
        paths,
    }
    .into()
}

/// The state staging work starts in.
#[derive(Grouped)]
pub struct StateTreeAdd {
    /// Whether the ignore rules are switched off.
    include_ignored: bool,
    /// The paths named, as they were written.
    paths: Vec<String>,
}

#[chain(routeify)]
pub fn handle_tree_add(state: StateTreeAdd, workspace: &mut LazyRes<ResWorkspace>) -> Next {
    let StateTreeAdd {
        include_ignored,
        paths,
    } = state;

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };
    let Some(first) = paths.first() else {
        return ErrorTreeAddArgument.into();
    };
    let Ok(cwd) = std::env::current_dir() else {
        return ErrorTreeAddArgument.into();
    };

    match librorolala::write::tree::stage_paths(held, &cwd, first, &paths, include_ignored) {
        Ok(changes) => ResultTreeAdd {
            actions: changes
                .iter()
                .map(|change| {
                    let word = match change.kind {
                        librorolala::write::tree::AddChangeKind::Added => {
                            t!("tree.add.added")
                        }
                        librorolala::write::tree::AddChangeKind::Updated => {
                            t!("tree.add.updated")
                        }
                        librorolala::write::tree::AddChangeKind::Deleted => {
                            t!("tree.add.deleted")
                        }
                    };
                    format!("{} {}", word.trim(), change.path)
                })
                .collect(),
        }
        .into(),
        Err(librorolala::write::WriteError::NotATree { path }) => {
            ErrorTreeNotATree::new(path).into()
        }
        Err(librorolala::write::WriteError::InvalidPath { .. }) => ErrorTreeAddArgument.into(),
        Err(librorolala::write::WriteError::NoStore) => {
            ErrorTreeAddFailed::new(t!("tree.err_no_store").trim().to_string()).into()
        }
        Err(librorolala::write::WriteError::NoIndex) => {
            ErrorTreeAddFailed::new(t!("tree.err_no_index").trim().to_string()).into()
        }
        Err(error) => ErrorTreeAddFailed::new(error.to_string()).into(),
    }
}

/// Result: work of a tree was staged.
#[derive(Grouped)]
pub struct ResultTreeAdd {
    /// What changed, a line each.
    actions: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_tree_add(result: ResultTreeAdd, ec: &mut ResExitCode) {
    for action in &result.actions {
        r_println!("{action}");
    }

    let _ = ec;
}

/// Error: a path cannot be staged.
#[derive(Grouped)]
pub struct ErrorTreeAddArgument;

impl Failure for ErrorTreeAddArgument {
    fn name(&self) -> &'static str {
        "error_tree_add_argument"
    }

    fn reason(&self) -> String {
        t!("tree.add.err_argument").trim().to_string()
    }
}

failure!(ErrorTreeAddArgument);

#[renderer(buffer)]
pub fn render_error_tree_add_argument(_: ErrorTreeAddArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.add.err_argument").trim()));
    r_eprintln!("{}", help_line!(t!("tree.add.err_argument_help").trim()));
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: staging the work of a tree failed.
#[derive(Grouped)]
pub struct ErrorTreeAddFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeAddFailed {
    /// The failure of staging a tree's work, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeAddFailed {
    fn name(&self) -> &'static str {
        "error_tree_add_failed"
    }

    fn reason(&self) -> String {
        t!("tree.add.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeAddFailed);

#[renderer(buffer)]
pub fn render_error_tree_add_failed(error: ErrorTreeAddFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.add.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

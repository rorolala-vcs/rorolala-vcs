//! The `rola tree reset` command: put the staging area back to the recorded version.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use mingling::{
    Grouped, LazyRes, Suggest,
    macros::{arg, buffer, command, completion, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::tree::cmd_tree::{ErrorTreeNotATree, ErrorTreeShouldInWorkspace};

#[help(buffer)]
pub fn help_tree_reset(_: EntryTreeReset, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.reset.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeReset)]
pub fn desc_tree_reset() -> Description {
    t!("tree.reset.description").to_string().into()
}

/// Completes what `rola tree reset` can be given next.
///
/// The paths are the filesystem's to answer, and there is nothing else the command takes.
#[completion(EntryTreeReset)]
pub fn complete_tree_reset() -> Suggest {
    Suggest::file_comp()
}

/// Puts the staging area of a tree back to the recorded version
///
/// With no `PATH`, the whole staging area is reset: its structure, its description, its conflicts
/// and the content staged in it. With paths, only what they name is reset, and every other staged
/// path is left as it was. The work is never touched: what was reset shows up in `rola tree status`
/// as work that is not staged.
///
/// A `PATH` is a file, a link or a directory the tree records or has staged. A directory resets
/// everything under it. A path inside a conflict is taken out of the conflict list, and the copy
/// beside it goes with it.
///
/// # Errors
///
/// Renders [`ErrorTreeResetArgument`] when a path names nothing the tree records or has staged,
/// [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorTreeResetFailed`] when the staging area cannot be read or written.
#[command(node = "tree.reset", entry = EntryTreeReset)]
pub fn tree_reset(args: EntryTreeReset, workspace: &mut LazyRes<ResWorkspace>) -> Next {
    let paths = args.pick(arg![Vec<String>]).unwrap_or_default();

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };
    let Ok(cwd) = std::env::current_dir() else {
        return ErrorTreeResetArgument.into();
    };
    let target = paths.first().cloned().unwrap_or_default();

    match librorolala::write::tree_reset::reset(held, &cwd, &target, &paths) {
        Ok(result) => ResultTreeReset {
            whole: result.whole,
            paths: result.paths,
        }
        .into(),
        Err(librorolala::write::WriteError::InvalidPath { .. }) => ErrorTreeResetArgument.into(),
        Err(librorolala::write::WriteError::NotATree { path }) => {
            ErrorTreeNotATree::new(path).into()
        }
        Err(librorolala::write::WriteError::NoStore) => {
            ErrorTreeResetFailed::new(t!("tree.err_no_store").trim().to_string()).into()
        }
        Err(librorolala::write::WriteError::NoIndex) => {
            ErrorTreeResetFailed::new(t!("tree.err_no_index").trim().to_string()).into()
        }
        Err(error) => ErrorTreeResetFailed::new(error.to_string()).into(),
    }
}

/// Result: staged work was put back to the recorded version.
#[derive(Grouped)]
pub struct ResultTreeReset {
    /// Whether the whole staging area was reset rather than the paths named.
    whole: bool,
    /// The paths named, a line each when the reset is not whole.
    paths: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_tree_reset(result: ResultTreeReset, ec: &mut ResExitCode) {
    if result.whole {
        r_println!("{}", t!("tree.reset.cleared").trim());
    } else {
        for path in &result.paths {
            r_println!("{}", t!("tree.reset.action", path = path).trim());
        }
    }

    let _ = ec;
}

/// Error: a path names nothing the tree records or has staged.
#[derive(Grouped)]
pub struct ErrorTreeResetArgument;

impl Failure for ErrorTreeResetArgument {
    fn name(&self) -> &'static str {
        "error_tree_reset_argument"
    }

    fn reason(&self) -> String {
        t!("tree.reset.err_argument").trim().to_string()
    }
}

failure!(ErrorTreeResetArgument);

#[renderer(buffer)]
pub fn render_error_tree_reset_argument(_: ErrorTreeResetArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.reset.err_argument").trim()));
    r_eprintln!("{}", help_line!(t!("tree.reset.err_argument_help").trim()));
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: the staging area cannot be reset.
#[derive(Grouped)]
pub struct ErrorTreeResetFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeResetFailed {
    /// The failure of resetting a staging area, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeResetFailed {
    fn name(&self) -> &'static str {
        "error_tree_reset_failed"
    }

    fn reason(&self) -> String {
        t!("tree.reset.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeResetFailed);

#[renderer(buffer)]
pub fn render_error_tree_reset_failed(error: ErrorTreeResetFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.reset.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

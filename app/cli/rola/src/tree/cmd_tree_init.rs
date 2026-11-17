//! The `rola tree init` command: make a directory a tree.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use std::io::IsTerminal as _;

use mingling::{
    Grouped, LazyRes, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResForce, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};

use crate::Next;
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::tree::cmd_tree::ErrorTreeShouldInWorkspace;
use rust_i18n::t;

#[help(buffer)]
pub fn help_tree_init(_: EntryTreeInit, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.init.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeInit)]
pub fn desc_tree_init() -> Description {
    t!("tree.init.description").to_string().into()
}

/// Completes what `rola tree init` can be given next.
///
/// The directory is the filesystem's to answer, and `--force` is a global flag rather than this
/// command's, so there is nothing else to offer.
#[completion(EntryTreeInit)]
pub fn complete_tree_init() -> Suggest {
    Suggest::file_comp()
}

/// Makes a directory a tree
///
/// `DIR` is the directory to make a tree of, defaulting to where the run was made. The directory
/// must exist and be inside the Workspace, and a directory that is already a tree is refused.
///
/// Making a tree of the Workspace root, or of a directory under which files are recorded as files,
/// is not reversible: those entries stop being files and their recorded content becomes the entries
/// of the tree's staged structure. The run asks before doing it, and `--force` answers yes for a run
/// that cannot be asked. What is recorded is the content each file was recorded with, not what the
/// work holds now, so anything unrecorded stays unrecorded and is shown as not staged.
///
/// # Errors
///
/// Renders [`ErrorTreeInitArgument`] when the directory cannot be a tree, [`ErrorTreeCancelled`]
/// when the confirmation is refused, [`ErrorTreeShouldInWorkspace`] when the run is not inside a
/// Workspace, and [`ErrorTreeInitFailed`] when the tree cannot be made.
#[command(node = "tree.init", entry = EntryTreeInit)]
pub fn tree_init(args: EntryTreeInit, force: &ResForce) -> Next {
    let picked = args.pick(arg![Option<String>]).pick(&UNMATCHED).to_result();
    let (target, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree init") {
        return next;
    }

    StateTreeInit {
        force: **force,
        target,
    }
    .into()
}

/// The state a tree's making starts in.
#[derive(Grouped)]
pub struct StateTreeInit {
    /// Whether the confirmation is answered yes.
    force: bool,
    /// The directory named, as it was written.
    target: Option<String>,
}

#[chain(routeify)]
pub fn handle_tree_init(state: StateTreeInit, workspace: &mut LazyRes<ResWorkspace>) -> Next {
    let StateTreeInit { force, target } = state;

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };
    let Ok(cwd) = std::env::current_dir() else {
        return ErrorTreeInitArgument.into();
    };
    let target = target.as_deref().unwrap_or_default();

    let plan = match librorolala::write::tree_init::plan_at(held, &cwd, target) {
        Ok(plan) => plan,
        Err(librorolala::write::WriteError::InvalidPath { .. }) => {
            return ErrorTreeInitArgument.into();
        }
        Err(error) => return ErrorTreeInitFailed::new(error.to_string()).into(),
    };

    if (plan.is_root || !plan.collapse.is_empty()) && !force && !confirmed(plan.is_root) {
        return ErrorTreeCancelled.into();
    }

    match librorolala::write::tree_init::init(held, &cwd, target, true) {
        Ok(done) => ResultTreeInit {
            path: done.path,
            collapsed: done.collapsed,
            is_root: done.is_root,
        }
        .into(),
        Err(librorolala::write::WriteError::InvalidPath { .. }) => ErrorTreeInitArgument.into(),
        Err(librorolala::write::WriteError::NoIndex) => {
            ErrorTreeInitFailed::new(t!("tree.err_no_index").trim().to_string()).into()
        }
        Err(error) => ErrorTreeInitFailed::new(error.to_string()).into(),
    }
}

/// Whether the run is to go ahead, asking on a terminal and answering no when there is none.
fn confirmed(is_root: bool) -> bool {
    let prompt = if is_root {
        t!("tree.init.confirm_root")
    } else {
        t!("tree.init.confirm_collapse")
    };
    eprintln!("{}", prompt.trim());

    if !std::io::stdin().is_terminal() {
        return false;
    }

    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }

    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Result: a directory was made a tree.
#[derive(Grouped)]
pub struct ResultTreeInit {
    /// The tree as a person knows it.
    path: String,
    /// How many recorded files became entries of the tree.
    collapsed: usize,
    /// Whether the tree is the Workspace root.
    is_root: bool,
}

#[renderer(buffer)]
pub fn render_result_tree_init(result: ResultTreeInit, ec: &mut ResExitCode) {
    if result.is_root {
        r_println!("{}", t!("tree.init.result_root", path = result.path).trim());
    } else {
        r_println!("{}", t!("tree.init.result", path = result.path).trim());
    }

    if result.collapsed > 0 {
        r_println!(
            "{}",
            t!(
                "tree.init.result_collapsed",
                count = result.collapsed.to_string()
            )
            .trim()
        );
    }

    let _ = ec;
}

/// Error: the directory cannot be made a tree.
#[derive(Grouped)]
pub struct ErrorTreeInitArgument;

impl Failure for ErrorTreeInitArgument {
    fn name(&self) -> &'static str {
        "error_tree_init_argument"
    }

    fn reason(&self) -> String {
        t!("tree.init.err_argument").trim().to_string()
    }
}

failure!(ErrorTreeInitArgument);

#[renderer(buffer)]
pub fn render_error_tree_init_argument(_: ErrorTreeInitArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.init.err_argument").trim()));
    r_eprintln!("{}", help_line!(t!("tree.init.err_argument_help").trim()));
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: the confirmation to make the tree was refused.
#[derive(Grouped)]
pub struct ErrorTreeCancelled;

impl Failure for ErrorTreeCancelled {
    fn name(&self) -> &'static str {
        "error_tree_cancelled"
    }

    fn reason(&self) -> String {
        t!("tree.init.err_cancelled").trim().to_string()
    }
}

failure!(ErrorTreeCancelled);

#[renderer(buffer)]
pub fn render_error_tree_cancelled(_: ErrorTreeCancelled, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.init.err_cancelled").trim()));
    r_eprintln!("{}", help_line!(t!("tree.init.err_cancelled_help").trim()));
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: the tree could not be made.
#[derive(Grouped)]
pub struct ErrorTreeInitFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeInitFailed {
    /// The failure of a tree's making, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeInitFailed {
    fn name(&self) -> &'static str {
        "error_tree_init_failed"
    }

    fn reason(&self) -> String {
        t!("tree.init.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeInitFailed);

#[renderer(buffer)]
pub fn render_error_tree_init_failed(error: ErrorTreeInitFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.init.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

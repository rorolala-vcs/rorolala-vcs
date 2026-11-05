//! The `rola tree` namespace: a directory versioned as one file.
//!
//! A tree is a directory holding `.rola-tree`, and what it holds is one unit of work: one version,
//! one holder, one place in the Layout. What is under it is the tree's own business, and what the
//! Workspace sees of it is one entry.
//!
//! The namespace is a folder of commands rather than one thing to do, so each of them is two words
//! deep and adding another is adding another word. Reaching `tree` with none of those words is a
//! question about the namespace, and it is answered with what the namespace holds.

use std::path::{Path, PathBuf};

use librorolala::tree::Tree;
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, Routable,
    macros::{buffer, chain, dispatcher, help, metadata, r_eprintln, renderer},
    metadata::Description,
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_location::Locate as _;
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{
    EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_ERR_TREE_NOT_A_TREE, EC_HELP,
};
use crate::failure::failure;

dispatcher!("tree", EntryTree);

#[help(buffer)]
pub fn help_tree(_: EntryTree, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTree)]
pub fn desc_tree() -> Description {
    t!("tree.description").to_string().into()
}

/// Names what `tree` holds.
///
/// A command the namespace has is matched before this is reached, so what arrives here is a run that
/// named none of them, or one it does not have. Both are a question about the namespace, and both
/// are answered the way [`help_tree`] answers.
#[chain]
pub fn handle_tree(_: EntryTree) -> Next {
    ResultTreeHelp.into()
}

/// Result: what `tree` holds was named.
#[derive(Grouped)]
pub struct ResultTreeHelp;

#[renderer(buffer)]
pub fn render_result_tree_help(_: ResultTreeHelp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.help")).trim());
    ec.exit_code = EC_HELP;
}

/// The tree a run names, or the one it is standing in.
///
/// `given` is a path as the run wrote it: a file or directory inside the tree, the tree's root, or
/// `.` for where the run was made. No name means the directory the run was made in.
///
/// # Errors
///
/// Renders [`ErrorTreeNotATree`] when nothing above the path is a tree, [`ErrorTreeShouldInWorkspace`]
/// when the run is not inside a Workspace, and [`ErrorTreeArgument`] when the path cannot be read.
pub fn locate(workspace: &Workspace, given: Option<&str>) -> Result<Tree, Next> {
    let root = workspace.get_root();
    let Ok(cwd) = std::env::current_dir() else {
        return Err(ErrorTreeArgument.to_chain());
    };
    let target = given.map_or_else(|| cwd.clone(), |given| resolve(&cwd, given));

    match Tree::locate(root, &target) {
        Ok(tree) => Ok(tree),
        Err(librorolala::tree::TreeError::NotATree(_)) => Err(ErrorTreeNotATree {
            path: target.display().to_string(),
        }
        .to_chain()),
        Err(other) => Err(ErrorTreeFailed::new(other.to_string()).to_chain()),
    }
}

/// `given` made absolute against `cwd`.
#[must_use]
pub fn resolve(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// The name a tree is shown by: its Layout path, with the Workspace root shown as `.`.
///
/// # Errors
///
/// Renders [`ErrorTreeFailed`] when the tree's path cannot be worked out.
pub fn shown(tree: &Tree) -> Result<String, Next> {
    tree.layout_path()
        .map(|path| {
            if path.is_root() {
                ".".to_owned()
            } else {
                path.as_str().to_owned()
            }
        })
        .map_err(|error| ErrorTreeFailed::new(error.to_string()).to_chain())
}

/// Error: the path is not inside a tree.
#[derive(Grouped)]
pub struct ErrorTreeNotATree {
    /// The path as it was named.
    path: String,
}

impl Failure for ErrorTreeNotATree {
    fn name(&self) -> &'static str {
        "error_tree_not_a_tree"
    }

    fn reason(&self) -> String {
        t!("tree.err_not_a_tree", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeNotATree);

#[renderer(buffer)]
pub fn render_error_tree_not_a_tree(error: ErrorTreeNotATree, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.err_not_a_tree_help").trim()));
    ec.exit_code = EC_ERR_TREE_NOT_A_TREE;
}

/// Error: the run is not inside a Workspace.
#[derive(Grouped)]
pub struct ErrorTreeShouldInWorkspace;

impl Failure for ErrorTreeShouldInWorkspace {
    fn name(&self) -> &'static str {
        "error_tree_should_in_workspace"
    }

    fn reason(&self) -> String {
        t!("tree.err_should_in_workspace").trim().to_string()
    }
}

failure!(ErrorTreeShouldInWorkspace);

#[renderer(buffer)]
pub fn render_error_tree_should_in_workspace(_: ErrorTreeShouldInWorkspace, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.err_should_in_workspace").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tree.err_should_in_workspace_help").trim())
    );
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: an argument a tree command cannot use.
#[derive(Grouped)]
pub struct ErrorTreeArgument;

impl Failure for ErrorTreeArgument {
    fn name(&self) -> &'static str {
        "error_tree_argument"
    }

    fn reason(&self) -> String {
        t!("tree.err_argument").trim().to_string()
    }
}

failure!(ErrorTreeArgument);

#[renderer(buffer)]
pub fn render_error_tree_argument(_: ErrorTreeArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tree.err_argument").trim()));
    r_eprintln!("{}", help_line!(t!("tree.err_argument_help").trim()));
    ec.exit_code = EC_ERR_TREE_ARGUMENT;
}

/// Error: a tree's files, store or Layout could not be read.
#[derive(Grouped)]
pub struct ErrorTreeFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeFailed {
    /// The failure of a tree operation that could not be made, for `cause`.
    pub fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeFailed {
    fn name(&self) -> &'static str {
        "error_tree_failed"
    }

    fn reason(&self) -> String {
        t!("tree.err_failed", cause = self.cause).trim().to_string()
    }
}

failure!(ErrorTreeFailed);

#[renderer(buffer)]
pub fn render_error_tree_failed(error: ErrorTreeFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

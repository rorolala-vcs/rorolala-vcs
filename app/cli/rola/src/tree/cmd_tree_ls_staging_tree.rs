//! The `rola tree ls-staging-tree` command: the entries a commit would record.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use librorolala::layout::Layout;
use librorolala::storage::RorolalaStorage;
use librorolala::tree::{Tree, TreeStructure};
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::EC_HELP;
use crate::layout::chosen;
use crate::tree::cmd_tree::{ErrorTreeFailed, ErrorTreeShouldInWorkspace, locate};
use crate::tree::cmd_tree_ls_tree::rows;

#[help(buffer)]
pub fn help_tree_ls_staging_tree(_: EntryTreeLsStagingTree, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.ls_staging_tree.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeLsStagingTree)]
pub fn desc_tree_ls_staging_tree() -> Description {
    t!("tree.ls_staging_tree.description").to_string().into()
}

/// Completes what `rola tree ls-staging-tree` can be given next.
#[completion(EntryTreeLsStagingTree)]
pub fn complete_tree_ls_staging_tree(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(&ctx, suggest!());
    }

    Suggest::file_comp()
}

/// Lists the entries a commit of the tree would record
///
/// What is listed is the staged structure, which is the recorded version with what was added and
/// removed since. A tree with nothing staged is listed as the version it records, and the run is
/// told that on stderr, since what it asked to see is not there yet.
///
/// `TREE` names the tree: its root, a path inside it, or `.` for where the run was made. `--json`
/// answers with the same entries as data.
///
/// # Errors
///
/// Renders [`ErrorTreeNotATree`](crate::tree::cmd_tree::ErrorTreeNotATree) when the path is not
/// inside a tree, [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorTreeFailed`] when the tree or the Layout cannot be read.
#[command(node = "tree.ls-staging-tree", entry = EntryTreeLsStagingTree)]
pub fn tree_ls_staging_tree(args: EntryTreeLsStagingTree) -> Next {
    let picked = args.pick(arg![Option<String>]).pick(&UNMATCHED).to_result();
    let (target, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree ls-staging-tree") {
        return next;
    }

    StateTreeLsStagingTree { target }.into()
}

/// The state a listing of the staged structure starts in.
#[derive(Grouped)]
pub struct StateTreeLsStagingTree {
    /// The tree named, as it was written.
    target: Option<String>,
}

#[chain(routeify)]
pub fn handle_tree_ls_staging_tree(
    state: StateTreeLsStagingTree,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let StateTreeLsStagingTree { target } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let tree = match locate(held, target.as_deref()) {
        Ok(tree) => tree,
        Err(next) => return next,
    };

    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorTreeFailed::new(t!("tree.err_no_store").trim().to_string()).into();
    };
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorTreeFailed::new(t!("tree.err_no_index").trim().to_string()).into();
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return ErrorTreeFailed::new(error.to_string()).into(),
    };

    let (structure, staged) = match staged_of(&tree, &layout, store, index, &runtime) {
        Ok(found) => found,
        Err(next) => return next,
    };

    ResultTreeLsStagingTree { structure, staged }.into()
}

/// The staged structure, or the recorded one when nothing is staged.
fn staged_of(
    tree: &Tree,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
) -> Result<(TreeStructure, bool), Next> {
    let staged = match tree.staging().structure() {
        Ok(staged) => staged,
        Err(error) => return Err(ErrorTreeFailed::new(error.to_string()).into()),
    };

    if let Some(staged) = staged {
        return Ok((staged, true));
    }

    let recorded = match runtime.block_on(tree.recorded_structure(layout, index, store)) {
        Ok(recorded) => recorded,
        Err(error) => return Err(ErrorTreeFailed::new(error.to_string()).into()),
    };

    Ok((recorded, false))
}

/// Result: the staged structure of one tree was listed.
#[derive(Grouped, StructuralData, serde::Serialize)]
pub struct ResultTreeLsStagingTree {
    /// The structure the listing is of.
    structure: TreeStructure,
    /// Whether anything was staged, rather than the recorded version being answered.
    staged: bool,
}

#[renderer(buffer)]
pub fn render_result_tree_ls_staging_tree(result: ResultTreeLsStagingTree, ec: &mut ResExitCode) {
    if !result.staged {
        r_eprintln!("{}", t!("tree.ls_staging_tree.nothing_staged").trim());
    }

    for line in rows(&result.structure) {
        r_println!("{line}");
    }

    let _ = ec;
}

//! The `rola tree cat-desc` command: the description of one tree version.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use librorolala::layout::Layout;
use librorolala::storage::{Key, RorolalaStorage};
use librorolala::tree::{TreeStructure, read_description, read_structure, version_of};
use librorolala::vcs::{VCSIndex, VCSIndexObject};
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_print, renderer,
        routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::layout::chosen;
use crate::tree::cmd_tree::{
    ErrorTreeArgument, ErrorTreeFailed, ErrorTreeShouldInWorkspace, locate,
};

#[help(buffer)]
pub fn help_tree_cat_desc(_: EntryTreeCatDesc, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.cat_desc.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeCatDesc)]
pub fn desc_tree_cat_desc() -> Description {
    t!("tree.cat_desc.description").to_string().into()
}

/// Completes what `rola tree cat-desc` can be given next.
#[completion(EntryTreeCatDesc)]
pub fn complete_tree_cat_desc() -> Suggest {
    Suggest::file_comp()
}

/// Prints the description of one tree version
///
/// `TARGET` names the tree, or a version hash. With no target the tree the run is standing in is
/// read at the version the Layout names. A version with no description prints nothing and ends
/// without an error: a description is optional, so its absence is an answer.
///
/// # Errors
///
/// Renders [`ErrorTreeNotATree`](crate::tree::cmd_tree::ErrorTreeNotATree) when the path is not
/// inside a tree, [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorTreeArgument`] when the target does not read, and
/// [`ErrorTreeCatDescFailed`] when the version or its description cannot be read.
#[command(node = "tree.cat-desc", entry = EntryTreeCatDesc)]
pub fn tree_cat_desc(args: EntryTreeCatDesc) -> Next {
    let picked = args.pick(arg![Option<String>]).pick(&UNMATCHED).to_result();
    let (target, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree cat-desc") {
        return next;
    }

    StateTreeCatDesc { target }.into()
}

/// The state a description reading starts in.
#[derive(Grouped)]
pub struct StateTreeCatDesc {
    /// The tree or version named, as it was written.
    target: Option<String>,
}

#[chain(routeify)]
pub fn handle_tree_cat_desc(
    state: StateTreeCatDesc,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let StateTreeCatDesc { target } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
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

    let structure = match structure_of(held, &layout, store, index, target.as_deref(), &runtime) {
        Ok(structure) => structure,
        Err(next) => return next,
    };

    let description = match runtime.block_on(read_description(store, &structure)) {
        Ok(description) => description,
        Err(error) => return ErrorTreeFailed::new(error.to_string()).into(),
    };

    ResultTreeCatDesc { description }.into()
}

/// The structure a description is read from: a version hash, or a tree's recorded version.
fn structure_of(
    held: &Workspace,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    target: Option<&str>,
    runtime: &tokio::runtime::Runtime,
) -> Result<TreeStructure, Next> {
    if let Some(text) = target {
        match version_of(text.trim_start_matches("blake3:")) {
            Ok(version) => {
                // A hash may name the version or the variant once it was made from: a variant names
                // its structure through the content it holds, and the description hangs off the
                // structure, so either hash reads the same description.
                return match runtime.block_on(index.read(Key::new(version))) {
                    Ok(VCSIndexObject::Variant(variant)) => {
                        let key = Key::new(*variant.storage_hash());
                        match runtime.block_on(store.read_object(&key)) {
                            Ok(bytes) => match TreeStructure::decode(&bytes) {
                                Ok(structure) => Ok(structure),
                                Err(error) => Err(ErrorTreeFailed::new(error.to_string()).into()),
                            },
                            Err(error) => Err(ErrorTreeFailed::new(error.to_string()).into()),
                        }
                    }
                    _ => runtime
                        .block_on(read_structure(index, store, &version))
                        .map_err(|error| ErrorTreeFailed::new(error.to_string()).into()),
                };
            }
            // A target that is not a hash is a path, and a path that reads as no tree is refused by
            // `locate`; a hash that does not read is an argument the run cannot use.
            Err(_) if text.len() == 64 => return Err(ErrorTreeArgument.into()),
            Err(_) => {}
        }
    }

    let tree = locate(held, target)?;

    runtime
        .block_on(tree.recorded_structure(layout, index, store))
        .map_err(|error| ErrorTreeFailed::new(error.to_string()).into())
}

/// Result: one version's description was read.
#[derive(Grouped)]
pub struct ResultTreeCatDesc {
    /// The description, when the version has one.
    description: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_tree_cat_desc(result: ResultTreeCatDesc, ec: &mut ResExitCode) {
    if let Some(description) = &result.description {
        r_print!("{description}");
    }

    let _ = ec;
}

/// Error: a version or its description could not be read.
#[derive(Grouped)]
pub struct ErrorTreeCatDescFailed {
    /// What happened.
    cause: String,
}

impl rorolala_errors::Failure for ErrorTreeCatDescFailed {
    fn name(&self) -> &'static str {
        "error_tree_cat_desc_failed"
    }

    fn reason(&self) -> String {
        t!("tree.cat_desc.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeCatDescFailed);

#[renderer(buffer)]
pub fn render_error_tree_cat_desc_failed(error: ErrorTreeCatDescFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.cat_desc.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

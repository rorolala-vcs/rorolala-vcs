//! The `rola tree ls-tree` command: the entries of one tree version, one a line.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use librorolala::layout::Layout;
use librorolala::storage::RorolalaStorage;
use librorolala::tree::{EntryKind, TreeStructure, read_structure, version_of};
use librorolala::vcs::VCSIndex;
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_print, r_println,
        renderer, routeify, suggest,
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
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::format::ResFormat;
use crate::layout::chosen;
use crate::tree::cmd_tree::{ErrorTreeFailed, ErrorTreeShouldInWorkspace, locate};

/// What an entry is drawn as when the run names no template.
const DEFAULT_FORMAT: &str =
    "{{ entries.mode }} {{ entries.type }} {{ entries.storage }} {{ entries.path }}";

#[help(buffer)]
pub fn help_tree_ls_tree(_: EntryTreeLsTree, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.ls_tree.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeLsTree)]
pub fn desc_tree_ls_tree() -> Description {
    t!("tree.ls_tree.description").to_string().into()
}

/// Completes what `rola tree ls-tree` can be given next.
#[completion(EntryTreeLsTree)]
pub fn complete_tree_ls_tree(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(&ctx, suggest!());
    }

    Suggest::file_comp()
}

/// Lists the entries one tree version holds
///
/// `TARGET` names the tree, or a version hash. With no target the tree the run is standing in is
/// read at the version the Layout names. A directory is answered as `040000 tree -`, a file as
/// `100644 blob <storage>` or `100755 blob <storage>`, and a symbolic link as `120000 link <target>`
/// followed by its path. Every entry is listed, since a tree structure is already flat and the
/// directory entries are part of what it records.
///
/// `--json` answers with the same entries as data.
///
/// # Errors
///
/// Renders [`ErrorTreeNotATree`](crate::tree::cmd_tree::ErrorTreeNotATree) when the path is not
/// inside a tree, [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorTreeArgument`](crate::tree::cmd_tree::ErrorTreeArgument) when the target does not read, and
/// [`ErrorTreeLsTreeFailed`] when the structure cannot be read.
#[command(node = "tree.ls-tree", entry = EntryTreeLsTree)]
pub fn tree_ls_tree(args: EntryTreeLsTree, format: &mut ResFormat) -> Next {
    format.default_template(DEFAULT_FORMAT);

    let picked = args.pick(arg![Option<String>]).pick(&UNMATCHED).to_result();
    let (target, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree ls-tree") {
        return next;
    }

    StateTreeLsTree { target }.into()
}

/// The state a listing of one tree version starts in.
#[derive(Grouped)]
pub struct StateTreeLsTree {
    /// The tree or version named, as it was written.
    target: Option<String>,
}

#[chain(routeify)]
pub fn handle_tree_ls_tree(
    state: StateTreeLsTree,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    format: &mut ResFormat,
) -> Next {
    let StateTreeLsTree { target } = state;

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

    let structure =
        match resolve_structure(held, &layout, store, index, target.as_deref(), &runtime) {
            Ok(structure) => structure,
            Err(next) => return next,
        };

    format.set("entries", row_values(&structure));

    ResultTreeLsTree { structure }.into()
}

/// The structure a listing names: a version hash, or a tree's recorded version.
pub fn resolve_structure(
    held: &Workspace,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    target: Option<&str>,
    runtime: &tokio::runtime::Runtime,
) -> Result<TreeStructure, Next> {
    if let Some(text) = target
        && let Ok(version) = version_of(text.trim_start_matches("blake3:"))
    {
        return match runtime.block_on(read_structure(index, store, &version)) {
            Ok(structure) => Ok(structure),
            Err(error) => Err(ErrorTreeFailed::new(error.to_string()).to_chain()),
        };
    }

    let tree = locate(held, target)?;

    match runtime.block_on(tree.recorded_structure(layout, index, store)) {
        Ok(structure) => Ok(structure),
        Err(error) => Err(ErrorTreeFailed::new(error.to_string()).to_chain()),
    }
}

/// Result: one tree version was listed.
#[derive(Grouped, StructuralData, serde::Serialize)]
pub struct ResultTreeLsTree {
    /// The structure the listing is of.
    structure: TreeStructure,
}

#[renderer(buffer)]
pub fn render_result_tree_ls_tree(
    result: ResultTreeLsTree,
    format: &ResFormat,
    ec: &mut ResExitCode,
) {
    if let Some(drawn) = format.drawn() {
        match drawn {
            Ok(text) => r_print!("{text}"),
            Err(error) => {
                r_eprintln!("{}", err_line!(error.to_string()));
                ec.exit_code = EC_ERR_TREE_FAILED;
            }
        }
    } else {
        for line in rows(&result.structure) {
            r_println!("{line}");
        }
    }
}

/// The entries as the data a template is drawn from: one row each with its mode, type and where it
/// holds content.
fn row_values(structure: &TreeStructure) -> Vec<serde_json::Value> {
    structure
        .entries()
        .iter()
        .map(|entry| {
            let (mode, kind, storage) = match entry.kind() {
                EntryKind::Directory => ("040000", "tree", "-".to_owned()),
                EntryKind::File => {
                    let mode = if entry.mode() == Some(librorolala::tree::FileMode::Executable) {
                        "100755"
                    } else {
                        "100644"
                    };
                    (
                        mode,
                        "blob",
                        entry
                            .storage()
                            .map_or_else(String::new, librorolala::storage::Key::hex),
                    )
                }
                EntryKind::Symlink => (
                    "120000",
                    "link",
                    entry.target().unwrap_or_default().to_owned(),
                ),
            };

            serde_json::json!({
                "mode": mode,
                "type": kind,
                "storage": storage,
                "path": entry.path(),
            })
        })
        .collect()
}

/// One line per entry, as `tree ls-tree` draws them.
#[must_use]
pub fn rows(structure: &TreeStructure) -> Vec<String> {
    structure
        .entries()
        .iter()
        .map(|entry| match entry.kind() {
            EntryKind::File => {
                let mode = if entry.mode() == Some(librorolala::tree::FileMode::Executable) {
                    "100755"
                } else {
                    "100644"
                };
                let storage = entry
                    .storage()
                    .map(librorolala::storage::Key::hex)
                    .unwrap_or_default();

                format!("{mode} blob {storage}\t{}", entry.path())
            }
            EntryKind::Directory => format!("040000 tree -\t{}", entry.path()),
            EntryKind::Symlink => format!(
                "120000 link {}\t{}",
                entry.target().unwrap_or_default(),
                entry.path()
            ),
        })
        .collect()
}

/// Error: a tree's entries could not be listed.
#[derive(Grouped)]
pub struct ErrorTreeLsTreeFailed {
    /// What happened.
    cause: String,
}

impl Failure for ErrorTreeLsTreeFailed {
    fn name(&self) -> &'static str {
        "error_tree_ls_tree_failed"
    }

    fn reason(&self) -> String {
        t!("tree.ls_tree.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeLsTreeFailed);

#[renderer(buffer)]
pub fn render_error_tree_ls_tree_failed(error: ErrorTreeLsTreeFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.ls_tree.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

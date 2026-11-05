//! The `rola tree status` command: how a tree's work stands beside what it records.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use librorolala::layout::Layout;
use librorolala::storage::RorolalaStorage;
use librorolala::tree::{ChangeKind, Tree, TreeStatus};
use librorolala::vcs::VCSIndex;
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::EC_HELP;
use crate::layout::chosen;
use crate::tree::cmd_tree::{ErrorTreeFailed, ErrorTreeShouldInWorkspace, locate, shown};

#[help(buffer)]
pub fn help_tree_status(_: EntryTreeStatus, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.status.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeStatus)]
pub fn desc_tree_status() -> Description {
    t!("tree.status.description").to_string().into()
}

/// Completes what `rola tree status` can be given next.
#[completion(EntryTreeStatus)]
pub fn complete_tree_status(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                crate::ignore::ARG_INCLUDE_IGNORED: t!("ignore.complete.include_ignored"),
            },
        );
    }

    Suggest::file_comp()
}

/// Reports how a tree's work stands
///
/// `TREE` names the tree: its root, a path inside it, or `.` for where the run was made. With no
/// name the tree the run is standing in is read.
///
/// What is reported is in three parts. **Staged** is what a commit would record: paths that differ
/// between the version the tree records and what is staged. **Unstaged** is what differs between
/// what is staged and the work on disk, and **untracked** is work that nothing staged holds. A path
/// a merge left in conflict is reported with the copy the merge left beside it, and it stops a
/// commit until it is settled.
///
/// `--json` answers with the same reading as data, and `--include-ignored` reads the work with the
/// ignore rules switched off.
///
/// # Errors
///
/// Renders [`ErrorTreeNotATree`](crate::tree::cmd_tree::ErrorTreeNotATree) when the path is not
/// inside a tree, [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace,
/// [`ErrorTreeFailed`] when the tree's own files cannot be
/// read.
#[command(node = "tree.status", entry = EntryTreeStatus)]
pub fn tree_status(args: EntryTreeStatus) -> Next {
    let picked = args
        .pick(crate::ignore::ARG_INCLUDE_IGNORED)
        .pick(arg![Option<String>])
        .pick(&UNMATCHED)
        .to_result();
    let (include_ignored, target, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree status") {
        return next;
    }

    StateTreeStatus {
        include_ignored: matches!(include_ignored, Flag::Active),
        target,
    }
    .into()
}

/// The state a status of a tree starts in.
#[derive(Grouped)]
pub struct StateTreeStatus {
    /// Whether the ignore rules are switched off.
    include_ignored: bool,
    /// The tree named, as it was written.
    target: Option<String>,
}

#[chain(routeify)]
pub fn handle_tree_status(
    state: StateTreeStatus,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let StateTreeStatus {
        include_ignored,
        target,
    } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let tree = locate(held, target.as_deref())?;
    let path = shown(&tree)?;

    let status = match status_of(held, &tree, &layout, storage, index, include_ignored) {
        Ok(status) => status,
        Err(next) => return next,
    };

    ResultTreeStatus { path, status }.into()
}

/// The reading of one tree.
fn status_of(
    held: &Workspace,
    tree: &Tree,
    layout: &Layout,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    include_ignored: bool,
) -> Result<TreeStatus, Next> {
    let Some(store): Option<&RorolalaStorage> = storage.get_ref().as_ref() else {
        return Err(ErrorTreeFailed::new(t!("tree.err_no_store").trim().to_string()).to_chain());
    };
    let Some(index): Option<&VCSIndex> = index.get_ref().as_ref() else {
        return Err(ErrorTreeFailed::new(t!("tree.err_no_index").trim().to_string()).to_chain());
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return Err(ErrorTreeFailed::new(error.to_string()).to_chain()),
    };

    let rules = crate::ignore::rules(held, include_ignored);
    match runtime.block_on(tree.status(layout, index, store, rules, include_ignored)) {
        Ok(status) => Ok(status),
        Err(error) => Err(ErrorTreeFailed::new(error.to_string()).to_chain()),
    }
}

/// Result: one tree's work was read.
#[derive(Grouped, StructuralData, serde::Serialize)]
pub struct ResultTreeStatus {
    /// The tree as a person knows it.
    path: String,
    /// How its work stands.
    status: TreeStatus,
}

#[renderer(buffer)]
pub fn render_result_tree_status(result: ResultTreeStatus, ec: &mut ResExitCode) {
    if result.status.is_clean() {
        r_println!("{}", t!("tree.status.clean", path = result.path).trim());
    } else {
        r_println!("{}", t!("tree.status.header", path = result.path).trim());

        for line in lines(&result.status) {
            r_println!("{line}");
        }
    }

    let _ = ec;
}

/// The lines one reading is drawn as.
pub fn lines(status: &TreeStatus) -> Vec<String> {
    let mut lines = Vec::new();

    if !status.conflicts.is_empty() {
        lines.push(String::new());
        lines.push(t!("tree.status.conflicts_header").trim().to_string());

        for conflict in &status.conflicts {
            lines.push(
                t!(
                    "tree.status.conflict",
                    path = conflict.path,
                    theirs = conflict.theirs
                )
                .trim()
                .to_string(),
            );
        }
    }

    for (header, changes) in [
        (t!("tree.status.staged_header"), &status.staged),
        (t!("tree.status.unstaged_header"), &status.unstaged),
    ] {
        if changes.is_empty() {
            continue;
        }

        lines.push(String::new());
        lines.push(header.trim().to_string());

        for change in changes {
            let said = match change.change {
                ChangeKind::Added => t!("tree.status.added"),
                ChangeKind::Modified => t!("tree.status.modified"),
                ChangeKind::Metadata => t!("tree.status.metadata"),
                ChangeKind::Deleted => t!("tree.status.deleted"),
                ChangeKind::TypeChanged => t!("tree.status.type_changed"),
                ChangeKind::Renamed => t!("tree.status.renamed"),
            };

            lines.push(change.old_path.as_ref().map_or_else(
                || format!("  {} {}", said.trim(), change.path),
                |old| format!("  {} {old} -> {}", said.trim(), change.path),
            ));
        }
    }

    if !status.untracked.is_empty() {
        lines.push(String::new());
        lines.push(t!("tree.status.untracked_header").trim().to_string());

        for path in &status.untracked {
            lines.push(format!("  {}", path.path));
        }
    }

    lines
}

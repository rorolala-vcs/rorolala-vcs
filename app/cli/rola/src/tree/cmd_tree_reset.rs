//! The `rola tree reset` command: put the staging area back to the recorded version.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use librorolala::layout::Layout;
use librorolala::storage::{Lockable as _, RorolalaStorage};
use librorolala::tree::{EntryKind, Staging, Tree, TreeEntry, TreeStructure};
use librorolala::vcs::VCSIndex;
use mingling::{
    Grouped, LazyRes, Suggest,
    macros::{arg, buffer, command, completion, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::layout::chosen;
use crate::tree::cmd_tree::{ErrorTreeShouldInWorkspace, locate, resolve};
use crate::tree::cmd_tree_rm::{prune_empty_directories, relative_to, under};

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
pub fn tree_reset(
    args: EntryTreeReset,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let paths = args.pick(arg![Vec<String>]).unwrap_or_default();

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorTreeResetFailed::new(t!("tree.err_no_store").trim().to_string()).into();
    };
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorTreeResetFailed::new(t!("tree.err_no_index").trim().to_string()).into();
    };

    let tree = match locate(held, paths.first().map(String::as_str)) {
        Ok(tree) => tree,
        Err(next) => return next,
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return ErrorTreeResetFailed::new(error.to_string()).into(),
    };

    match reset(&tree, &layout, store, index, &paths, &runtime) {
        Ok(result) => result.into(),
        Err(next) => next,
    }
}

/// Resets the staged paths `paths` name, or the whole staging area when none are named.
fn reset(
    tree: &Tree,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    paths: &[String],
    runtime: &tokio::runtime::Runtime,
) -> Result<ResultTreeReset, Next> {
    let staging = tree.staging();
    let _guard = match runtime.block_on(staging.lock()) {
        Ok(guard) => guard,
        Err(error) => return Err(ErrorTreeResetFailed::new(error.to_string()).into()),
    };

    if paths.is_empty() {
        return clear_staging(&staging, tree.root());
    }

    let recorded = match runtime.block_on(tree.recorded_structure(layout, index, store)) {
        Ok(recorded) => recorded,
        Err(error) => return Err(ErrorTreeResetFailed::new(error.to_string()).into()),
    };
    let staged_structure = match staging.structure() {
        Ok(staged) => staged,
        Err(error) => return Err(ErrorTreeResetFailed::new(error.to_string()).into()),
    };
    let staged = staged_structure.clone().unwrap_or_else(|| recorded.clone());
    let conflicts = match staging.conflicts() {
        Ok(conflicts) => conflicts,
        Err(error) => return Err(ErrorTreeResetFailed::new(error.to_string()).into()),
    };

    let mut entries = entries_of(&staged);
    let recorded_by_path = entries_of(&recorded);
    let Ok(cwd) = std::env::current_dir() else {
        return Err(ErrorTreeResetArgument.into());
    };

    let Ok((targets, named)) = targets(
        tree.root(),
        &cwd,
        paths,
        &entries,
        &recorded_by_path,
        &conflicts,
    ) else {
        return Err(ErrorTreeResetArgument.into());
    };

    if staged_structure.is_none() && conflicts.is_empty() {
        return Ok(ResultTreeReset {
            whole: false,
            paths: named,
        });
    }

    put_back(&mut entries, &recorded_by_path, &targets);
    let conflicts = settle_conflicts(conflicts, &targets, tree.root());
    prune_empty_directories(&mut entries);

    let structure = match TreeStructure::build(None, entries.values().cloned()) {
        Ok(structure) => structure,
        Err(error) => return Err(ErrorTreeResetFailed::new(error.to_string()).into()),
    };
    let description = match staging.description() {
        Ok(description) => description,
        Err(error) => return Err(ErrorTreeResetFailed::new(error.to_string()).into()),
    };

    let same_entries = structure.entries() == recorded.entries();
    let empty = conflicts.is_empty() && description.is_none() && same_entries;

    let written = if empty {
        staging.clear()
    } else {
        staging
            .write_structure(&structure)
            .and_then(|()| staging.write_conflicts(&conflicts))
    };
    if let Err(error) = written {
        return Err(ErrorTreeResetFailed::new(error.to_string()).into());
    }

    Ok(ResultTreeReset {
        whole: false,
        paths: named,
    })
}

/// Puts the whole staging area back to nothing, taking the copies a merge left behind with it.
fn clear_staging(staging: &Staging, root: &Path) -> Result<ResultTreeReset, Next> {
    let conflicts = match staging.conflicts() {
        Ok(conflicts) => conflicts,
        Err(error) => return Err(ErrorTreeResetFailed::new(error.to_string()).into()),
    };
    for path in &conflicts {
        remove_conflict(root, path);
    }

    if let Err(error) = staging.clear() {
        return Err(ErrorTreeResetFailed::new(error.to_string()).into());
    }

    Ok(ResultTreeReset {
        whole: true,
        paths: Vec::new(),
    })
}

/// The entries of a structure, by path.
fn entries_of(structure: &TreeStructure) -> BTreeMap<String, TreeEntry> {
    structure
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.clone()))
        .collect()
}

/// What the named paths stand for: every entry to put back, and the paths as the run wrote them.
///
/// A path the tree records, has staged or has in conflict is that path; a directory among them
/// brings everything under it. A path outside all three names nothing this command can put back.
fn targets(
    tree_root: &Path,
    cwd: &Path,
    paths: &[String],
    entries: &BTreeMap<String, TreeEntry>,
    recorded: &BTreeMap<String, TreeEntry>,
    conflicts: &[String],
) -> Result<(BTreeSet<String>, Vec<String>), ()> {
    let known: BTreeSet<String> = entries
        .keys()
        .chain(recorded.keys())
        .chain(conflicts.iter())
        .cloned()
        .collect();

    let mut targets: BTreeSet<String> = BTreeSet::new();
    let mut named: Vec<String> = Vec::with_capacity(paths.len());

    for given in paths {
        let resolved = resolve(cwd, given);
        let relative = relative_to(tree_root, &resolved).ok_or(())?;

        if relative.is_empty() {
            targets.extend(known.iter().cloned());
            named.push(".".to_owned());

            continue;
        }

        if is_directory(entries, recorded, &relative) {
            targets.extend(known.iter().filter(|path| under(&relative, path)).cloned());
        } else if known.contains(&relative) {
            targets.insert(relative.clone());
        } else {
            return Err(());
        }

        named.push(relative);
    }

    Ok((targets, named))
}

/// Puts the entries at `targets` back to what the recorded structure holds, or takes them out when
/// it holds nothing there.
fn put_back(
    entries: &mut BTreeMap<String, TreeEntry>,
    recorded: &BTreeMap<String, TreeEntry>,
    targets: &BTreeSet<String>,
) {
    for path in targets {
        match recorded.get(path) {
            Some(entry) => {
                entries.insert(path.clone(), entry.clone());
            }
            None => {
                entries.remove(path);
            }
        }
    }
}

/// Drops the conflicts a reset resolves, and the copies they left beside them.
fn settle_conflicts(
    conflicts: Vec<String>,
    targets: &BTreeSet<String>,
    root: &Path,
) -> Vec<String> {
    conflicts
        .into_iter()
        .filter(|path| {
            let reset = targets
                .iter()
                .any(|target| target == path || under(target, path));

            if reset {
                remove_conflict(root, path);
            }

            !reset
        })
        .collect()
}

/// Whether `path` is a directory of the staged or the recorded structure.
fn is_directory(
    staged: &BTreeMap<String, TreeEntry>,
    recorded: &BTreeMap<String, TreeEntry>,
    path: &str,
) -> bool {
    staged
        .get(path)
        .or_else(|| recorded.get(path))
        .is_some_and(|entry| entry.kind() == EntryKind::Directory)
}

/// Removes the `~theirs` copy a merge left beside `path`, when it is there.
fn remove_conflict(root: &Path, path: &str) {
    let copy = root.join(librorolala::tree::conflict_copy(path));

    let _ = std::fs::remove_file(&copy);
    let _ = std::fs::remove_dir_all(&copy);
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

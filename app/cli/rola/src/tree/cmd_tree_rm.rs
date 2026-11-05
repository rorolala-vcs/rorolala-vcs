//! The `rola tree rm` command: stage the removal of work from a tree.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use std::collections::BTreeMap;
use std::path::Path;

use librorolala::layout::Layout;
use librorolala::storage::{Lockable as _, RorolalaStorage, key_of_file};
use librorolala::tree::{EntryKind, Tree, TreeEntry, TreeStructure};
use librorolala::vcs::VCSIndex;
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
use rorolala_cli_setups::{ResForce, ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::{strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::layout::chosen;
use crate::tree::cmd_tree::{ErrorTreeShouldInWorkspace, locate, resolve};

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
pub fn handle_tree_rm(
    state: StateTreeRm,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorTreeRmFailed::new(t!("tree.err_no_store").trim().to_string()).into();
    };
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorTreeRmFailed::new(t!("tree.err_no_index").trim().to_string()).into();
    };

    let Some(first) = state.paths.first() else {
        return ErrorTreeRmArgument.into();
    };
    let tree = match locate(held, Some(first)) {
        Ok(tree) => tree,
        Err(next) => return next,
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return ErrorTreeRmFailed::new(error.to_string()).into(),
    };

    let StateTreeRm {
        cached,
        recursive,
        force,
        paths,
    } = state;

    match remove(
        &tree,
        &layout,
        store,
        index,
        &paths,
        (cached, recursive, force),
        &runtime,
    ) {
        Ok(result) => result.into(),
        Err(next) => next,
    }
}

/// Removes the paths named from the staged structure.
fn remove(
    tree: &Tree,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    paths: &[String],
    (cached, recursive, force): (bool, bool, bool),
    runtime: &tokio::runtime::Runtime,
) -> Result<ResultTreeRm, Next> {
    let staging = tree.staging();
    let _guard = match runtime.block_on(staging.lock()) {
        Ok(guard) => guard,
        Err(error) => return Err(ErrorTreeRmFailed::new(error.to_string()).into()),
    };

    let base = match staging.structure() {
        Ok(Some(staged)) => staged,
        Ok(None) => match runtime.block_on(tree.recorded_structure(layout, index, store)) {
            Ok(recorded) => recorded,
            Err(error) => return Err(ErrorTreeRmFailed::new(error.to_string()).into()),
        },
        Err(error) => return Err(ErrorTreeRmFailed::new(error.to_string()).into()),
    };

    let mut entries: BTreeMap<String, TreeEntry> = base
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.clone()))
        .collect();
    let mut conflicts = match staging.conflicts() {
        Ok(conflicts) => conflicts,
        Err(error) => return Err(ErrorTreeRmFailed::new(error.to_string()).into()),
    };

    let cwd = std::env::current_dir().map_err(|_| ErrorTreeRmArgument.to_chain())?;
    let root = tree.root().to_path_buf();
    let mut targets: Vec<String> = Vec::new();

    for given in paths {
        let resolved = resolve(&cwd, given);
        let Some(relative) = relative_to(&root, &resolved) else {
            return Err(ErrorTreeRmArgument.into());
        };

        match entries.get(&relative) {
            Some(entry) if entry.kind() == EntryKind::Directory => {
                if !recursive {
                    return Err(ErrorTreeRmArgument.into());
                }
                if holds_tree_or_data(&root, &relative) {
                    return Err(ErrorTreeRmArgument.into());
                }

                let held: Vec<String> = entries
                    .keys()
                    .filter(|path| under(&relative, path))
                    .cloned()
                    .collect();
                targets.extend(held);
            }
            Some(_) => targets.push(relative),
            None => {
                if relative.is_empty() {
                    return Err(ErrorTreeRmArgument.into());
                }

                // A path the tree never had is not something to remove, but one that is already
                // gone from the work is a removal to stage.
                if !root.join(&relative).exists() {
                    return Err(ErrorTreeRmArgument.into());
                }

                return Err(ErrorTreeRmArgument.into());
            }
        }
    }

    targets.sort_unstable();
    targets.dedup();

    let mut actions = Vec::new();
    for path in &targets {
        let Some(entry) = entries.get(path).cloned() else {
            continue;
        };

        if !cached
            && entry.kind() == EntryKind::File
            && !force
            && let Some(storage) = entry.storage()
        {
            let disk = root.join(path);
            match key_of_file(&disk) {
                Ok(key) if key.digest() == storage.digest() => {}
                Ok(_) => return Err(ErrorTreeRmDirty { path: path.clone() }.into()),
                Err(_) => {}
            }
        }

        entries.remove(path);
        if let Some(at) = conflicts.iter().position(|held| held == path) {
            conflicts.remove(at);
            let theirs = librorolala::tree::conflict_copy(path);
            let _ = std::fs::remove_file(root.join(theirs));
        }

        if !cached {
            take_work(&root, path, entry.kind());
        }

        actions.push(format!("{} {}", t!("tree.rm.deleted").trim(), path));
    }

    prune_empty_directories(&mut entries);

    let structure = match TreeStructure::build(None, entries.values().cloned()) {
        Ok(structure) => structure,
        Err(error) => return Err(ErrorTreeRmFailed::new(error.to_string()).into()),
    };

    if let Err(error) = staging.write_structure(&structure) {
        return Err(ErrorTreeRmFailed::new(error.to_string()).into());
    }
    if let Err(error) = staging.write_conflicts(&conflicts) {
        return Err(ErrorTreeRmFailed::new(error.to_string()).into());
    }

    Ok(ResultTreeRm { actions, cached })
}

/// Takes out the directory entries a removal left with nothing under them.
fn prune_empty_directories(entries: &mut BTreeMap<String, TreeEntry>) {
    let directories: Vec<String> = entries
        .iter()
        .filter(|(_, entry)| entry.kind() == EntryKind::Directory)
        .map(|(path, _)| path.clone())
        .collect();

    for directory in directories {
        if !entries
            .keys()
            .any(|path| path != &directory && under(&directory, path))
        {
            entries.remove(&directory);
        }
    }
}

/// Deletes `path` under `root`, when it is there.
fn take_work(root: &Path, path: &str, kind: EntryKind) {
    let disk = root.join(path);

    match kind {
        EntryKind::Directory => {
            let _ = std::fs::remove_dir(&disk);
        }
        EntryKind::File | EntryKind::Symlink => {
            let _ = std::fs::remove_file(&disk);
            prune_empty(root, disk.parent());
        }
    }
}

/// Removes the directories a file's removal left empty, up to but not including the tree's root.
fn prune_empty(root: &Path, mut at: Option<&Path>) {
    while let Some(directory) = at {
        if directory == root
            || !directory.starts_with(root)
            || std::fs::remove_dir(directory).is_err()
        {
            break;
        }

        at = directory.parent();
    }
}

/// Whether the directory at `path` holds a tree of its own or the Workspace's data.
fn holds_tree_or_data(root: &Path, path: &str) -> bool {
    let disk = root.join(path);

    disk.join(".rola-tree").is_dir()
        || disk.join(".rola").is_dir()
        || std::fs::read_dir(&disk).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                entry.path().join(".rola-tree").is_dir() || entry.path().join(".rola").is_dir()
            })
        })
}

/// `path` inside `root`, or nothing when it is not.
fn relative_to(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        return Some(String::new());
    }

    relative
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

/// Whether `path` is `scope` itself or under it.
fn under(scope: &str, path: &str) -> bool {
    path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
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

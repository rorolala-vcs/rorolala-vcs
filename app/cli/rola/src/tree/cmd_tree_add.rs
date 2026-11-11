//! The `rola tree add` command: stage work of a tree.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use std::collections::BTreeMap;
use std::path::Path;

use librorolala::layout::Layout;
use librorolala::storage::Lockable as _;
use librorolala::storage::RorolalaStorage;
use librorolala::tree::{EntryKind, Tree, TreeEntry, TreeStructure};
use librorolala::vcs::VCSIndex;
use librorolala::workspace::Workspace;
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
use rorolala_cli_setups::{ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::complete::strip_written;
use crate::exit_codes::{EC_ERR_TREE_ARGUMENT, EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::layout::chosen;
use crate::tree::cmd_tree::{ErrorTreeShouldInWorkspace, locate, resolve};

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
pub fn handle_tree_add(
    state: StateTreeAdd,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let StateTreeAdd {
        include_ignored,
        paths,
    } = state;

    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };
    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let Some(store) = storage.get_ref().as_ref() else {
        return ErrorTreeAddFailed::new(t!("tree.err_no_store").trim().to_string()).into();
    };
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorTreeAddFailed::new(t!("tree.err_no_index").trim().to_string()).into();
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return ErrorTreeAddFailed::new(error.to_string()).into(),
    };

    let Some(first) = paths.first() else {
        return ErrorTreeAddArgument.into();
    };
    let tree = match locate(held, Some(first)) {
        Ok(tree) => tree,
        Err(next) => return next,
    };

    match stage(
        held,
        &tree,
        &layout,
        store,
        index,
        &paths,
        include_ignored,
        &runtime,
    ) {
        Ok(result) => result.into(),
        Err(next) => next,
    }
}

/// Stages every path named, and answers what changed.
// The staging is one pass over the named paths: splitting it would restate the reading, the
// scopes it builds and the conflict list it settles at every step.
#[allow(clippy::too_many_lines)]
fn stage(
    held: &Workspace,
    tree: &Tree,
    layout: &Layout,
    store: &RorolalaStorage,
    index: &VCSIndex,
    paths: &[String],
    include_ignored: bool,
    runtime: &tokio::runtime::Runtime,
) -> Result<ResultTreeAdd, Next> {
    let staging = tree.staging();
    let _guard = match runtime.block_on(staging.lock()) {
        Ok(guard) => guard,
        Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
    };
    let staging_store = match staging.ensure_store() {
        Ok(store) => store,
        Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
    };
    let mut staging_batch = runtime.block_on(staging_store.begin_batch());

    let base = match staging.structure() {
        Ok(Some(staged)) => staged,
        Ok(None) => match runtime.block_on(tree.recorded_structure(layout, index, store)) {
            Ok(recorded) => recorded,
            Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
        },
        Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
    };

    let rules = if include_ignored {
        librorolala::tree::TreeRules::empty(crate::ignore::rules(held, true))
    } else {
        match tree.rules(crate::ignore::rules(held, false)) {
            Ok(rules) => rules,
            Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
        }
    };

    let scanned = match tree.scan(&rules, include_ignored) {
        Ok(scanned) => scanned,
        Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
    };

    let mut entries: BTreeMap<String, TreeEntry> = base
        .entries()
        .iter()
        .map(|entry| (entry.path().to_owned(), entry.clone()))
        .collect();
    let before: BTreeMap<String, TreeEntry> = entries.clone();
    let mut conflicts = match staging.conflicts() {
        Ok(conflicts) => conflicts,
        Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
    };

    let Ok(cwd) = std::env::current_dir() else {
        return Err(ErrorTreeAddArgument.into());
    };
    let tree_root = tree.root().to_path_buf();
    let mut scopes: Vec<String> = Vec::new();

    for given in paths {
        let resolved = resolve(&cwd, given);
        let Some(relative) = relative_to(&tree_root, &resolved) else {
            return Err(ErrorTreeAddArgument.into());
        };

        if relative.is_empty() {
            scopes.push(String::new());
            continue;
        }

        if let Some(entry) = scanned.structure.entry(&relative).cloned() {
            let staged = stage_entry(
                &staging,
                &staging_store,
                &mut staging_batch,
                &tree_root,
                &entry,
                entries.get(&relative),
                runtime,
            )?;
            // A whole directory brings the work under it; a file brings itself.
            if entry.kind() == EntryKind::Directory {
                scopes.push(relative);
            } else {
                entries.insert(staged.path().to_owned(), staged);
                resolve_conflict(&staging, &tree_root, &mut conflicts, &relative);
            }
            continue;
        }

        // What is not in the tree now is either something that went, which a directory's expansion
        // collects, or a path this run cannot stage.
        if entries.contains_key(&relative) {
            entries.remove(&relative);
            scopes.push(relative);
            continue;
        }

        return Err(ErrorTreeAddArgument.into());
    }

    // Every staged path under a scope is brought up to what the work holds: what the reading did
    // not find is what went, and what a scope did not cover stays as staged.
    if !scopes.is_empty() {
        for entry in scanned.structure.entries() {
            if scopes.iter().any(|scope| under(scope, entry.path())) {
                let staged = stage_entry(
                    &staging,
                    &staging_store,
                    &mut staging_batch,
                    &tree_root,
                    entry,
                    entries.get(entry.path()),
                    runtime,
                )?;
                entries.insert(staged.path().to_owned(), staged);
            }
        }

        let gone: Vec<String> = entries
            .keys()
            .filter(|path| {
                scopes.iter().any(|scope| under(scope, path))
                    && scanned.structure.entry(path).is_none()
            })
            .cloned()
            .collect();
        for path in &gone {
            entries.remove(path);
            resolve_conflict(&staging, &tree_root, &mut conflicts, path);
        }
    }

    if let Err(error) = runtime.block_on(staging_batch.publish()) {
        return Err(ErrorTreeAddFailed::new(error.to_string()).into());
    }

    let structure = match TreeStructure::build(None, entries.values().cloned()) {
        Ok(structure) => structure,
        Err(error) => return Err(ErrorTreeAddFailed::new(error.to_string()).into()),
    };

    if let Err(error) = staging.write_structure(&structure) {
        return Err(ErrorTreeAddFailed::new(error.to_string()).into());
    }
    if let Err(error) = staging.write_conflicts(&conflicts) {
        return Err(ErrorTreeAddFailed::new(error.to_string()).into());
    }

    Ok(ResultTreeAdd {
        actions: changes(&before, &structure),
    })
}

/// Hands one scanned entry into the staging store, unless what is already staged is the same work.
fn stage_entry(
    staging: &librorolala::tree::Staging,
    staging_store: &librorolala::storage::RorolalaStorage,
    staging_batch: &mut librorolala::storage::Batch,
    tree_root: &Path,
    entry: &TreeEntry,
    known: Option<&TreeEntry>,
    runtime: &tokio::runtime::Runtime,
) -> Result<TreeEntry, Next> {
    // The scan already read the file and knows the key its content would be stored under. When the staged
    // structure has the same entry, that content is already in the store it names, and reading the file
    // again would copy what is there over itself.
    if known == Some(entry) {
        return Ok(entry.clone());
    }

    let disk = tree_root.join(entry.path());
    let result = match entry.kind() {
        EntryKind::File => runtime.block_on(staging.stage_file_batch(
            staging_store,
            staging_batch,
            &disk,
            entry.path(),
        )),
        EntryKind::Symlink => staging.stage_symlink(&disk, entry.path()),
        EntryKind::Directory => return Ok(entry.clone()),
    };

    result.map_err(|error| ErrorTreeAddFailed::new(error.to_string()).into())
}

/// Removes a path from the conflict list, and the copy a merge left beside it, when it was in it.
fn resolve_conflict(
    staging: &librorolala::tree::Staging,
    tree_root: &Path,
    conflicts: &mut Vec<String>,
    path: &str,
) {
    if let Some(at) = conflicts.iter().position(|held| held == path) {
        conflicts.remove(at);

        let theirs = librorolala::tree::conflict_copy(path);
        let _ = std::fs::remove_file(tree_root.join(theirs));
    }

    let _ = staging;
}

/// How the staged structure differs from what was staged before.
fn changes(before: &BTreeMap<String, TreeEntry>, after: &TreeStructure) -> Vec<String> {
    let mut actions = Vec::new();

    for entry in after.entries() {
        match before.get(entry.path()) {
            None => actions.push(format!("{} {}", t!("tree.add.added").trim(), entry.path())),
            Some(was) if was != entry => {
                actions.push(format!(
                    "{} {}",
                    t!("tree.add.updated").trim(),
                    entry.path()
                ));
            }
            Some(_) => {}
        }
    }

    for path in before.keys() {
        if after.entry(path).is_none() {
            actions.push(format!("{} {}", t!("tree.add.deleted").trim(), path));
        }
    }

    actions
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
    scope.is_empty()
        || path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
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

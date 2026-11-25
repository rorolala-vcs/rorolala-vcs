//! The `rola fs-ops cp` command: put a copy of a path at another.

// What a copy is about, the two paths, the Workspace each sits in, the entries it forks, and the
// tree ids it rewrites, is read in one place, so the order the filesystem and the Layout are kept in
// step reads off the handler rather than being assembled from helpers.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use librorolala::layout::{EntryKind, Layout, LayoutPath, MutableData};
use librorolala::tree as rola_tree;
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, Routable, ShellContext, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_utils_cli_theme::trd;
use rorolala_utils_location::Locate as _;
use rorolala_utils_location::normalize;
use rust_i18n::t;
use uuid::Uuid;

use crate::Next;
use crate::cmd_fs_ops::{
    self, ErrorFsOpsArguments, ErrorFsOpsBlocked, ErrorFsOpsFailed, FsOpsBlock, ResultFsOps,
};
use crate::complete::typing_flag;
use crate::exit_codes::EC_HELP;

#[help(buffer)]
pub fn help_fs_ops_cp(_: EntryFsOpsCp, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("fs_ops.cp_help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFsOpsCp)]
pub fn desc_fs_ops_cp() -> Description {
    t!("fs_ops.cp_description").to_string().into()
}

/// Completes what `rola fs-ops cp` can be given next.
///
/// Both words name where something sits, so the filesystem answers them; there are no flags to
/// offer.
#[completion(EntryFsOpsCp)]
pub fn complete_fs_ops_cp(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return suggest!();
    }

    Suggest::file_comp()
}

/// Copies `FROM` to `TO`, `FROM` being left where it is.
///
/// `TO` is the path the copy is to have, not the directory it is to be put in: what a name already
/// taken there means was settled before this was called. Either path may name a file or a directory,
/// and copying a directory copies everything under it, content and structure, which is all that is
/// claimed: permissions, ownership and times are the destination's own, as the system gives them for
/// anything newly written.
///
/// A copy inside one Workspace keeps that Workspace's Layout in step: an entry the copy takes in is
/// forked at the destination path with a new `Uuid` and the source's own version, so the copy shares
/// the version and the history and is a separate entry. A copied tree's `.rola-tree/id` is written
/// with the fork's `Uuid`, or with a fresh one when the tree is marked but the Layout does not name
/// it. A destination path the copy lands on that already names an entry is replaced. A copy in a
/// git-rola project, or one that leaves the Workspace, changes no Layout, and the filesystem alone
/// is then what the operation is.
///
/// # Errors
///
/// Renders [`ErrorFsOpsArguments`] when both paths were not named, [`ErrorFsOpsBlocked`] when the
/// destination is the source or lies inside it, and [`ErrorFsOpsFailed`] when the filesystem, a
/// copied tree's id or the Layout refused the work.
#[command(node = "fs-ops.cp")]
pub fn fs_ops_cp(args: EntryFsOpsCp) -> Next {
    let picked = args
        .pick_or_route(arg![PathBuf], || {
            ErrorFsOpsArguments {
                verb: "cp",
                needs_to: true,
            }
            .to_chain()
        })
        .pick_or_route(arg![PathBuf], || {
            ErrorFsOpsArguments {
                verb: "cp",
                needs_to: true,
            }
            .to_chain()
        })
        .pick(&UNMATCHED)
        .to_result();
    let (from, to, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "fs-ops cp") {
        return next;
    }

    StateFsOpsCp { from, to }.into()
}

/// The state of copying one path to another.
#[derive(Grouped)]
pub struct StateFsOpsCp {
    /// What is being copied.
    from: PathBuf,
    /// Where the copy is being put.
    to: PathBuf,
}

#[chain(routeify)]
pub fn handle_fs_ops_cp(state: StateFsOpsCp) -> Next {
    // Taken apart rather than borrowed from, so that the state is consumed rather than only read.
    let StateFsOpsCp { from, to } = state;

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return ErrorFsOpsFailed {
                verb: "cp",
                cause: error.to_string(),
            }
            .into();
        }
    };

    let from = whole(&cwd, &from);
    let to = whole(&cwd, &to);

    // A copy that writes into what it is reading, or onto the file it reads, would take the source
    // apart while copying it. The Desktop refuses it before calling, and the command does too.
    if to == from || (from.is_dir() && to.starts_with(&from)) {
        return ErrorFsOpsBlocked {
            path: to.display().to_string(),
            kind: FsOpsBlock::Inside,
        }
        .into();
    }

    // The Workspace each path sits in, looked for from the paths themselves and never from where the
    // run was made. Only one Workspace changes here: a copy that crosses Workspaces, or enters or
    // leaves one, is the filesystem's alone. A git-rola project keeps its record in `rola.toml` and
    // `rola.lock`, which a copy does not change, so a source inside one never forks a Layout.
    let source = cmd_fs_ops::workspace_of(&cwd, &from);
    let target = cmd_fs_ops::workspace_of(&cwd, &to);
    let same = source
        .as_ref()
        .zip(target.as_ref())
        .is_some_and(|(source, target)| source.get_root() == target.get_root());

    let plan = if same && cmd_fs_ops::git_rola::project_of(&from).is_none() {
        source
            .as_ref()
            .and_then(|held| plan_cp(held, &cwd, &from, &to))
    } else {
        None
    };

    // The filesystem first, since that is what was asked for, and the Layout follows it. A Layout
    // that could not be written leaves the two disagreeing, which is what `rola align` settles
    // rather than something this run can put back.
    if let Err(error) = cmd_fs_ops::copy_to(&from, &to) {
        return ErrorFsOpsFailed {
            verb: "cp",
            cause: error.to_string(),
        }
        .into();
    }

    // Every marked tree in the copy is given an id of its own before the Layout is written, so the
    // Layout never names a tree whose mark still carries the original's id. The fork's id is used
    // where the Layout names the tree; a marked tree the Layout does not name gets a fresh one,
    // when its mark already carries an id to replace.
    let forks = plan.as_ref().map_or_default(CpPlan::fork_trees);
    let root = plan.as_ref().map(|plan| plan.root.as_path());

    if let Err(error) = rekey_trees(root, &to, &forks) {
        return ErrorFsOpsFailed {
            verb: "cp",
            cause: error.to_string(),
        }
        .into();
    }

    if let Some(plan) = plan
        && let Err(next) = record(plan)
    {
        return next;
    }

    ResultFsOps {
        verb: "cp",
        from: from.display().to_string(),
        to: Some(to.display().to_string()),
    }
    .into()
}

/// What the copy is about in the Layout being worked in.
struct CpPlan {
    /// The root of the Workspace the Layout belongs to.
    root: PathBuf,
    /// The Layout being worked in.
    layout: Layout,
    /// The entries the copy makes, one for each tracked entry it takes in.
    forks: Vec<Fork>,
    /// The entries the copy lands on, which no longer describe what those paths hold.
    dropped: BTreeSet<Uuid>,
}

impl CpPlan {
    /// The tree forks by the path they land on, so a copied mark can be written with its own id.
    fn fork_trees(&self) -> BTreeMap<String, Uuid> {
        self.forks
            .iter()
            .filter(|fork| fork.data.kind() == EntryKind::Tree)
            .map(|fork| (fork.dest.as_str().to_owned(), fork.id))
            .collect()
    }
}

/// One entry the copy makes: the entry it comes from, the path it lands on, and its new id.
struct Fork {
    /// The path the Layout names the source by.
    source: LayoutPath,
    /// The path the copy lands on in the Layout.
    dest: LayoutPath,
    /// The id the copy is given.
    id: Uuid,
    /// What the source entry holds, kept as it is so the two share the version.
    data: MutableData,
}

/// What the copy is about in the Layout being worked in, when there is anything to keep in step.
///
/// Nothing is returned for a copy whose Layout names nothing it takes in and whose destination names
/// nothing it lands on: the filesystem alone is then what the operation is.
fn plan_cp(held: &Workspace, cwd: &Path, from: &Path, to: &Path) -> Option<CpPlan> {
    let layout = cmd_fs_ops::working(held)?;
    let source = cmd_fs_ops::inside(held, cwd, from)?;
    let target = cmd_fs_ops::inside(held, cwd, to)?;

    let named = cmd_fs_ops::named(&layout, &cmd_fs_ops::touched(&layout, held, cwd, from));

    let mut forks = Vec::new();
    for (path, id) in named {
        // A path that names no entry, an orphan, has nothing to fork; the copy is then plain.
        let Some(data) = layout.entry(id) else {
            continue;
        };

        let dest = cmd_fs_ops::moved(&source, &target, &path);
        forks.push(Fork {
            source: path,
            dest,
            id: Uuid::new_v4(),
            data,
        });
    }

    let fork_dests: BTreeSet<String> = forks
        .iter()
        .map(|fork| fork.dest.as_str().to_owned())
        .collect();

    let mut dropped = BTreeSet::new();
    for (path, old) in layout.paths() {
        if !cmd_fs_ops::under(&target, &path) {
            continue;
        }

        if fork_dests.contains(path.as_str()) {
            dropped.insert(old);

            continue;
        }

        // The copy writes over this destination path, so the entry there no longer describes what
        // it holds. A destination path the source does not write over is left as it is.
        let mirrored = cmd_fs_ops::moved(&target, &source, &path);

        if held.get_root().join(mirrored.to_path_buf()).exists() {
            dropped.insert(old);
        }
    }

    if forks.is_empty() && dropped.is_empty() {
        return None;
    }

    Some(CpPlan {
        root: held.get_root().to_path_buf(),
        layout,
        forks,
        dropped,
    })
}

/// Writes the forks down and remembers the edit state each file copy carries.
fn record(plan: CpPlan) -> Result<(), Next> {
    let mut batch = plan.layout.batch();

    for id in plan.dropped {
        if let Err(error) = batch.remove_entry(id) {
            return Err(crate::layout::failed(&error));
        }
    }

    for fork in &plan.forks {
        if let Err(error) = batch.create_file(fork.id, fork.data.clone(), &fork.dest) {
            return Err(crate::layout::failed(&error));
        }
    }

    if let Err(error) = batch.commit() {
        return Err(crate::layout::failed(&error));
    }

    for fork in &plan.forks {
        if fork.data.kind() != EntryKind::File {
            continue;
        }

        if let Err(error) = librorolala::workspace_analyze::remember_copied(
            &plan.root,
            &plan.layout,
            &fork.source,
            &fork.dest,
        ) {
            return Err(ErrorFsOpsFailed {
                verb: "cp",
                cause: error.to_string(),
            }
            .into());
        }
    }

    Ok(())
}

/// Gives every marked tree in the copy an id of its own.
///
/// A copy of a tree is another tree, so the copy's mark is written with the id the Layout will name
/// it by, or with a fresh id when the Layout does not name it. A mark that carries no id has nothing
/// to collide, so it is left without one. What is walked is the copy, never the source.
fn rekey_trees(
    root: Option<&Path>,
    copy: &Path,
    forks: &BTreeMap<String, Uuid>,
) -> std::io::Result<()> {
    if !copy.is_dir() {
        return Ok(());
    }

    rekey_at(root, copy, forks)
}

/// Gives the marked tree at `at` an id of its own, and every marked tree under it one too.
fn rekey_at(root: Option<&Path>, at: &Path, forks: &BTreeMap<String, Uuid>) -> std::io::Result<()> {
    let mark = at.join(rola_tree::consts::MARK);

    if mark.is_dir() {
        let named = root
            .and_then(|root| at.strip_prefix(root).ok())
            .map(cmd_fs_ops::prefix)
            .and_then(|path| forks.get(&path).copied());

        if let Some(id) = named {
            rola_tree::rekey(at, id).map_err(|error| std::io::Error::other(error.to_string()))?;
        } else if mark.join(rola_tree::consts::ENTRY_ID).exists() {
            rola_tree::rekey(at, Uuid::new_v4())
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }
    }

    for entry in std::fs::read_dir(at)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if name.as_ref() == rola_tree::consts::MARK || name.as_ref() == rola_tree::consts::DATA {
            continue;
        }

        if entry.file_type()?.is_dir() {
            rekey_at(root, &entry.path(), forks)?;
        }
    }

    Ok(())
}

/// The whole of a path, so that a relative name is read where the run was made.
fn whole(cwd: &Path, path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    normalize(&path)
}

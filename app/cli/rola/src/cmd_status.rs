//! The `rola status` command: how the working tree stands beside the Layout it works from.
//!
//! It is the reading behind the two [porcelain](crate) commands that act on the tree, said for a
//! person rather than for a program: what is lost or untagged needs `rola align`, what changed in
//! place needs `rola track`, and a move is one `rola align` confirms and `track` confirms by
//! itself. What it is drawn as is a few lines — what moved, what is gone, what appeared, what
//! changed — and the one thing to do about each.
//!
//! A clean tree says so and nothing else. A run that asked for `--json` is answered with the same
//! reading as data, since a reading is what it is either way; a template is not offered, as what a
//! person is shown here is not a list of one shape but a sentence about several.

// The reading of how a file stands takes every part of it — the version, the Vault's copy, the
// index, the merge in progress — and gathering them into one value would only move the count
// somewhere else.
#![allow(clippy::too_many_arguments)]

use std::collections::BTreeSet;

use std::path::{Component, Path, PathBuf};
use std::str::FromStr as _;

use librorolala::layout::{Layout, LayoutPath};
use librorolala::storage::{Blake3Hash, Key};
use librorolala::tree_analyze::{entry_of, tree_diff};
use librorolala::vcs::{VCSIndex, VCSIndexObject, VCSWrite as _, Version};
use librorolala::workspace::{Merging, Pending, Workspace};
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVCSIndex, ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{filling_flag, positional, strip_written, typing_flag};
use crate::exit_codes::{EC_ERR_LAYOUT_ARGUMENT, EC_HELP};
use crate::failure::failure;
use crate::layout::{ErrorLayoutFailed, ErrorLayoutShouldInWorkspace, chosen, readonly_layout_dir};
use crate::ownership;
use crate::vcs_index::cmd_vcs_index_lookback::{
    MergeHint, ResLookback, Standing, StandingRelation, from_object,
};
use crate::vcs_index::{ErrorVcsIndexNoIndex, ErrorVcsIndexRead, parse_hash, runtime};

/// How alike two text files have to be to count as the same file moved, when nothing is said.
const DEFAULT_ALIKE: f32 = 0.6;

/// Draw the lines above the chain without the chain itself.
const ARG_NO_GRAPH: PickerArg<'static, Flag> = arg![no_graph: Flag];

/// Draw the chain without the lines that say where the file comes from and how it stands.
const ARG_NO_HINT: PickerArg<'static, Flag> = arg![no_hint: Flag];

#[help(buffer)]
pub fn help_status(_: EntryStatus, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("status.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryStatus)]
pub fn desc_status() -> Description {
    t!("status.description").to_string().into()
}

/// Completes what `rola status` can be given next.
///
/// What is read is the tree, so a target that names a path is answered by the filesystem. The flags
/// are the look-back ones the command shares with `vcs-index lookback`; the length one takes a
/// number, which is nothing a list of names can offer.
#[completion(EntryStatus)]
pub fn complete_status(ctx: ShellContext) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                crate::vcs_index::cmd_vcs_index_lookback::ARG_COMPACT: t!("status.complete.compact"),
                crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_MESSAGE: t!("status.complete.no_message"),
                crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_CREATOR: t!("status.complete.no_creator"),
                crate::vcs_index::cmd_vcs_index_lookback::ARG_MAX_MESSAGE_LENGTH: t!("status.complete.max_message_length"),
                ARG_NO_GRAPH: t!("status.complete.no_graph"),
                ARG_NO_HINT: t!("status.complete.no_hint"),
            },
        );
    }

    if filling_flag(
        &ctx,
        &crate::vcs_index::cmd_vcs_index_lookback::ARG_MAX_MESSAGE_LENGTH,
    ) {
        return suggest!();
    }

    if positional(&ctx, "status") == 0 {
        Suggest::file_comp()
    } else {
        suggest!()
    }
}

/// Shows how the working tree stands beside the Layout being worked in
///
/// What moved is shown first, then what the Layout names and the tree does not hold, then what the
/// tree holds and the Layout does not name; a line with none of the marks is content that changed
/// where it was. A `(*)` marks a line that is also a content change, which for a move is a move
/// whose file was edited as well.
///
/// What is lost is what `rola align` settles and what changed is what `rola track` records; a move
/// is confirmed by `rola align` or, by naming the file where it is now, by `rola track`.
///
/// # Errors
///
/// Renders the run-not-in-a-workspace failure when the run is not inside a Workspace, the argument
/// or not-there failures when there is no Layout to read, and [`ErrorLayoutFailed`] when the tree
/// or the Layout could not be read.
#[command(node = "status", entry = EntryStatus)]
pub fn status(args: EntryStatus, lookback: &mut ResLookback) -> Next {
    let picked = args
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_COMPACT)
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_MESSAGE)
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_NO_CREATOR)
        .pick(&crate::vcs_index::cmd_vcs_index_lookback::ARG_MAX_MESSAGE_LENGTH)
        .pick(&ARG_NO_GRAPH)
        .pick(&ARG_NO_HINT)
        .pick(&arg![Option<String>])
        .to_result();
    let (compact, no_message, no_creator, max_message_length, no_graph, no_hint, target) =
        match picked {
            Ok(picked) => picked,
            Err(next) => return next,
        };

    lookback.asked(compact, no_message, no_creator, max_message_length);
    lookback.asked_status(no_graph, no_hint);

    StateStatus { target }.into()
}

/// The state a reading of the tree starts in.
#[derive(Grouped)]
pub struct StateStatus {
    /// What to look back from, when the run named something rather than asking how the work stands.
    target: Option<String>,
}

#[chain(routeify)]
pub fn handle_status(
    state: StateStatus,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    account: &mut LazyRes<ResCurrentAccount>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Next {
    let layout = match chosen(workspace.get_ref(), vault.get_ref(), None) {
        Ok(layout) => layout,
        Err(next) => return next,
    };

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };

    let merging = Merging::read(held.get_root());

    if let Some(target) = state.target {
        let me = account.get_ref().must_bind().ok();
        let me = me.as_deref();

        // A name is written against where the run was made, the way `rola track` and `rola align`
        // read one, so the reading starts there. A run whose own directory cannot be read still
        // reads the name as the Layout writes it.
        let cwd = std::env::current_dir().ok();

        // A variant file is not a path the Layout names, so no chain hangs from it: what it is and
        // what it is waiting for are said instead of a drawing.
        if let Some((pending, path)) =
            variant_pending(&merging, &layout, held.get_root(), cwd.as_deref(), &target)
        {
            return ResultStatusMerge {
                variant: crate::vcs_index::hex(pending.variant()),
                path: path.as_str().to_owned(),
                target: layout
                    .path_of(pending.target())
                    .map(|path| path.as_str().to_owned()),
            }
            .into();
        }

        return lookback_of(&target, &layout, held, index, me, cwd.as_deref(), &merging);
    }

    let diff = match tree_diff(&layout, held, DEFAULT_ALIKE) {
        Ok(diff) => diff,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    // What is not this account's to change is read before anything is said, and kept out of the
    // content changes: a run that read them as its own work would be reading past the one thing it
    // is not to do.
    let me = account.get_ref().must_bind().ok();
    let tracked = ownership::tracked_vault(held, &layout);
    let unowned = match ownership::unowned(held, &layout, tracked.as_deref(), me.as_deref(), &diff)
    {
        Ok(unowned) => unowned,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };
    let not_mine: BTreeSet<&str> = unowned.iter().map(String::as_str).collect();

    // Which of the moves were edited as well: a move's destination is among the modified when what
    // it holds is not what the Layout agreed with at the path it came from.
    let edited: BTreeSet<&str> = diff.modified.iter().map(LayoutPath::as_str).collect();

    // What is waiting to be joined is read against the Layout, the tree and the index: a record that
    // does not stand is not a merge, and saying so is the point of reading them at all. A variant
    // file the tree holds elsewhere is claimed here, so it is not also reported as a new file.
    let (merging, broken, moved) = merging_view(
        &merging,
        &layout,
        held,
        index.get_ref().as_ref(),
        &diff.untagged,
    );

    ResultStatus {
        lost: diff
            .lost
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect(),
        untagged: diff
            .untagged
            .iter()
            .filter(|path| !moved.contains(*path))
            .map(|path| path.as_str().to_owned())
            .collect(),
        modified: diff
            .modified
            .iter()
            .filter(|path| !not_mine.contains(path.as_str()))
            .map(|path| path.as_str().to_owned())
            .collect(),
        renamed: diff
            .renamed
            .iter()
            .map(|rename| RenameItem {
                from: rename.from.as_str().to_owned(),
                to: rename.to.as_str().to_owned(),
                modified: edited.contains(rename.to.as_str()),
                strong: rename.strong,
            })
            .collect(),
        unowned,
        merging,
        broken,
    }
    .into()
}

/// The variant waiting whose file is at the path `target` names, when the target is one.
///
/// A variant file is not a path the Layout names, so it is found by the reading a name gets anywhere
/// else — from where the run was made, then as it stands — and answered from the merge in progress
/// rather than from the Layout.
fn variant_pending<'a>(
    merging: &'a Merging,
    layout: &Layout,
    root: &Path,
    cwd: Option<&Path>,
    target: &str,
) -> Option<(&'a Pending, LayoutPath)> {
    for path in targets(target, root, cwd) {
        if let Some(pending) = merging.variant_of(layout, &path) {
            return Some((pending, path));
        }
    }

    None
}

/// What the merge in progress says, read against the Layout, the tree and the index.
///
/// A record is a merge only while all of it stands: the target is still named, the variant file is
/// still there — or is found again where a move put it — the index still holds the variant and the
/// file still holds that variant's content. One that does not stand is not dropped quietly — a merge
/// that cannot go on is worse unseen — so it is answered as what it is rather than among the merges
/// that are.
///
/// The paths the variant files were found at are handed back with it, so a reading that was told to
/// leave variant files out can leave these out too.
fn merging_view(
    merging: &Merging,
    layout: &Layout,
    held: &Workspace,
    index: Option<&VCSIndex>,
    untagged: &[LayoutPath],
) -> (Vec<MergingItem>, Vec<String>, BTreeSet<LayoutPath>) {
    let Ok(runtime) = crate::vcs_index::runtime() else {
        return (Vec::new(), Vec::new(), BTreeSet::new());
    };

    let moves = crate::merging::moves(merging, layout, held.get_root(), index, &runtime, untagged);
    let mut waiting = Vec::new();
    let mut broken = Vec::new();
    let mut claimed = BTreeSet::new();

    for pending in merging.iter() {
        // A file that moved is not one to report as gone: the record still stands, and what is to be
        // confirmed is where the file now lies.
        if let Some(moved) = moves.iter().find(|moved| moved.target == pending.target())
            && let Some(target) = layout.path_of(pending.target())
        {
            claimed.insert(moved.to.clone());
            waiting.push(MergingItem {
                target: target.as_str().to_owned(),
                variant: crate::vcs_index::hex(pending.variant()),
                path: moved.from.as_str().to_owned(),
                moved: Some(moved.to.as_str().to_owned()),
            });

            continue;
        }

        match merging_item(merging, layout, held, index, &runtime, pending) {
            Ok(item) => waiting.push(item),
            Err(cause) => broken.push(cause),
        }
    }

    (waiting, broken, claimed)
}

/// One record of the merge in progress, read as a merge that can go on or as why it cannot.
fn merging_item(
    merging: &Merging,
    layout: &Layout,
    held: &Workspace,
    index: Option<&VCSIndex>,
    runtime: &tokio::runtime::Runtime,
    pending: &Pending,
) -> Result<MergingItem, String> {
    let variant = crate::vcs_index::hex(pending.variant());

    let Some(target) = layout.path_of(pending.target()) else {
        return Err(t!("status.merge_no_target", variant = variant)
            .trim()
            .to_owned());
    };
    let Some(path) = merging.variant_path(layout, pending) else {
        return Err(t!("status.merge_no_place", variant = variant)
            .trim()
            .to_owned());
    };

    let disk = held.get_root().join(path.to_path_buf());
    if !disk.is_file() {
        return Err(t!("status.merge_file_gone", path = path.as_str())
            .trim()
            .to_owned());
    }

    let Some(index) = index else {
        return Err(t!("status.merge_no_index", path = path.as_str())
            .trim()
            .to_owned());
    };
    let Some(joined) = read_variant(index, runtime, pending.variant()) else {
        return Err(t!("status.merge_no_variant", variant = variant)
            .trim()
            .to_owned());
    };

    let Ok(entry) = entry_of(&disk) else {
        return Err(t!("status.merge_unreadable", path = path.as_str())
            .trim()
            .to_owned());
    };
    if entry.digest() != *joined.storage_hash() {
        return Err(t!("status.merge_changed", path = path.as_str())
            .trim()
            .to_owned());
    }

    Ok(MergingItem {
        target: target.as_str().to_owned(),
        variant,
        path: path.as_str().to_owned(),
        moved: None,
    })
}

/// One thing a target named: the index object to look back from, and whether the work on it has
/// changes that were never recorded.
struct Found {
    /// The hash of the object the chain is drawn from.
    key: Key,
    /// Whether the file the version belongs to has changes that were never recorded.
    editing: bool,
    /// The entry the target named, when it named one rather than an object by hash.
    id: Option<Uuid>,
}

/// Draws the chain what `target` names sits at the top of.
///
/// What a target may name is read in the order a run is likely to have meant: a path read from where
/// the run was made, a path of the Layout being worked in, where a path of the Layout has moved to
/// in the tree, a path in the Vault's own Layout as the copy here has it, a `Uuid`, then the hash of
/// an index object. The first that names something is what is drawn — so a name that is a path of
/// this work is the work's, even when some object of the index happens to hash to it — and a target
/// that names nothing at all is refused rather than guessed at.
#[routeify]
fn lookback_of(
    target: &str,
    layout: &Layout,
    held: &Workspace,
    index: &mut LazyRes<ResVCSIndex>,
    me: Option<&str>,
    cwd: Option<&Path>,
    merging: &Merging,
) -> Next {
    let Some(index) = index.get_ref().as_ref() else {
        return ErrorVcsIndexNoIndex.into();
    };
    let runtime = match runtime() {
        Ok(runtime) => runtime,
        Err(error) => return error.into(),
    };

    let remote = tracked_layout(held);
    let found = match found(
        target,
        layout,
        remote.as_ref().map(|(_, layout)| layout),
        held,
        cwd,
    ) {
        Ok(found) => found,
        Err(cause) => {
            return ErrorStatusTarget {
                target: target.to_owned(),
                cause,
            }
            .into();
        }
    };

    // Where the Vault's own Layout stands for this entry: the version it records, when it records
    // one. What the chain is read for is where the Vault is, so a target naming an entry the Vault
    // knows about is answered with the mark that says so.
    let origin = found.id.and_then(|id| {
        if let Some((vault, remote)) = remote.as_ref()
            && let Some(version) = remote.entry(id).map(|data| data.version())
            && version != [0; 32]
        {
            return Some((vault.as_str(), version));
        }

        None
    });
    let id = found.id;
    let editing = found.editing;

    let mut result = match from_object(index, &runtime, found.key, origin) {
        Ok(result) => result,
        Err(cause) => return ErrorVcsIndexRead::new(cause).into(),
    };
    if editing {
        result.editing();
    }
    if let Some(id) = id {
        result.standing(standing(
            id,
            editing,
            layout,
            remote.as_ref(),
            index,
            &runtime,
            me,
            merging,
        ));
    }

    result.into()
}

/// How the file the target named stands, for the lines drawn above its chain.
///
/// The Vault's copy is what ownership and the version to compare are read from. A Layout that tracks
/// no Vault, or one whose copy does not hold the entry, falls back to what this Layout itself says,
/// and there is then no version to compare: the state says who holds the file and whether it was
/// changed, and nothing about how far the two sides are apart.
fn standing(
    id: Uuid,
    modified: bool,
    layout: &Layout,
    remote: Option<&(String, Layout)>,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    me: Option<&str>,
    merging: &Merging,
) -> Standing {
    let local = layout
        .entry(id)
        .map(|data| data.version())
        .filter(|version| *version != [0; 32]);

    let (vault, holder, source, upstream) = match remote {
        Some((vault, copy)) => {
            let data = copy.entry(id);

            (
                vault.clone(),
                data.as_ref()
                    .and_then(|data| data.owner().map(str::to_owned)),
                copy.path_of(id).map(|path| path.as_str().to_owned()),
                data.map(|data| data.version())
                    .filter(|version| *version != [0; 32]),
            )
        }
        None => (
            String::new(),
            layout
                .entry(id)
                .and_then(|data| data.owner().map(str::to_owned)),
            None,
            None,
        ),
    };

    let (relation, distance) = match (local, upstream) {
        (Some(local), Some(upstream)) => apart(index, runtime, &local, &upstream),
        _ => (StandingRelation::Unknown, None),
    };

    Standing {
        vault,
        source,
        mine: holder.is_some() && holder.as_deref() == me,
        holder,
        modified,
        relation,
        distance,
        merging: merging.at(id).map(|pending| MergeHint {
            variant: crate::vcs_index::hex(pending.variant()),
            path: merging
                .variant_path(layout, pending)
                .map_or_else(String::new, |path| path.as_str().to_owned()),
        }),
    }
}

/// How the version this Layout is at stands beside the Vault's, and how many versions apart they are.
///
/// A version the index does not hold is one this side never had, so how far apart the two are is not
/// known here: the Vault has moved and the chain that says how far is not local.
fn apart(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    local: &Blake3Hash,
    upstream: &Blake3Hash,
) -> (StandingRelation, Option<u64>) {
    if local == upstream {
        return (StandingRelation::Same, Some(0));
    }

    let (Some(local), Some(upstream)) = (
        read_version(index, runtime, local),
        read_version(index, runtime, upstream),
    ) else {
        return (StandingRelation::Unknown, None);
    };

    if let Some(distance) = distance_to(index, runtime, &local, upstream.hash().digest()) {
        return (StandingRelation::Ahead, Some(distance));
    }
    if let Some(distance) = distance_to(index, runtime, &upstream, local.hash().digest()) {
        return (StandingRelation::Behind, Some(distance));
    }

    (StandingRelation::Apart, None)
}

/// How many versions below `descendant` the version `ancestor` is, when it is below it at all.
fn distance_to(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    descendant: &Version,
    ancestor: &Blake3Hash,
) -> Option<u64> {
    let mut current = descendant.clone();
    let mut steps = 0;

    while !current.is_root() {
        let variant = read_variant(index, runtime, current.variant())?;
        let base = read_version(index, runtime, variant.base_version())?;

        steps += 1;
        if base.hash().digest() == ancestor {
            return Some(steps);
        }

        current = base;
    }

    None
}

/// The version the index holds under `hash`, when it holds one.
fn read_version(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &Blake3Hash,
) -> Option<Version> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Version(version)) => Some(version),
        _ => None,
    }
}

/// The variant the index holds under `hash`, when it holds one.
fn read_variant(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &Blake3Hash,
) -> Option<librorolala::vcs::Variant> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Variant(variant)) => Some(variant),
        _ => None,
    }
}

/// The copy of the Vault's own Layout the Layout being worked in tracks, when there is one here,
/// under the name the Vault is bound by.
///
/// A target that names a path of the Vault's is read from it. A Layout that tracks no Vault, or one
/// nobody has fetched, has no copy: what the run named is then read as something else, or refused.
fn tracked_layout(held: &Workspace) -> Option<(String, Layout)> {
    let layouts = held.layouts();
    let name = layouts.current().ok().flatten()?;
    let track = layouts.track(&name).ok().flatten()?;
    let dir = readonly_layout_dir(held, &track, VAULT_LAYOUT_NAME);

    Layout::open(&dir).ok().map(|layout| (track, layout))
}

/// The 7 hex characters a hash is drawn by.
fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

/// The paths `target` may name, in the order a run is likely to have meant them.
///
/// A name is written the way a shell writes one — against the directory the run was made in, or
/// absolute — while a Layout names a path from the Workspace root. Where the run was made is what
/// the name is read against first, since that is what the completion offered and what the commands
/// acting on the tree take. A relative name as it stands is read after, so a path written from the
/// root keeps working from anywhere in the tree. An absolute name has no such second reading: a
/// Layout path is always relative, and taking the leading separator off would name a path of the
/// root beginning with the name's own first component rather than the file that was named.
fn targets(target: &str, root: &Path, cwd: Option<&Path>) -> Vec<LayoutPath> {
    let mut paths = Vec::new();

    if let Some(cwd) = cwd {
        let root = normalize(root);
        let absolute = normalize(&resolve(cwd, target));

        if let Ok(relative) = absolute.strip_prefix(&root)
            && let Ok(path) = LayoutPath::from_relative(relative)
        {
            paths.push(path);
        }
    }

    if !Path::new(target).is_absolute()
        && let Ok(path) = LayoutPath::new(target)
        && !paths.contains(&path)
    {
        paths.push(path);
    }

    paths
}

/// The path `given` names, made absolute against the directory the run was made in.
fn resolve(cwd: &Path, given: &str) -> PathBuf {
    if Path::new(given).is_absolute() {
        PathBuf::from(given)
    } else {
        cwd.join(given)
    }
}

/// `path` with its `.` dropped and its `..` climbed, worked out lexically rather than on disk.
///
/// Working it out here, before the disk is asked anything, is what lets a name be read from where
/// the run was made even when the path it makes does not exist — a file the Layout names and the
/// tree has lost is exactly what a status is asked about.
fn normalize(path: &Path) -> PathBuf {
    let mut components: Vec<Component<'_>> = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if matches!(components.last(), Some(Component::Normal(_))) => {
                components.pop();
            }
            other => components.push(other),
        }
    }

    components.into_iter().collect()
}

/// Reads what `target` names, in the order a run is likely to have meant it.
///
/// Every path is read from where the run was made first, which is how a shell writes a name and how
/// `rola track`, `rola align` and the completion read one, and a relative one is read again as it
/// stands, which is how a Layout names a path from its root. The first reading that names something
/// is what is answered, so a name means the file beside the run when there is one, and keeps meaning
/// the file at the root otherwise. A `Uuid` and a hash are read by shape and name no path at all.
fn found(
    target: &str,
    layout: &Layout,
    remote: Option<&Layout>,
    held: &Workspace,
    cwd: Option<&Path>,
) -> Result<Found, String> {
    for path in targets(target, held.get_root(), cwd) {
        if let Some(id) = layout.id_of(&path) {
            return Ok(Found {
                key: Key::new(version_of(layout, id)?),
                editing: editing(held, layout, &path)?,
                id: Some(id),
            });
        }

        // A path the Layout does not name is still a path of this work when the tree reading finds a
        // path it does name moved there: a move's destination is how a run asks after the file that
        // moved, since that is where it is now. What is on disk is asked before the reading is, so a
        // hash — which also reads as a path — does not walk the tree for nothing.
        if held.get_root().join(path.to_path_buf()).is_file() {
            let diff = tree_diff(layout, held, DEFAULT_ALIKE).map_err(|error| error.to_string())?;

            if let Some(rename) = diff.renamed.iter().find(|rename| rename.to == path)
                && let Some(id) = layout.id_of(&rename.from)
            {
                return Ok(Found {
                    key: Key::new(version_of(layout, id)?),
                    editing: diff.modified.contains(&rename.to),
                    id: Some(id),
                });
            }
        }

        if let Some(remote) = remote
            && let Some(id) = remote.id_of(&path)
        {
            return Ok(Found {
                key: Key::new(version_of(remote, id)?),
                editing: false,
                id: Some(id),
            });
        }
    }

    if let Ok(id) = Uuid::from_str(target) {
        if let Some(path) = layout.path_of(id) {
            return Ok(Found {
                key: Key::new(version_of(layout, id)?),
                editing: editing(held, layout, &path)?,
                id: Some(id),
            });
        }

        if let Some(remote) = remote
            && remote.entry(id).is_some()
        {
            return Ok(Found {
                key: Key::new(version_of(remote, id)?),
                editing: false,
                id: Some(id),
            });
        }
    }

    // A `Uuid` and a path are read by shape, so what is left is a hash: which kind of object it is
    // is what reading it settles, and one that is no chain is refused where it is read.
    if let Some(key) = parse_hash(target) {
        return Ok(Found {
            key,
            editing: false,
            id: None,
        });
    }

    Err(t!("status.err_target_names_nothing").trim().to_owned())
}

/// The version the Layout names the entry `id` at.
///
/// A `Uuid` the Layout names but was never given a version for is one no chain hangs from: what was
/// recorded is what a lookback reads, and there is nothing recorded.
fn version_of(layout: &Layout, id: Uuid) -> Result<Blake3Hash, String> {
    let Some(data) = layout.entry(id) else {
        return Err(t!("status.err_target_no_version").trim().to_owned());
    };
    let version = data.version();

    if version == [0; 32] {
        return Err(t!("status.err_target_no_version").trim().to_owned());
    }

    Ok(version)
}

/// Whether the file at `path` has changes the Layout does not name yet.
///
/// A move whose file was edited is already among the modified: the tree reading counts a move's
/// destination there as well, since what changed is the content at the path the file is at now.
fn editing(held: &Workspace, layout: &Layout, path: &LayoutPath) -> Result<bool, String> {
    let diff = tree_diff(layout, held, DEFAULT_ALIKE).map_err(|error| error.to_string())?;

    Ok(diff.modified.contains(path))
}

/// One path that moved, as `status` shows it.
#[derive(Serialize)]
pub struct RenameItem {
    /// The path the Layout names it by.
    from: String,
    /// The path the tree holds it at.
    to: String,
    /// Whether it was edited as well as moved.
    modified: bool,
    /// Whether the move is one the two being the same bytes makes rather than one their likeness
    /// suggests.
    strong: bool,
}

/// Error: what the run named is nothing a chain can be looked back from.
#[derive(Grouped)]
pub struct ErrorStatusTarget {
    /// What the run named.
    pub target: String,
    /// Why nothing was read from it.
    pub cause: String,
}

impl Failure for ErrorStatusTarget {
    fn name(&self) -> &'static str {
        "error_status_target"
    }

    fn reason(&self) -> String {
        t!(
            "status.err_target",
            target = self.target,
            cause = self.cause
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorStatusTarget);

#[renderer(buffer)]
pub fn render_error_status_target(error: ErrorStatusTarget, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("status.err_target_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Result: how the tree stood beside the Layout.
///
/// The reading is the same one `rola layout tree-diff` gives, so what a `--json` run reads here is
/// what a reader of that command would: every path the two disagree about, with `modified` holding
/// the ones that changed where they were as well as the destinations of moves — except the ones the
/// fetched copy says are not this account's, which are held apart in `unowned` rather than counted
/// among the run's own work.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultStatus {
    /// Paths the Layout names that the tree does not hold.
    lost: Vec<String>,
    /// Paths the tree holds that the Layout does not name.
    untagged: Vec<String>,
    /// Paths that changed where they were, including the destinations of moves.
    modified: Vec<String>,
    /// Paths that moved.
    renamed: Vec<RenameItem>,
    /// Paths the fetched copy of the Vault's Layout says another account holds.
    unowned: Vec<String>,
    /// Files a variant is waiting to be joined into, and the variant waiting.
    merging: Vec<MergingItem>,
    /// Merges that cannot go on, one line each.
    broken: Vec<String>,
}

/// One variant waiting to be joined into one file.
#[derive(Serialize)]
pub struct MergingItem {
    /// The file it is to be joined into.
    target: String,
    /// The variant that was checked in.
    variant: String,
    /// The path its file lies at, or lay at before a move.
    path: String,
    /// Where the tree holds its file now, when a move was found.
    moved: Option<String>,
}

/// Result: one thing a variant is waiting to be joined into.
///
/// A variant file is no path the Layout names, so there is no chain to draw for it: what is said is
/// the variant it holds and the file it is waiting for, which is what a run that named it wanted.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultStatusMerge {
    /// The variant the file holds.
    variant: String,
    /// The path the variant file lies at.
    path: String,
    /// The file it is to be joined into, when the Layout still names one.
    target: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_status_merge(result: ResultStatusMerge, lookback: &ResLookback) {
    r_println!(
        "{}",
        trd!(t!(
            "status.merge_variant_from",
            variant = short(&result.variant),
            path = result.path
        ))
        .trim()
    );

    if lookback.hint()
        && let Some(target) = &result.target
    {
        r_println!(
            "{}",
            help_line!(t!("status.merge_variant_into", target = target).trim())
        );
    }
}

#[renderer(buffer)]
pub fn render_result_status(result: ResultStatus) {
    let structural = result.renamed.len() + result.lost.len() + result.untagged.len();

    // A move's destination is already a line of its own among the moves, so what is left for the
    // content that changed is everything modified that did not move.
    let moved: BTreeSet<&str> = result
        .renamed
        .iter()
        .map(|rename| rename.to.as_str())
        .collect();
    let changed: Vec<&str> = result
        .modified
        .iter()
        .filter(|path| !moved.contains(path.as_str()))
        .map(String::as_str)
        .collect();

    if structural == 0
        && changed.is_empty()
        && result.unowned.is_empty()
        && result.merging.is_empty()
        && result.broken.is_empty()
    {
        r_println!("{}", trd!(t!("status.clean")).trim());
    } else {
        let mark = t!("status.mark_modified");
        let unowned = !result.unowned.is_empty();
        let merging = !result.merging.is_empty() || !result.broken.is_empty();

        // What is being merged is said first: it is work already under way, and what the next
        // `rola track` of the file would record.
        if merging {
            r_println!(
                "{}",
                trd!(t!("status.header_merging", count = result.merging.len())).trim()
            );
            r_println!("{}", trd!(t!("status.body_merging")).trim());
            r_println!("");

            for item in &result.merging {
                let said = item.moved.as_ref().map_or_else(
                    || {
                        t!(
                            "status.merging_line",
                            target = item.target,
                            variant = short(&item.variant),
                            path = item.path
                        )
                    },
                    |to| {
                        t!(
                            "status.merging_line_moved",
                            target = item.target,
                            variant = short(&item.variant),
                            from = item.path,
                            to = to
                        )
                    },
                );

                r_println!("{}", trd!(said).trim());
            }

            for cause in &result.broken {
                r_println!("{}", err_line!(cause));
            }
        }

        // What must not be recorded is said first: a run that reads the rest as its work has read past
        // the one thing it is not to do.
        if unowned {
            if merging {
                r_println!("");
            }

            r_println!(
                "{}",
                trd!(t!("status.header_unowned", count = result.unowned.len())).trim()
            );
            r_println!("{}", trd!(t!("status.body_unowned")).trim());
            r_println!("");

            for path in &result.unowned {
                r_println!("{}", trd!(format!("  {path} {mark}")));
            }
        }

        if structural > 0 {
            // A blank line separates the blocks; the first block has none before it.
            if unowned || merging {
                r_println!("");
            }

            let header = if changed.is_empty() || result.renamed.is_empty() {
                trd!(t!("status.header_structural", count = structural))
            } else {
                trd!(t!(
                    "status.header_structural_matched",
                    count = structural,
                    renamed = result.renamed.len()
                ))
            };

            r_println!("{}", header.trim());
            r_println!("");

            for rename in &result.renamed {
                let mark = if rename.modified {
                    t!("status.mark_modified").to_string()
                } else {
                    String::new()
                };

                let line = format!("[[cyan]]>[[/]] {} -> {}{}", rename.from, rename.to, mark);
                r_println!("{}", trd!(line));
            }
            for path in &result.lost {
                r_println!("{}", trd!(format!("[[red]]-[[/]] {path}")));
            }
            for path in &result.untagged {
                r_println!("{}", trd!(format!("[[green]]+[[/]] {path}")));
            }
        }

        if !changed.is_empty() {
            // A blank line separates the blocks; the first block has none before it.
            if unowned || merging || structural > 0 {
                r_println!("");
            }

            let header = if structural == 0 {
                trd!(t!("status.header_content", count = changed.len()))
            } else {
                trd!(t!("status.header_content_more", count = changed.len()))
            };
            r_println!("{}", header.trim());
            r_println!("");

            for path in &changed {
                r_println!("{}", trd!(format!("  {path} {mark}")));
            }
        }

        r_println!("");

        if !result.unowned.is_empty() {
            r_println!("{}", help_line!(t!("status.help_unowned").trim()));
        }
        if merging {
            if result.merging.iter().any(|item| item.moved.is_some()) {
                r_println!("{}", help_line!(t!("status.help_merging_moved").trim()));
            }

            r_println!("{}", help_line!(t!("status.help_merging").trim()));
        }
        if structural > 0 {
            let said = if changed.is_empty() {
                t!("status.help_align")
            } else {
                t!("status.help_align_both")
            };
            r_println!("{}", help_line!(said.trim()));
        }
        if !changed.is_empty() {
            r_println!("{}", help_line!(t!("status.help_track").trim()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::targets;

    /// The paths a run in `cwd` makes of `target`, in the order they would be tried.
    fn candidates(target: &str, cwd: &str) -> Vec<String> {
        targets(target, Path::new("/ws"), Some(Path::new(cwd)))
            .into_iter()
            .map(|path| path.as_str().to_owned())
            .collect()
    }

    #[test]
    fn a_name_beside_the_run_is_read_from_where_the_run_was_made() {
        assert_eq!(
            candidates("hero.psd", "/ws/models"),
            vec!["models/hero.psd".to_owned(), "hero.psd".to_owned()]
        );
    }

    #[test]
    fn a_path_written_from_the_root_is_read_after_the_name_beside_the_run() {
        assert_eq!(
            candidates("models/hero.psd", "/ws/models"),
            vec![
                "models/models/hero.psd".to_owned(),
                "models/hero.psd".to_owned()
            ]
        );
    }

    #[test]
    fn a_name_read_at_the_root_is_read_once() {
        assert_eq!(
            candidates("models/hero.psd", "/ws"),
            vec!["models/hero.psd".to_owned()]
        );
    }

    #[test]
    fn a_run_that_climbs_out_of_its_directory_is_read_where_it_lands() {
        assert_eq!(
            candidates("../hero.psd", "/ws/models/sub"),
            vec!["models/hero.psd".to_owned()]
        );
    }

    #[test]
    fn an_absolute_name_is_read_from_the_root_once() {
        assert_eq!(
            candidates("/ws/models/hero.psd", "/elsewhere"),
            vec!["models/hero.psd".to_owned()]
        );
    }

    #[test]
    fn a_name_climbing_out_of_the_root_names_nothing() {
        assert!(candidates("../hero.psd", "/ws").is_empty());
    }

    #[test]
    fn a_run_that_cannot_say_where_it_is_reads_the_name_as_written() {
        let paths = targets("hero.psd", Path::new("/ws"), None);

        assert_eq!(
            paths
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .collect::<Vec<_>>(),
            vec!["hero.psd".to_owned()]
        );
    }
}

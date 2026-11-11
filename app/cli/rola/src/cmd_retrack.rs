//! The `rola retrack` command: set what a file's Layout entry records.
//!
//! A Layout names one version for each path, and the content behind a version stays in the index
//! whether or not anything points at it. What a run names after the file is what the path is to
//! record: a version (a step back, a number, or a hash) which moves the entry's pointer, or another
//! entry (a `Uuid`, or a path of the Vault's own Layout) which makes the path record that entry in
//! place of the one it recorded.
//!
//! Moving the pointer and leaving the file alone would leave the two apart, the file holds what a
//! newer version did, and the Layout now names an older one, so the version's content is written
//! back over the file as well, and the reading is told the two agree again, the way
//! [`crate::checkout`] tells it for a pull. `--only-index` is what a run gives when it wants the
//! record moved and the work left alone; what the tree then holds is what `rola status` reports as
//! a change that was never recorded.
//!
//! A file with changes that were never recorded, and one a variant is waiting to be joined into,
//! are not states a retrack that writes content may write over: both are refused. `--force` is how a
//! run says to go ahead anyway, for the first, the changes are discarded and the content switched;
//! for the second, the merge is ended, its variant file removed, and the content switched.

// A retrack reads a target, a Layout, a chain and a version before it moves one pointer, and every
// step of that is a refusal of its own to say; keeping them in one function is what lets the order
// they are read in be read off the function rather than assembled from helpers.
#![allow(clippy::too_many_lines)]
// The handler's parameters are the resources the framework injects, so how many of them there are
// is what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr as _;

use librorolala::auth::Account;
use librorolala::daemon::{Chain, action_bring_remote_index_async, action_fetch_layout};
use librorolala::ignore::IgnoreRuleSet;
use librorolala::layout::{EntryKind as LayoutEntryKind, Layout, LayoutPath, MutableData};
use librorolala::status::{tracked_vault, variant_moves};
use librorolala::storage::{Blake3Hash, Key, RorolalaStorage, StorageBackend as _};
use librorolala::tree::{EntryKind as TreeEntryKind, Staging, Tree, TreeStructure, read_structure};
use librorolala::vcs::{VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version};
use librorolala::workspace::{Merging, Pending, Workspace};
use librorolala::workspace_analyze::{Cache, cache_path, entry_of, tree_diff};
use mingling::{
    Grouped, LazyRes, Routable, ShellContext, StructuralData, Suggest,
    consts::UNMATCHED,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, PickerArg, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{
    ResConcurrency, ResCurrentRemoteVault, ResForce, ResOffline, ResProgressSetting,
    ResRorolalaStorage, ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rorolala_utils_progress::Progress;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::checkout::remember;
use crate::complete::{
    IndexObject, index_keys, offer, positional, strip_written, target_words, typing_flag,
};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_RETRACK, EC_ERR_RETRACK_ARGUMENT, EC_ERR_RETRACK_DIRTY, EC_HELP};
use crate::failure::failure;
use crate::fetch::{self, Sources};
use crate::hash::{HashMiss, resolve as resolve_hash};
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutShouldInWorkspace, chosen, failed as layout_failed, readonly_layout_copy,
    remote_spec, switch_entry,
};
use crate::progress::Reporting;

/// How alike two text files have to be to count as the same file moved.
///
/// It is the same default the reading uses, since what this acts on is what the reading found.
const DEFAULT_ALIKE: f32 = 0.6;

/// Leave the record where it is put and leave the file on disk alone.
const ARG_ONLY_INDEX: PickerArg<'static, Flag> = arg![only_index: Flag];

/// Refuse a version or an entry that is not on the way to what the path records now.
const ARG_DENY_JUMP: PickerArg<'static, Flag> = arg![deny_jump: Flag];

/// Record a fresh `Uuid` for the file, with what it records now.
const ARG_FORK: PickerArg<'static, Flag> = arg![fork: Flag];

/// The number of words a run may name beside its flags: the file, and the target.
const WORDS: usize = 2;

#[help(buffer)]
pub fn help_retrack(_: EntryRetrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("retrack.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryRetrack)]
pub fn desc_retrack() -> Description {
    t!("retrack.description").to_string().into()
}

/// Completes what `rola retrack` can be given next.
///
/// `FILE` is a local path the Layout records, or a path inside a tree the Layout records, so the filesystem
/// answers it. `TARGET` names what the path is to record: the versions the index holds, and the entries a
/// `Uuid` or a path of the fetched copy of the Vault's Layout can name. A step count or a version number is
/// not offered, since nothing here can say how far back a run means to go.
#[completion(EntryRetrack)]
pub fn complete_retrack(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    index: &mut LazyRes<ResVCSIndex>,
) -> Suggest {
    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_ONLY_INDEX: t!("retrack.complete.only_index"),
                ARG_DENY_JUMP: t!("retrack.complete.deny_jump"),
                ARG_FORK: t!("retrack.complete.fork"),
            },
        );
    }

    match positional(&ctx, "retrack") {
        0 => Suggest::file_comp(),
        1 => offer(
            &ctx,
            target_words(
                workspace.get_ref(),
                vault.get_ref(),
                index.get_ref().as_ref(),
            ),
        ),
        _ => suggest!(),
    }
}

/// Records what is named for a file, in the Layout being worked in
///
/// `FILE` is one local path in this Workspace (a `Uuid`, a hash and a remote name are refused) and
/// it has to be a path the Layout records, or a path inside a tree the Layout records. `TARGET` is
/// what the path is to record, and is one version back when it is not given.
///
/// A version target moves the entry's pointer: `~N` N versions back, `N` the version numbered N, or
/// a hash a version on the way back from where the entry is now, a hash off that way is a jump, and
/// `--deny-jump` is what refuses one. `~N` and `N` past either end of the chain are always refused.
/// The `Uuid`, the path and the description stay as they are, and nothing is removed from the index.
/// A version this index does not hold is brought from the Vault the Layout tracks before the pointer
/// moves, so a run may go to a version this side never fetched; `--offline` refuses that instead of
/// reaching for it.
///
/// An entry target (a `Uuid`, or a path of the Vault's own Layout) makes the path record that
/// entry in place of the one it recorded, which is dropped. The entry must hold no path of its own
/// and must record a version; what it holds is read from the fetched copy of the Vault's Layout, or
/// from this Layout when the `Uuid` is one it holds at no path. Whether the file changes follows how
/// the two versions stand: the same version or one the path's version is ahead of leaves the file as
/// it is, a version behind it has that version's content written over the file, and another chain is
/// a jump, which `--deny-jump` refuses.
///
/// `--fork` records a fresh `Uuid` for what the path records now and leaves the file, the index and
/// the Vault alone; it cannot be given with a target.
///
/// The version's content is written back over the file with the record, so the two agree again,
/// what a file standing as recorded is given without being asked, since the content it holds is the
/// run's to replace. `--only-index` moves the record and leaves the file alone, which is what a run
/// wants when the work on disk is not to be touched: `rola status` then reports the file as changed.
///
/// A file with changes that were never recorded is refused where the content is to be switched, and
/// so is one a variant is waiting to be joined into. `--force` goes ahead anyway: the changes are
/// discarded, and a merge is ended, its variant file removed, and the content switched. `--force`
/// and `--only-index` say opposite things and cannot be given together.
///
/// # Errors
///
/// Renders [`ErrorRetrackNoFile`] when no file is named, [`ErrorRetrackManyWords`] when more than a
/// file and a target are, [`ErrorRetrackFlags`] when `--force` and `--only-index` are given
/// together, [`ErrorRetrackForkTarget`] when `--fork` and a target are, [`ErrorRetrackFile`] when
/// what was named is not a local path the Layout records, [`ErrorRetrackTarget`] when `TARGET` names
/// no version or entry it can read, [`ErrorRetrackEntry`] when the entry it names is not one it can
/// record, [`ErrorRetrackSwitch`] when a crossed chain is what `--deny-jump` refuses,
/// [`ErrorRetrackNothing`] when the entry was never recorded, [`ErrorRetrackModified`] when the file
/// has changes the Layout does not name, [`ErrorRetrackMerging`] when a variant is waiting for the
/// file, [`ErrorOffline`] when a version or a copy is not here and the run is offline,
/// [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorRetrackFailed`] when the store, the index or the Layout refuses.
#[command(node = "retrack", entry = EntryRetrack)]
pub fn retrack(args: EntryRetrack, force: &ResForce) -> Next {
    let picked = args
        .pick(ARG_ONLY_INDEX)
        .pick(ARG_DENY_JUMP)
        .pick(ARG_FORK)
        .pick_or_route(arg![Vec<String>], || ErrorRetrackNoFile.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (only_index, deny_jump, fork, words, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    // A step count written `-N` is the one target a word of the command line cannot be: it is read
    // as a flag, so a run that meant `~N` is told the spelling that reads rather than that a flag it
    // never wrote is unknown.
    if let Some(word) = dash_number(&unmatched) {
        return ErrorRetrackTarget {
            target: word,
            kind: RetrackTargetError::DashNumber,
        }
        .into();
    }

    if let Some(next) = crate::unmatched::refused(&unmatched, "retrack") {
        return next;
    }

    // What a retrack takes is one file and, when it says what to record, one target: a run that
    // names more is refused rather than acted on for the first and quietly ignoring the rest.
    if words.len() > WORDS {
        return ErrorRetrackManyWords { count: words.len() }.into();
    }

    let mut words = words.into_iter();
    let Some(file) = words.next() else {
        return ErrorRetrackNoFile.into();
    };
    let target = words.next();

    let only_index = matches!(only_index, Flag::Active);

    // `--force` and `--only-index` say opposite things about the file on disk, so a run that gives
    // both is refused before anything is read rather than one of them winning silently.
    if **force && only_index {
        return ErrorRetrackFlags.into();
    }

    // A target names what the path is to record, and a fork is what the path records now under a new
    // `Uuid`: the two are different runs, and one that gives both says neither.
    let fork = matches!(fork, Flag::Active);
    if fork && target.is_some() {
        return ErrorRetrackForkTarget.into();
    }

    StateRetrack {
        file,
        target,
        fork,
        only_index,
        deny_jump: matches!(deny_jump, Flag::Active),
        force: **force,
    }
    .into()
}

/// The state a retracking starts in.
// What the run is to do is a handful of independent answers it was given, a fresh `Uuid`, leaving
// the file alone, refusing jumps, going past unrecorded changes, and making each a two-variant
// enum would say no more than the bool does while writing every use of it out twice.
#[allow(clippy::struct_excessive_bools)]
#[derive(Grouped)]
pub struct StateRetrack {
    /// The one file named, as it was written.
    file: String,
    /// What the path is to record, as it was written; one version back when it was not said.
    target: Option<String>,
    /// Whether the path's entry is given a fresh `Uuid`, holding what it records now.
    fork: bool,
    /// Whether the record moves and the file on disk is left alone.
    only_index: bool,
    /// Whether a version or an entry off the way to what the path records is refused.
    deny_jump: bool,
    /// Whether changes that were never recorded, and a merge in progress, are gone past anyway.
    force: bool,
}

/// What a run said the path is to record.
enum Recording {
    /// A version the entry is to be at, which moves its pointer.
    Version(VersionTarget),
    /// Another entry the path is to record, in place of the one it recorded.
    Entry(EntryTarget),
    /// A fresh `Uuid`, with what the path records now.
    Fork,
}

/// Where a version target moves the pointer to.
enum VersionTarget {
    /// N versions back.
    Steps(u64),
    /// The version numbered N.
    Number(u64),
    /// The version a hash names.
    Hash(Key),
}

/// What an entry target names.
enum EntryTarget {
    /// A `Uuid`, which needs no path to name an entry.
    Uuid(Uuid),
    /// A path of the Vault's own Layout.
    Upstream(LayoutPath),
}

/// The number a word written `-N` names, when it is one.
///
/// A step count is a word that begins with a dash, which the parser reads as a flag: what is wanted
/// is the `~N` spelling said back to the run rather than a refusal about a flag it did not write.
fn dash_number(words: &mingling::picker::UnmatchedArgs) -> Option<String> {
    words.into_iter().find_map(|word| {
        let digits = word.raw.strip_prefix('-')?;

        (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| word.raw.clone())
    })
}

/// Reads what a run said the path is to record.
///
/// The shapes are read in the order that cannot be mistaken: a step count and a version number are
/// digits, a `Uuid` is the one shape that names an entry without a path, a hash is settled among the
/// versions `versions` names, and anything left is a path of the Vault's own Layout. A hash that is
/// well formed but names no version is refused rather than read as a path, since a run that wrote one
/// meant the version.
///
/// # Errors
///
/// [`ErrorRetrackTarget`] when `text` is no version the run can move to, and when it is a step count
/// written with a dash.
fn recording_of(
    text: Option<&str>,
    versions: impl FnOnce() -> Vec<Key>,
) -> Result<Recording, ErrorRetrackTarget> {
    let Some(text) = text else {
        return Ok(Recording::Version(VersionTarget::Steps(1)));
    };

    let not_a_target = || ErrorRetrackTarget {
        target: text.to_owned(),
        kind: RetrackTargetError::NotATarget,
    };

    // Nothing at all names nothing either: a target is something, and an empty word is not.
    if text.is_empty() {
        return Err(not_a_target());
    }

    // A sign that is not a step count is a word the parser would read as an option, and one written
    // this way names no level: `~N` is how a step count is written as a word.
    if text.strip_prefix('-').is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return Err(ErrorRetrackTarget {
            target: text.to_owned(),
            kind: RetrackTargetError::DashNumber,
        });
    }
    if text.starts_with('-') || text.starts_with('+') {
        return Err(not_a_target());
    }

    if let Some(rest) = text.strip_prefix('~') {
        return rest
            .parse::<u64>()
            .map(VersionTarget::Steps)
            .map(Recording::Version)
            .map_err(|_| not_a_target());
    }

    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        return text
            .parse::<u64>()
            .map(VersionTarget::Number)
            .map(Recording::Version)
            .map_err(|_| not_a_target());
    }

    if let Ok(id) = Uuid::from_str(text) {
        return Ok(Recording::Entry(EntryTarget::Uuid(id)));
    }

    match resolve_hash(text, versions) {
        Ok(key) => return Ok(Recording::Version(VersionTarget::Hash(key))),
        Err(HashMiss::Malformed) => {}
        Err(HashMiss::Unknown) => {
            return Err(ErrorRetrackTarget {
                target: text.to_owned(),
                kind: RetrackTargetError::NotAVersion,
            });
        }
        Err(HashMiss::Ambiguous(candidates)) => {
            return Err(ErrorRetrackTarget {
                target: text.to_owned(),
                kind: RetrackTargetError::AmbiguousHash { candidates },
            });
        }
    }

    LayoutPath::new(text)
        .map(|path| Recording::Entry(EntryTarget::Upstream(path)))
        .map_err(|_| not_a_target())
}

#[chain(routeify)]
pub fn handle_retrack(
    state: StateRetrack,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResRorolalaStorage>,
    index: &mut LazyRes<ResVCSIndex>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: &ResOffline,
    concurrency: &ResConcurrency,
    progress: &ResProgressSetting,
) -> Next {
    let StateRetrack {
        file,
        target,
        fork,
        only_index,
        deny_jump,
        force,
    } = state;

    // The target is read before anything is looked at, so a run that wrote one the command cannot
    // use is told that rather than a complaint about a file it would have refused for another
    // reason. A fork names no target, the two cannot be given together, and is its own answer.
    let written = target.clone().unwrap_or_else(|| "~1".to_owned());
    let recording = if fork {
        Recording::Fork
    } else {
        recording_of(target.as_deref(), || {
            index_keys(index.get_ref().as_ref(), IndexObject::Version)
        })?
    };

    let layout = chosen(workspace.get_ref(), vault.get_ref(), None)?;

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorLayoutShouldInWorkspace.into();
    };

    let root = held.get_root().to_path_buf();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return failed(error.to_string()),
    };
    let path = locate(&root, &cwd, &file)?;
    // A tree is one Layout entry, so a path inside one names the tree's record: a retrack of a file
    // the tree holds is a retrack of the tree, which is the entry the Layout actually has.
    let path = tree_above(&layout, &path).unwrap_or(path);

    let Some(id) = layout.id_of(&path) else {
        return ErrorRetrackFile::new(&file, RetrackFileError::NotRecorded).into();
    };
    let Some(data) = layout.entry(id) else {
        return ErrorRetrackFile::new(&file, RetrackFileError::NotRecorded).into();
    };

    // The root version is what an entry is at before anything was recorded for it, so it is no
    // version to go back to: a file the Layout names this way has no record yet.
    if data.version() == [0; 32] {
        return ErrorRetrackNothing {
            path: path.as_str().to_owned(),
        }
        .into();
    }

    let Some(vcs) = index.get_ref().as_ref() else {
        return failed(t!("retrack.err_no_index").trim().to_owned());
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return failed(error.to_string()),
    };

    // A tree is one version unit rather than content at a path: what moves back is the whole
    // structure, and what the work holds is put back from it rather than from a content object.
    if data.kind() == LayoutEntryKind::Tree {
        let Some(store) = storage.get_ref().as_ref() else {
            return failed(t!("retrack.err_no_store").trim());
        };

        let mut asked = RetrackTree {
            recording: &recording,
            written: &written,
            layout: &layout,
            held,
            root: &root,
            path: &path,
            id,
            data: &data,
            vcs,
            runtime: &runtime,
            store,
            remote,
            current,
            offline: **offline,
            only_index,
            force,
            deny_jump,
        };

        return retrack_tree(&mut asked);
    }

    // The chain and the content may be fetched from a Vault, and what the exchanges do is watched
    // while they run: a retrack of a version this side never fetched is a pull of blocks, and it
    // reaches its end only after the bytes are here.
    let reporting = Reporting::start(*progress);

    // A run that names an entry, or a fork, is done before anything a version target is resolved
    // for: what it records is not a version of this entry but the entry the path is to be.
    let next = match recording {
        Recording::Version(level) => record_version(RecordingVersion {
            level: &level,
            written: &written,
            layout: &layout,
            held,
            root: &root,
            path: &path,
            id,
            data: &data,
            vcs,
            runtime: &runtime,
            storage,
            remote,
            current,
            offline: **offline,
            concurrency: **concurrency,
            progress: reporting.progress(),
            only_index,
            deny_jump,
            force,
        }),
        Recording::Entry(entry) => Ok(switch_uuid(Switching {
            entry: Some(entry),
            written: &written,
            layout: &layout,
            path: &path,
            old: id,
            data: &data,
            held,
            root: &root,
            vcs,
            runtime: &runtime,
            storage,
            remote,
            current,
            offline: **offline,
            concurrency: **concurrency,
            progress: reporting.progress(),
            only_index,
            deny_jump,
            force,
        })),
        Recording::Fork => Ok(switch_uuid(Switching {
            entry: None,
            written: &written,
            layout: &layout,
            path: &path,
            old: id,
            data: &data,
            held,
            root: &root,
            vcs,
            runtime: &runtime,
            storage,
            remote,
            current,
            offline: **offline,
            concurrency: **concurrency,
            progress: reporting.progress(),
            only_index,
            deny_jump,
            force,
        })),
    };

    reporting.finish();

    next?
}

/// What a run that moves a version pointer was asked for, and what it was asked about.
// The flags are the independent answers a run gave, and folding them into a value would only have
// to be unfolded at every use.
#[allow(clippy::struct_excessive_bools)]
struct RecordingVersion<'a> {
    /// Where the pointer goes.
    level: &'a VersionTarget,
    /// The target as it was written, for the words a refusal uses.
    written: &'a str,
    /// The Layout being worked in.
    layout: &'a Layout,
    /// The Workspace the run is in.
    held: &'a Workspace,
    /// The Workspace's root, where the working tree is.
    root: &'a Path,
    /// Where the entry sits in the Layout.
    path: &'a LayoutPath,
    /// The `Uuid` the path names.
    id: Uuid,
    /// What that entry holds now.
    data: &'a MutableData,
    /// The version control index.
    vcs: &'a VCSIndex,
    /// The runtime the index is waited on.
    runtime: &'a tokio::runtime::Runtime,
    /// The store the content is taken from.
    storage: &'a mut LazyRes<ResRorolalaStorage>,
    /// The Vault the run reaches for.
    remote: &'a mut LazyRes<ResCurrentRemoteVault>,
    /// The account the run acts as.
    current: &'a mut LazyRes<ResCurrentAccount>,
    /// Whether the run may reach a Vault at all.
    offline: bool,
    /// How many units of content a transfer carries at a time.
    concurrency: usize,
    /// What the transfers of a retrack say about themselves.
    progress: Progress,
    /// Whether the file on disk is to be left alone.
    only_index: bool,
    /// Whether a version off the way back is refused.
    deny_jump: bool,
    /// Whether unrecorded changes and a merge in progress are gone past anyway.
    force: bool,
}

/// Moves the version pointer of the entry the path names to the version `asked` names.
///
/// # Errors
///
/// Renders [`ErrorRetrackTarget`] when the version cannot be reached, [`ErrorRetrackModified`] when
/// the file has changes the Layout does not name, [`ErrorRetrackMerging`] when a variant is waiting
/// for the file, [`ErrorOffline`] when the version is not here and the run is offline, and
/// [`ErrorRetrackFailed`] when the store, the index or the Layout refuses.
///
/// The answer is the state the run ends in, and what a refusal is drawn from is the error it is
/// handed, the two are one value here because a command's handler answers with the one or the other.
fn record_version(asked: RecordingVersion<'_>) -> Result<Next, Next> {
    let RecordingVersion {
        level,
        written,
        layout,
        held,
        root,
        path,
        id,
        data,
        vcs,
        runtime,
        storage,
        remote,
        current,
        offline,
        concurrency,
        progress,
        only_index,
        deny_jump,
        force,
    } = asked;
    let here = read_version(vcs, runtime, data.version())?;
    let chain = ancestors(vcs, runtime, &here)?;

    // The version the pointer goes to may be one this index never fetched. It is checked here
    // before the target is resolved, so that a hash the Vault holds is a version to go to rather
    // than one this side cannot name, and so that the chain left behind is one that can be read.
    if let VersionTarget::Hash(key) = level {
        match presence(vcs, runtime, *key.digest()) {
            Presence::Version => {}
            Presence::Elsewhere => {
                return Err(ErrorRetrackTarget {
                    target: written.to_owned(),
                    kind: RetrackTargetError::NotAVersion,
                }
                .into());
            }
            Presence::Missing => {
                // Nothing the run asked for is here, and an offline run may not go looking for it.
                if offline {
                    return Err(ErrorOffline.into());
                }

                let bringer = Bringer {
                    workspace: held,
                    runtime,
                    end: vault_end(held, remote, current)?,
                    text: written,
                    progress: progress.clone(),
                };

                bringer.bring(*key.digest())?;
            }
        }
    }

    let asked = Asked {
        text: written,
        level,
        path: path.as_str(),
        deny_jump,
    };
    let (target, number) = resolve_target(vcs, runtime, &chain, &asked)?;

    let current_hash = data.version();
    let moved = target != current_hash;

    // What the run is to do to the file is worked out before anything is written, so a run that is
    // refused leaves the Layout, the tree and the merge exactly as they were.
    let mut merging = Merging::read(root);
    let pending = merging.at(id).cloned();
    // Retracking moves a version pointer of an entry the Layout names, which the ignore rules
    // have no jurisdiction over, so the reading is taken unfiltered.
    let diff = match tree_diff(layout, held, DEFAULT_ALIKE, &IgnoreRuleSet::empty()) {
        Ok(diff) => diff,
        Err(error) => return Err(failed(error.to_string())),
    };

    let action = match &pending {
        // A variant waiting for the file is a merge that was never recorded, and a pointer moved
        // under it would leave the merge with a file it does not match. `--only-index` cannot end
        // one, so it is refused rather than answered with a merge still in progress.
        Some(pending) if only_index || !force => {
            return Err(ErrorRetrackMerging {
                path: path.as_str().to_owned(),
                variant: hex(*pending.variant()),
            }
            .into());
        }
        None if only_index => Action::Pointer,
        // Changes the Layout does not name are the run's to discard or to leave alone, and they are
        // not something a retrack may decide by itself.
        None if !force && diff.modified.contains(path) => {
            return Err(ErrorRetrackModified {
                path: path.as_str().to_owned(),
            }
            .into());
        }
        _ => Action::Content,
    };

    if moved {
        let owner = data.owner().map(str::to_owned);

        if let Err(error) = layout.update_entry(id, data.renewed(owner, target)) {
            return Err(layout_failed(&error));
        }
    }

    // Content is read, and a Vault is bound to, only when the run writes it back, and only when
    // the store here does not already hold it: a retrack of content this Workspace has is local
    // work, and asking for an account would make it fail where there is nothing to fetch.
    let content = match action {
        Action::Pointer => None,
        Action::Content => Some(content_of(vcs, runtime, target)?),
    };
    let account = match content {
        Some(recorded) if !holds(storage, runtime, recorded) => {
            let bound = current.get_ref().must_bind().map_err(Into::<Next>::into)?;

            Some(account_named(&bound, Some(held), None).map_err(Into::<Next>::into)?)
        }
        _ => None,
    };
    let sources = match &account {
        Some(account) => {
            let tracked = tracked_vault(held);
            let primary = fetch::primary(remote.get_ref(), tracked.as_deref());
            let sources = Sources::new(held, account, remote.get_ref(), primary, offline)
                .map_err(failed)?
                .reporting(progress)
                .with_concurrency(concurrency);

            Some(sources)
        }
        None => None,
    };

    if let Some(recorded) = content {
        restore(
            storage,
            runtime,
            layout,
            root,
            path,
            recorded,
            sources.as_ref(),
        )?;

        // Only a forced run reaches here with a merge waiting, and forcing it is what ends it: the
        // variant file is the content that was just replaced, so the record and the file go with it.
        if let Some(pending) = &pending {
            let disk = variant_disk(
                &merging,
                layout,
                root,
                vcs,
                runtime,
                &diff.untagged,
                pending,
            );

            end_merge(&mut merging, root, id, disk)?;
        }

        if let Some(sources) = &sources {
            sources.report();
        }
    } else if moved {
        // What the file is to agree with now, which is the content the version moved to holds. One
        // the index does not hold is nothing to read, and the agreement is then taken back.
        let agreed = read_version(vcs, runtime, target)
            .ok()
            .and_then(|version| stored_hash(vcs, runtime, &version).ok());

        disagree(layout, root, path, agreed)?;
    }

    Ok(ResultRetrack {
        path: path.as_str().to_owned(),
        version: hex(target),
        number,
        changed: moved,
        switched: content.is_some(),
        from: None,
        to: None,
        forked: false,
    }
    .into())
}

/// What a run is to do to the file on disk, once the reading says it may be acted on.
enum Action {
    /// Move the pointer and leave the file alone.
    Pointer,
    /// Write the version's content back over the file, the pointer moving with it.
    Content,
}

/// What a run that records an entry for a path was asked for, and what it was asked about.
// What the run is to do is a handful of independent answers it was given, leaving the file alone,
// jumping between chains, going past unrecorded changes, and making each a two-variant enum would
// say no more than the bool does while writing every use of it out twice.
#[allow(clippy::struct_excessive_bools)]
struct Switching<'a> {
    /// The entry the path is to record; none when the run gives it a fresh `Uuid` of its own.
    entry: Option<EntryTarget>,
    /// The target as it was written, for the words a refusal uses.
    written: &'a str,
    /// The Layout being worked in.
    layout: &'a Layout,
    /// Where the entry sits in the Layout.
    path: &'a LayoutPath,
    /// The `Uuid` the path names now.
    old: Uuid,
    /// What that entry holds now.
    data: &'a MutableData,
    /// The Workspace the run is in.
    held: &'a Workspace,
    /// The Workspace's root, where the working tree is.
    root: &'a Path,
    /// The version control index.
    vcs: &'a VCSIndex,
    /// The runtime the index is waited on.
    runtime: &'a tokio::runtime::Runtime,
    /// The store the content is taken from.
    storage: &'a mut LazyRes<ResRorolalaStorage>,
    /// The Vault the run reaches for.
    remote: &'a mut LazyRes<ResCurrentRemoteVault>,
    /// The account the run acts as.
    current: &'a mut LazyRes<ResCurrentAccount>,
    /// Whether the run may reach a Vault at all.
    offline: bool,
    /// How many units of content a transfer carries at a time.
    concurrency: usize,
    /// What the transfers of a retrack say about themselves.
    progress: Progress,
    /// Whether the file on disk is to be left alone.
    only_index: bool,
    /// Whether a version off the way to the other one is refused.
    deny_jump: bool,
    /// Whether unrecorded changes and a merge in progress are gone past anyway.
    force: bool,
}

/// Records another `Uuid` for the path in `asked`, from a target that names an entry or from `--fork`.
///
/// What the path records is dropped and the entry it is to record takes its place, in one batch: an
/// entry is its `Uuid`, so the two are one change and not a removal and a creation a reader could
/// meet between. Nothing else is touched, the index, the stores and the working tree know nothing
/// of a Layout's paths, except the file the run is told to write back or to leave alone.
///
/// What is read is read before anything is written, so a refused run leaves the Layout, the tree and
/// the merge as they were.
///
/// # Errors
///
/// Renders [`ErrorRetrackEntry`] when the entry to record is not one the run can record,
/// [`ErrorRetrackSwitch`] when a crossed chain is what `--deny-jump` refuses,
/// [`ErrorRetrackModified`] when the file has changes the Layout does not name,
/// [`ErrorRetrackMerging`] when a variant is waiting for the file, [`ErrorOffline`] when a version or
/// a copy is not here and the run is offline, [`ErrorRetrackFailed`] when the store, the index or the
/// Layout refuses, and [`ErrorRetrackTarget`] when a version the run needs cannot be brought.
fn switch_uuid(asked: Switching<'_>) -> Next {
    let Switching {
        entry,
        written,
        layout,
        path,
        old,
        data,
        held,
        root,
        vcs,
        runtime,
        storage,
        remote,
        current,
        offline,
        concurrency,
        progress,
        only_index,
        deny_jump,
        force,
    } = asked;

    let mut merging = Merging::read(root);
    let pending = merging.at(old).cloned();

    // A variant waiting for the file is a merge that was never recorded, and an entry recorded under
    // another `Uuid` under it would leave the merge with a file it does not match. `--only-index`
    // cannot end one, so only `--force` goes past this.
    if let Some(pending) = &pending
        && !force
    {
        return ErrorRetrackMerging {
            path: path.as_str().to_owned(),
            variant: hex(*pending.variant()),
        }
        .into();
    }

    // What the path is to record, and what that entry holds: a fresh `Uuid` with what the path
    // records now, or the entry the target names.
    let (target, source) = match &entry {
        None => (Uuid::new_v4(), data.clone()),
        Some(entry) => {
            match recorded_entry(entry, written, layout, held, old, remote, current, offline) {
                Ok(target) => (target.id, target.data),
                Err(next) => return next,
            }
        }
    };

    // The file is left alone unless the versions say otherwise: an entry that records the same
    // version, or one ahead of what the target records, keeps the work on disk as it is. Whether the
    // file may be written at all is what the reading of it says below: standing as recorded, it is
    // written; holding changes that were never recorded, it is what `--force` is for.
    let action = match &entry {
        None => SwitchAction::Keep,
        Some(_) => match stand_of(
            data, &source, vcs, runtime, held, remote, current, offline, &progress,
        ) {
            Ok(relation) => match switch_action(relation, only_index, deny_jump) {
                Ok(action) => action,
                Err(SwitchRefusal::NotOnChain) => {
                    return ErrorRetrackSwitch {
                        path: path.as_str().to_owned(),
                    }
                    .into();
                }
            },
            Err(next) => return next,
        },
    };

    // What the new entry records: the version the path records now when that is what the file
    // holds, and the version the entry to record holds when the content is switched.
    let version = match action {
        SwitchAction::Keep => data.version(),
        SwitchAction::Pointer | SwitchAction::Content => source.version(),
    };
    let new_data = switched_data(&source, data, version);

    // Whether the file is to be written, and where the variant a merge is waiting for lies, are
    // both read from the tree as it stands before the Layout changes: a tree read after the change
    // has no entry naming the variant's file, so a move of one is not found again.
    let diff = if pending.is_some() || action == SwitchAction::Content {
        match tree_diff(layout, held, DEFAULT_ALIKE, &IgnoreRuleSet::empty()) {
            Ok(diff) => Some(diff),
            Err(error) => return failed(error.to_string()),
        }
    } else {
        None
    };
    let pending_disk = match (&pending, &diff) {
        (Some(pending), Some(diff)) => variant_disk(
            &merging,
            layout,
            root,
            vcs,
            runtime,
            &diff.untagged,
            pending,
        ),
        _ => None,
    };

    // Changes the Layout does not name are the run's to discard or to leave alone: a content switch
    // over them is what `--force` is for, and what a run without it is refused for.
    if action == SwitchAction::Content
        && !force
        && diff
            .as_ref()
            .is_some_and(|diff| diff.modified.contains(path))
    {
        return ErrorRetrackModified {
            path: path.as_str().to_owned(),
        }
        .into();
    }

    // The Layout is written before the content, so a run whose store is short of it leaves a record
    // naming a version the file does not hold, which `rola status` reports as a change, and a
    // second `rola retrack` puts right.
    if let Err(error) = switch_entry(layout, path, target, new_data) {
        return layout_failed(&error);
    }

    let switched = match action {
        SwitchAction::Keep => false,
        SwitchAction::Pointer => {
            // What the file is to agree with now: the content the version it records holds, when
            // that version can be read, and nothing at all when it cannot.
            let agreed = read_version(vcs, runtime, version)
                .ok()
                .and_then(|version| stored_hash(vcs, runtime, &version).ok());

            if let Err(next) = disagree(layout, root, path, agreed) {
                return next;
            }

            false
        }
        SwitchAction::Content => {
            let recorded = match content_of(vcs, runtime, version) {
                Ok(recorded) => recorded,
                Err(next) => return next,
            };

            // Content is read, and a Vault is bound to, only when the store here does not hold it.
            let account = if holds(storage, runtime, recorded) {
                None
            } else {
                let bound = match current.get_ref().must_bind() {
                    Ok(bound) => bound,
                    Err(next) => return next.into(),
                };

                match account_named(&bound, Some(held), None) {
                    Ok(account) => Some(account),
                    Err(next) => return next.into(),
                }
            };
            let sources = match &account {
                Some(account) => {
                    let tracked = tracked_vault(held);
                    let primary = fetch::primary(remote.get_ref(), tracked.as_deref());
                    let sources =
                        match Sources::new(held, account, remote.get_ref(), primary, offline) {
                            Ok(sources) => sources
                                .reporting(progress.clone())
                                .with_concurrency(concurrency),
                            Err(cause) => return failed(cause),
                        };

                    Some(sources)
                }
                None => None,
            };

            if let Err(next) = restore(
                storage,
                runtime,
                layout,
                root,
                path,
                recorded,
                sources.as_ref(),
            ) {
                return next;
            }

            if let Some(sources) = &sources {
                sources.report();
            }

            true
        }
    };

    // Only a forced run reaches here with a merge waiting, and forcing it is what ends it: the merge
    // was never recorded, so its record goes and the variant file it named goes with it.
    if pending.is_some()
        && let Err(next) = end_merge(&mut merging, root, old, pending_disk)
    {
        return next;
    }

    let number = read_version(vcs, runtime, version)
        .ok()
        .and_then(|version| runtime.block_on(vcs.version_num(&version)).ok());

    ResultRetrack {
        path: path.as_str().to_owned(),
        version: hex(version),
        number,
        changed: true,
        switched,
        from: Some(old.to_string()),
        to: Some(target.to_string()),
        forked: entry.is_none(),
    }
    .into()
}

/// The `Uuid` a run that records an entry is to record, and what that entry holds.
struct Entry {
    /// The `Uuid` the path is to record.
    id: Uuid,
    /// What it holds, from the copy of the Vault's Layout that has it or from this Layout.
    data: MutableData,
}

/// Reads what the entry a target names is to be recorded as.
///
/// A `Uuid` needs no path and no fetch: one this Layout holds at no path is the entry to record, and
/// the copy of the Vault's Layout is read only when it is here already. A path is read from that
/// copy, fetched first when it is not here, the one step of this that may leave the machine.
fn recorded_entry(
    target: &EntryTarget,
    written: &str,
    layout: &Layout,
    held: &Workspace,
    old: Uuid,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: bool,
) -> Result<Entry, Next> {
    let vault = tracked_vault(held);

    let (id, source) = match target {
        EntryTarget::Uuid(id) => {
            // The copy is used when it is here, and never fetched for a `Uuid`: a `Uuid` gives no
            // reason to reach anywhere, and the entry may be one this Layout holds at no path.
            let copy = match &vault {
                Some(name) => open_copy(held, name)?,
                None => None,
            };
            let source = copy
                .as_ref()
                .and_then(|copy| copy.entry(*id))
                .or_else(|| layout.entry(*id));

            (*id, source)
        }
        EntryTarget::Upstream(path) => {
            let Some(name) = vault else {
                return Err(ErrorRetrackEntry {
                    entry: written.to_owned(),
                    kind: RetrackEntryError::NoVault,
                }
                .into());
            };

            let copy = if let Some(copy) = open_copy(held, &name)? {
                copy
            } else {
                if offline {
                    return Err(ErrorOffline.into());
                }

                fetch_copy(held, remote, current, &name)?;

                match open_copy(held, &name)? {
                    Some(copy) => copy,
                    None => {
                        return Err(failed(format!(
                            "the copy of `{name}`'s Layout was fetched and is not here"
                        )));
                    }
                }
            };

            let Some(id) = copy.id_of(path) else {
                return Err(ErrorRetrackEntry {
                    entry: written.to_owned(),
                    kind: RetrackEntryError::NotUpstream { vault: name },
                }
                .into());
            };

            let source = copy.entry(id);

            (id, source)
        }
    };

    // The entry the path records already is the one case every other check would be told apart
    // from: it is named by a path, this one, so it is refused for being the entry it is.
    if id == old {
        return Err(ErrorRetrackEntry {
            entry: written.to_owned(),
            kind: RetrackEntryError::SameUuid {
                uuid: id.to_string(),
            },
        }
        .into());
    }
    if let Some(named) = layout.path_of(id) {
        return Err(ErrorRetrackEntry {
            entry: written.to_owned(),
            kind: RetrackEntryError::Named {
                uuid: id.to_string(),
                path: named.as_str().to_owned(),
            },
        }
        .into());
    }

    let Some(data) = source else {
        return Err(ErrorRetrackEntry {
            entry: written.to_owned(),
            kind: RetrackEntryError::UnknownUuid {
                uuid: id.to_string(),
            },
        }
        .into());
    };

    // The root version is what an entry is at before anything was recorded for it, so an entry that
    // records it records no version to switch to.
    if data.version() == [0; 32] {
        return Err(ErrorRetrackEntry {
            entry: written.to_owned(),
            kind: RetrackEntryError::NoVersion {
                uuid: id.to_string(),
            },
        }
        .into());
    }

    Ok(Entry { id, data })
}

/// Where the version a path records stands beside the version the entry it is to record is at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Relation {
    /// They are the same version.
    Equal,
    /// The version the path records is ahead: the other is behind it on its own chain.
    Ahead,
    /// The version the path records is behind: it is behind the other on the other's chain.
    Behind,
    /// They are on different chains.
    Apart,
}

/// The relation `target` has to `cur`, from the chains each version is the top of.
///
/// Both chains are the hashes of what is under each version, the nearest first, as [`ancestors`]
/// answers them, and each holds what is behind its own version rather than the version itself: what
/// is asked is whether one version is on the way down from the other.
fn relation(
    cur: [u8; 32],
    target: [u8; 32],
    cur_chain: &[[u8; 32]],
    target_chain: &[[u8; 32]],
) -> Relation {
    if cur == target {
        return Relation::Equal;
    }

    // The target is behind `cur` on the way down, so what the path records is the newer one.
    if cur_chain.contains(&target) {
        return Relation::Ahead;
    }
    if target_chain.contains(&cur) {
        return Relation::Behind;
    }

    Relation::Apart
}

/// How the version a path records stands beside the version the entry to record holds.
///
/// A version the index does not hold is brought from the Vault the Layout tracks, since a chain that
/// cannot be read cannot be compared, which is the one step of this that may leave the machine.
fn stand_of(
    data: &MutableData,
    target: &MutableData,
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    held: &Workspace,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: bool,
    progress: &Progress,
) -> Result<Relation, Next> {
    let cur = data.version();
    let target_version = target.version();
    if cur == target_version {
        return Ok(Relation::Equal);
    }

    let text = hex(target_version);
    match presence(vcs, runtime, target_version) {
        Presence::Version => {}
        Presence::Elsewhere => {
            return Err(failed(format!("the object {text} is not a version")));
        }
        Presence::Missing => {
            // Nothing the run asked for is here, and an offline run may not go looking for it.
            if offline {
                return Err(ErrorOffline.into());
            }

            let bringer = Bringer {
                workspace: held,
                runtime,
                end: vault_end(held, remote, current)?,
                text: &text,
                progress: progress.clone(),
            };

            bringer.bring(target_version)?;
        }
    }

    let here = read_version(vcs, runtime, cur)?;
    let there = read_version(vcs, runtime, target_version)?;
    let cur_chain = digests(&ancestors(vcs, runtime, &here)?);
    let target_chain = digests(&ancestors(vcs, runtime, &there)?);

    Ok(relation(cur, target_version, &cur_chain, &target_chain))
}

/// The hashes `chain` holds, in the order it holds them.
fn digests(chain: &[Version]) -> Vec<[u8; 32]> {
    chain
        .iter()
        .map(|version| *version.hash().digest())
        .collect()
}

/// What changing the `Uuid` a path records does to the file on disk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SwitchAction {
    /// Leave the file alone: it still holds what the entry it records holds.
    Keep,
    /// Record the entry and leave the file alone, the reading told the two disagree.
    Pointer,
    /// Write the recorded version's content over the file.
    Content,
}

/// The ways the two versions standing apart refuse a change of `Uuid`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SwitchRefusal {
    /// The two are on different chains, and the run asked to stay on the chain.
    NotOnChain,
}

/// What changing the `Uuid` a path records does, for how the two versions stand.
///
/// The two versions being on one chain is what decides how little is done: the file already holds
/// what the path records, so an entry that records the same version or a later one is a change of
/// identity and nothing more, and one that records an earlier version has that version's content
/// written over the file. Whether the file may be written is what the caller's reading of it decides
///, a file that stands as recorded is written without being asked, and one holding changes that were
/// never recorded is what `--force` is for. Different chains are not a step back at all, so they are
/// what a run does unless it asked to stay on the chain with `--deny-jump`.
fn switch_action(
    relation: Relation,
    only_index: bool,
    deny_jump: bool,
) -> Result<SwitchAction, SwitchRefusal> {
    match relation {
        Relation::Equal | Relation::Ahead => Ok(SwitchAction::Keep),
        Relation::Apart if deny_jump => Err(SwitchRefusal::NotOnChain),
        Relation::Behind | Relation::Apart if only_index => Ok(SwitchAction::Pointer),
        Relation::Behind | Relation::Apart => Ok(SwitchAction::Content),
    }
}

/// The data the entry a path comes to record is given.
///
/// Who holds it and what it says come from the entry being recorded, since that is the entry the
/// path comes to be. The cut it prefers stays the one the path's own entry preferred: how content is
/// written is about the file, not about the `Uuid` it is recorded under.
fn switched_data(source: &MutableData, old: &MutableData, version: [u8; 32]) -> MutableData {
    MutableData::new(
        source.owner().map(str::to_owned),
        version,
        source.description().to_owned(),
    )
    .with_preference(old.preference().map(str::to_owned))
}

/// Whether the store here already holds the content `recorded`.
///
/// A store that cannot answer is read as holding none of it, the way a fetch reads one: an asking
/// that turns out to be unnecessary costs a look-up, while the other way round leaves content that
/// is needed behind.
fn holds(
    storage: &mut LazyRes<ResRorolalaStorage>,
    runtime: &tokio::runtime::Runtime,
    recorded: Blake3Hash,
) -> bool {
    let Some(store) = storage.get_ref().as_ref() else {
        return false;
    };

    runtime
        .block_on(store.contains_keys(&[Key::new(recorded)]))
        .is_ok_and(|presence| presence.held(0))
}

/// The stored content the version `version` is of.
fn content_of(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    version: Blake3Hash,
) -> Result<Blake3Hash, Next> {
    let version = read_version(vcs, runtime, version)?;

    stored_hash(vcs, runtime, &version)
}

/// Ends the merge waiting for `target`: the record goes, and the variant file with it.
///
/// The record is written out before the file is removed, since a record that outlived its variant
/// file is a merge a reading reports as broken, while a variant file left by a write that failed is
/// only a path the tree holds untagged, and the target's content is replaced by the caller.
///
/// `disk` is where the variant file was found before the caller changed anything, read before the
/// change because a change of `Uuid` takes away the entry the merge names its file by.
///
/// # Errors
///
/// [`ErrorRetrackFailed`] when the merge in progress cannot be written back or the variant file
/// cannot be removed.
fn end_merge(
    merging: &mut Merging,
    root: &Path,
    target: Uuid,
    disk: Option<PathBuf>,
) -> Result<(), Next> {
    merging.remove(target);
    if let Err(error) = merging.write(root) {
        return Err(failed(error.to_string()));
    }

    // A variant file that is already gone is one the merge can still be ended from: what was checked
    // in is the content the record names, and the file was only how it was shown.
    if let Some(disk) = disk
        && disk.is_file()
        && let Err(error) = fs::remove_file(&disk)
    {
        return Err(failed(error.to_string()));
    }

    Ok(())
}

/// Where the variant file waiting for `pending` lies, when the tree still holds it.
///
/// The record names the place under the convention, but a run that moved the file with the
/// filesystem rather than with `rola fs-ops mv` leaves it elsewhere; it is found again by the
/// content it holds, the way a reading of the tree finds it.
fn variant_disk(
    merging: &Merging,
    layout: &Layout,
    root: &Path,
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    untagged: &[LayoutPath],
    pending: &Pending,
) -> Option<PathBuf> {
    let from = merging.variant_path(layout, pending)?;
    let disk = root.join(from.to_path_buf());
    if disk.is_file() {
        return Some(disk);
    }

    variant_moves(merging, layout, root, Some(index), runtime, untagged)
        .into_iter()
        .find(|moved| moved.target == pending.target())
        .map(|moved| root.join(moved.to.to_path_buf()))
}

/// What a run asked the pointer to be moved to, and what a refusal about it says.
struct Asked<'a> {
    /// The target as it was written, for the words a refusal uses.
    text: &'a str,
    /// Where the pointer is to go.
    level: &'a VersionTarget,
    /// The path the entry is at, for the words a refusal uses.
    path: &'a str,
    /// Whether a version off the way back is refused.
    deny_jump: bool,
}

/// The version the pointer is to be moved to, and its number when it is known.
///
/// A step count and a number are worked out from the chain the entry is the top of; a hash is that
/// chain when it names a version on it, and a version of the index otherwise, which is a jump the
/// run makes unless it asked to stay on the chain.
fn resolve_target(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    chain: &[Version],
    asked: &Asked<'_>,
) -> Result<(Blake3Hash, Option<u64>), Next> {
    let top = chain.len() - 1;
    let out_of_chain = || {
        ErrorRetrackTarget {
            target: asked.text.to_owned(),
            kind: RetrackTargetError::OutOfChain {
                path: asked.path.to_owned(),
            },
        }
        .into()
    };

    match asked.level {
        VersionTarget::Steps(back) => {
            let Ok(back) = usize::try_from(*back) else {
                return Err(out_of_chain());
            };
            let Some(version) = chain.get(back) else {
                return Err(out_of_chain());
            };

            Ok((*version.hash().digest(), Some(number_at(top, back))))
        }
        VersionTarget::Number(number) => {
            let Ok(number) = usize::try_from(*number) else {
                return Err(out_of_chain());
            };
            if number > top {
                return Err(out_of_chain());
            }

            let at = top - number;
            Ok((
                *chain[at].hash().digest(),
                Some(u64::try_from(number).unwrap_or(u64::MAX)),
            ))
        }
        VersionTarget::Hash(key) => {
            if let Some((at, version)) = chain
                .iter()
                .enumerate()
                .find(|(_, version)| version.hash().digest() == key.digest())
            {
                return Ok((*version.hash().digest(), Some(number_at(top, at))));
            }

            let Ok(version) = read_version(index, runtime, *key.digest()) else {
                return Err(ErrorRetrackTarget {
                    target: asked.text.to_owned(),
                    kind: RetrackTargetError::NotAVersion,
                }
                .into());
            };

            if asked.deny_jump {
                return Err(ErrorRetrackTarget {
                    target: asked.text.to_owned(),
                    kind: RetrackTargetError::NotAncestor {
                        path: asked.path.to_owned(),
                    },
                }
                .into());
            }

            let number = runtime.block_on(index.version_num(&version)).ok();
            Ok((*key.digest(), number))
        }
    }
}

/// The copy of `vault`'s own Layout the Workspace keeps, opened, or nothing when it was not fetched.
///
/// A copy that is not here is nothing rather than a failure: a run that has not fetched reads no
/// copy here, and whether one is fetched is the caller's to say.
///
/// # Errors
///
/// [`ErrorRetrackFailed`] when a copy that is there cannot be read.
fn open_copy(held: &Workspace, vault: &str) -> Result<Option<Layout>, Next> {
    let Some(dir) = readonly_layout_copy(held, vault, VAULT_LAYOUT_NAME) else {
        return Ok(None);
    };

    // A directory the Layout's own files are not in is not a copy, and reading it must not make one.
    match Layout::open_existing(&dir) {
        Ok(copy) => Ok(Some(copy)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(failed(error.to_string())),
    }
}

/// Fetches `vault`'s own Layout into the read-only copy the Workspace keeps.
///
/// # Errors
///
/// [`ErrorRetrackFailed`] when the Vault, its address or an account cannot be read, and whatever the
/// fetch refuses with.
fn fetch_copy(
    held: &Workspace,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    vault: &str,
) -> Result<(), Next> {
    let name = remote
        .get_ref()
        .name_or_default(vault)
        .map_err(mingling::Routable::to_chain)?;
    let target = remote
        .get_ref()
        .vault_or_default(name.clone())
        .map_err(mingling::Routable::to_chain)?;
    let account_name = current.get_ref().must_bind().map_err(Into::<Next>::into)?;
    let account = account_named(&account_name, Some(held), None).map_err(Into::<Next>::into)?;

    action_fetch_layout(held, &account, target.to_string(), name)
        .map_err(mingling::Routable::to_chain)?;

    Ok(())
}

/// Whether this index holds the version under a hash, holds something else there, or holds nothing.
enum Presence {
    /// The version is here, so a run that goes to it needs nothing brought.
    Version,
    /// The index holds an object under the hash, and it is not a version.
    Elsewhere,
    /// The index holds nothing under the hash.
    Missing,
}

/// What a version this index does not hold is read through: the Vault, and the account it is read as.
struct VaultEnd {
    /// The address the Vault is reached at.
    address: String,
    /// The account the read is spoken as.
    account: Account,
}

/// Reads the chain a version is the top of into this index, from the Vault that holds it.
///
/// A retrack may go to a version the Vault holds and this side never fetched. The chain under it, /// the version, the variant that produced it, the words they are named by, and so on down to a version
/// this index already holds, is asked for in one exchange, and what the Vault walks is written here
/// as one pack: what is left behind is a chain as readable here as one a sync brought.
struct Bringer<'a> {
    /// The Workspace the exchange is spoken from.
    workspace: &'a Workspace,
    /// The runtime the exchange is waited on.
    runtime: &'a tokio::runtime::Runtime,
    /// Where the objects are read from.
    end: VaultEnd,
    /// The target as the run wrote it, for the refusal that names it.
    text: &'a str,
    /// What the transfer says about itself while it runs.
    progress: Progress,
}

impl Bringer<'_> {
    /// Walks the chain from `hash` down to a version this index already holds, writing each object.
    ///
    /// One exchange rather than one per object: the Vault walks the chain itself, down to the first
    /// version this side already holds, and sends the whole of what is missing, which is one
    /// connection and one pack here however long the chain is.
    ///
    /// # Errors
    ///
    /// [`ErrorRetrackTarget`] when the Vault answers with something that is not a version, and
    /// [`ErrorRetrackFailed`] when the exchange, the index or the chain refuses.
    fn bring(&self, hash: Blake3Hash) -> Result<(), Next> {
        let said = self
            .runtime
            .block_on(action_bring_remote_index_async(
                self.workspace,
                &self.end.account,
                self.end.address.clone(),
                Key::new(hash).hex(),
                self.progress.clone(),
            ))
            .map_err(|error| failed(error.to_string()))?;

        let chain: Chain =
            serde_json::from_str(&said).map_err(|error| failed(error.to_string()))?;

        match chain {
            Chain::Brought => Ok(()),
            Chain::NotAVersion => Err(ErrorRetrackTarget {
                target: self.text.to_owned(),
                kind: RetrackTargetError::NotAVersion,
            }
            .into()),
            Chain::NotAVariant(key) => Err(failed(format!(
                "the object {key} is not the variant of the version"
            ))),
            Chain::Missing(key) => Err(failed(format!("nothing is stored under the hash {key}"))),
        }
    }
}

/// Whether this index holds a version under `hash`.
fn presence(vcs: &VCSIndex, runtime: &tokio::runtime::Runtime, hash: Blake3Hash) -> Presence {
    match runtime.block_on(vcs.read(Key::new(hash))) {
        Ok(VCSIndexObject::Version(_)) => Presence::Version,
        Ok(_) => Presence::Elsewhere,
        Err(_) => Presence::Missing,
    }
}

/// Reads where a version missing here is brought from, and as whom.
///
/// A hash names no Vault, so the Vault the Layout tracks is the one asked.
///
/// # Errors
///
/// [`ErrorRetrackFailed`] when the Layout tracks no Vault or the Vault or account cannot be read.
fn vault_end(
    held: &Workspace,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Result<VaultEnd, Next> {
    let name =
        tracked_vault(held).ok_or_else(|| failed(t!("retrack.err_no_vault").trim().to_owned()))?;
    let name = remote
        .get_ref()
        .name_or_default(name)
        .map_err(mingling::Routable::to_chain)?;
    let address = remote
        .get_ref()
        .vault_or_default(name)
        .map_err(mingling::Routable::to_chain)?;
    let account_name = current.get_ref().must_bind().map_err(Into::<Next>::into)?;
    let account = account_named(&account_name, Some(held), None).map_err(Into::<Next>::into)?;

    Ok(VaultEnd {
        address: address.to_string(),
        account,
    })
}

/// Writes the version's content back over the file, and remembers that the two agree.
///
/// `recorded` is the content the version is of, which is what the store holds it under. The store
/// may not hold it: what is not here is brought from a Vault first, which is the one step of a
/// retrack that may leave the machine, and `sources` is the run that may do it, nothing when the
/// content was already here and there is nothing to ask for.
fn restore(
    storage: &mut LazyRes<ResRorolalaStorage>,
    runtime: &tokio::runtime::Runtime,
    layout: &Layout,
    root: &Path,
    path: &LayoutPath,
    recorded: Blake3Hash,
    sources: Option<&Sources<'_>>,
) -> Result<(), Next> {
    let Some(store) = storage.get_ref().as_ref() else {
        return Err(failed(t!("retrack.err_no_store").trim().to_owned()));
    };

    let disk = root.join(path.to_path_buf());

    if let Some(parent) = disk.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return Err(failed(error.to_string()));
    }

    if let Some(sources) = sources {
        sources.bring(store, &[Key::new(recorded)]);
    }

    if let Err(error) = runtime.block_on(store.extract_file(&Key::new(recorded), &disk)) {
        return Err(failed(error.to_string()));
    }

    // What was written is what the Layout names again, so the reading has to be told: a path left
    // marked as disagreed would be reported as changed by the very next reading.
    if let Err(cause) = remember(layout, root, path.as_str()) {
        return Err(failed(cause));
    }

    Ok(())
}

/// Writes down what the file is to agree with now that the Layout names an older version.
///
/// Moving the pointer and leaving the file alone leaves the two apart, the file is what the newer
/// version held, and the Layout names an older one, but the reading does not tell that from the
/// pointer: what it compares is the file against what the Layout was last known to agree with. So
/// the agreement is written afresh here rather than left standing, or the very next `rola status`
/// would call a file with changes never recorded one with nothing to record.
///
/// What is written is the content the new version holds, so a file that is then put back to it by
/// hand reads as agreed again, a plain "nothing" would leave such a file reported as changed with
/// `rola track` having nothing to say about it. The content of a version the index does not hold is
/// nothing that can be read, so the agreement is taken back rather than guessed at.
///
/// A path the tree does not hold is nothing to write down: it is read as lost however the cache
/// stands, and there is no file to read a stamp from.
fn disagree(
    layout: &Layout,
    root: &Path,
    path: &LayoutPath,
    agreed: Option<Blake3Hash>,
) -> Result<(), Next> {
    let disk = root.join(path.to_path_buf());
    if !disk.is_file() {
        return Ok(());
    }

    let file = cache_path(root, layout);
    let mut cache = Cache::read(&file);
    let entry = entry_of(&disk).map_err(|error| failed(error.to_string()))?;

    cache.insert(path.clone(), entry.with_agreed(agreed));
    cache
        .write(&file)
        .map_err(|error| failed(error.to_string()))
}

/// The versions from `top` back to the first, the top first.
fn ancestors(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    top: &Version,
) -> Result<Vec<Version>, Next> {
    let mut chain = Vec::new();
    let mut current = top.clone();

    while !current.is_root() {
        let variant = read_variant(vcs, runtime, *current.variant())?;
        chain.push(current);
        current = read_version(vcs, runtime, *variant.base_version())?;
    }

    Ok(chain)
}

/// The number of the version `at` steps back from a chain whose top is numbered `top`.
fn number_at(top: usize, at: usize) -> u64 {
    u64::try_from(top.saturating_sub(at)).unwrap_or(u64::MAX)
}

/// The `Version` stored under `hash`.
fn read_version(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: Blake3Hash,
) -> Result<Version, Next> {
    runtime
        .block_on(vcs.read(Key::new(hash)))
        .map_err(|error| failed(error.reason()))?
        .expect_version()
        .map_err(|error| failed(error.to_string()))
}

/// The `Variant` stored under `hash`.
fn read_variant(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: Blake3Hash,
) -> Result<Variant, Next> {
    runtime
        .block_on(vcs.read(Key::new(hash)))
        .map_err(|error| failed(error.reason()))?
        .expect_variant()
        .map_err(|error| failed(error.to_string()))
}

/// The stored hash the variant of `version` points at.
fn stored_hash(
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    version: &Version,
) -> Result<Blake3Hash, Next> {
    let variant = read_variant(vcs, runtime, *version.variant())?;

    Ok(*variant.storage_hash())
}

/// The path the run named, read as one the Layout being worked in names.
///
/// Only a local path is taken. A `Uuid`, a hash and a remote name are refused by shape rather than
/// answered as a path that happens not to be there, since a run that wrote one meant the other
/// command that takes one. A name holds `@` often enough, `logo@2x.png`, that a plain one is
/// left to be a path: what is refused is the Vault-side marker and the one name a Vault's Layout
/// is kept under.
fn locate(root: &Path, cwd: &Path, given: &str) -> Result<LayoutPath, Next> {
    if Uuid::from_str(given).is_ok() || Key::from_str(given).is_ok() || is_remote(given) {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::NotLocal).into());
    }

    let root = normalize(root);
    let disk = normalize(&resolve(cwd, given));

    let Ok(relative) = disk.strip_prefix(&root) else {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::OutsideWorkspace).into());
    };

    let Ok(path) = LayoutPath::from_relative(relative) else {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::OutsideWorkspace).into());
    };

    if path.as_str().split('/').any(|part| part == ".rola") {
        return Err(ErrorRetrackFile::new(given, RetrackFileError::InsideData).into());
    }

    Ok(path)
}

/// The tree whose Layout entry holds `path`, when one does.
///
/// It is the path itself when the path is a tree, and its nearest recorded ancestor otherwise: a file
/// inside a tree is not an entry of its own, and a retrack names the tree's record rather than a path
/// the Layout does not have.
fn tree_above(layout: &Layout, path: &LayoutPath) -> Option<LayoutPath> {
    let parts: Vec<&str> = path.as_str().split('/').collect();

    for end in (1..=parts.len()).rev() {
        let Ok(candidate) = LayoutPath::new(&parts[..end].join("/")) else {
            continue;
        };
        let Some(id) = layout.id_of(&candidate) else {
            continue;
        };
        let Some(data) = layout.entry(id) else {
            continue;
        };

        if data.kind() == LayoutEntryKind::Tree {
            return Some(candidate);
        }
    }

    None
}

/// Whether `given` is a name of the Vault's side rather than a path in this tree.
fn is_remote(given: &str) -> bool {
    // `@` is the marker a Vault's own path starts with; `truth@VAULT` is how a Vault's one Layout
    // is named. A file of one's own may hold an `@` and is left to be read as the path it is.
    given.starts_with('@')
        || matches!(remote_spec(given), Some((layout, _)) if layout == VAULT_LAYOUT_NAME)
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
/// A name is written against where the run was made, so `..` climbs out of the Workspace as
/// readily as it climbs into it, and what is then left is refused for being outside it.
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

/// The stored hash `hash` is written as, in hex.
fn hex(hash: Blake3Hash) -> String {
    Key::new(hash).hex()
}

/// The answer a store, an index or a Layout refusing is given.
fn failed(cause: impl Into<String>) -> Next {
    ErrorRetrackFailed {
        cause: cause.into(),
    }
    .into()
}

/// Names a version for a person: its number when it is known, and the head of its hash.
fn named(version: &str, number: Option<u64>) -> String {
    let short = &version[..version.len().min(7)];

    number.map_or_else(|| short.to_owned(), |number| format!("V{number} ({short})"))
}

/// Result: what a file's Layout entry records was set.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultRetrack {
    /// Where the entry sits in the Layout.
    path: String,
    /// The version it records now, as hex.
    version: String,
    /// That version's number in its chain, when it is known.
    number: Option<u64>,
    /// Whether what the path records is not what it recorded before.
    changed: bool,
    /// Whether the version's content was written back over the file.
    switched: bool,
    /// The `Uuid` the path recorded before, when the run recorded an entry.
    from: Option<String>,
    /// The `Uuid` the path records now, when the run recorded an entry.
    to: Option<String>,
    /// Whether the run gave the entry a fresh `Uuid` rather than recording another entry's.
    forked: bool,
}

#[renderer(buffer)]
pub fn render_result_retrack(result: ResultRetrack) {
    let version = named(&result.version, result.number);

    // What a run that recorded an entry says is about the two `Uuid`s, since the version it records
    // is only part of what changed; a version move says where the pointer is now.
    let said = match (&result.from, &result.to) {
        (Some(from), Some(to)) => {
            if result.forked {
                t!(
                    "retrack.result_uuid_forked",
                    path = result.path,
                    to = to,
                    version = version
                )
            } else if result.switched {
                t!(
                    "retrack.result_uuid_switched",
                    path = result.path,
                    from = from,
                    to = to,
                    version = version
                )
            } else {
                t!(
                    "retrack.result_uuid_kept",
                    path = result.path,
                    from = from,
                    to = to,
                    version = version
                )
            }
        }
        _ => match (result.changed, result.switched) {
            (true, true) => t!(
                "retrack.result_retracked",
                path = result.path,
                version = version
            ),
            (true, false) => t!(
                "retrack.result_retracked_index",
                path = result.path,
                version = version
            ),
            (false, false) => t!(
                "retrack.result_unchanged",
                path = result.path,
                version = version
            ),
            (false, true) => t!(
                "retrack.result_unchanged_switched",
                path = result.path,
                version = version
            ),
        },
    };

    r_println!("{}", said.trim());
}

/// Error: `--fork` and a target say different things about what the path is to record.
#[derive(Grouped)]
pub struct ErrorRetrackForkTarget;

impl Failure for ErrorRetrackForkTarget {
    fn name(&self) -> &'static str {
        "error_retrack_fork_target"
    }

    fn reason(&self) -> String {
        t!("retrack.err_fork_target").trim().to_string()
    }
}

failure!(ErrorRetrackForkTarget);

#[renderer(buffer)]
pub fn render_error_retrack_fork_target(_: ErrorRetrackForkTarget, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("retrack.err_fork_target").trim()));
    r_eprintln!("{}", help_line!(t!("retrack.err_fork_target_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: the target names no entry the run can record.
#[derive(Grouped)]
pub struct ErrorRetrackEntry {
    /// What was written as the target.
    entry: String,
    /// What is wrong with it.
    kind: RetrackEntryError,
}

/// The ways a target names no entry to record.
pub enum RetrackEntryError {
    /// It is a path, and the Layout being worked in tracks no Vault.
    NoVault,
    /// The Vault's Layout names no such path.
    NotUpstream {
        /// The Vault whose Layout names none.
        vault: String,
    },
    /// Neither this Layout nor the copy of the Vault's Layout holds the `Uuid`.
    UnknownUuid {
        /// The `Uuid` nothing holds.
        uuid: String,
    },
    /// The `Uuid` is the one the path records already.
    SameUuid {
        /// The `Uuid` the path records.
        uuid: String,
    },
    /// The `Uuid` is recorded under another path.
    Named {
        /// The `Uuid` another path records.
        uuid: String,
        /// The path that records it.
        path: String,
    },
    /// The entry records no version, so there is nothing to switch to.
    NoVersion {
        /// The `Uuid` whose entry records no version.
        uuid: String,
    },
}

impl Failure for ErrorRetrackEntry {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackEntryError::NoVault => "error_retrack_entry_no_vault",
            RetrackEntryError::NotUpstream { .. } => "error_retrack_entry_not_upstream",
            RetrackEntryError::UnknownUuid { .. } => "error_retrack_entry_unknown_uuid",
            RetrackEntryError::SameUuid { .. } => "error_retrack_entry_same_uuid",
            RetrackEntryError::Named { .. } => "error_retrack_entry_named",
            RetrackEntryError::NoVersion { .. } => "error_retrack_entry_no_version",
        }
    }

    fn reason(&self) -> String {
        let said = match &self.kind {
            RetrackEntryError::NoVault => {
                t!("retrack.err_entry_no_vault", entry = self.entry)
            }
            RetrackEntryError::NotUpstream { vault } => t!(
                "retrack.err_entry_not_upstream",
                entry = self.entry,
                vault = vault
            ),
            RetrackEntryError::UnknownUuid { uuid } => {
                t!("retrack.err_entry_unknown_uuid", uuid = uuid)
            }
            RetrackEntryError::SameUuid { uuid } => {
                t!("retrack.err_entry_same_uuid", uuid = uuid)
            }
            RetrackEntryError::Named { uuid, path } => {
                t!("retrack.err_entry_named", uuid = uuid, path = path)
            }
            RetrackEntryError::NoVersion { uuid } => {
                t!("retrack.err_entry_no_version", uuid = uuid)
            }
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackEntry);

#[renderer(buffer)]
pub fn render_error_retrack_entry(error: ErrorRetrackEntry, ec: &mut ResExitCode) {
    let help = match error.kind {
        RetrackEntryError::NoVault => t!("retrack.err_entry_no_vault_help"),
        RetrackEntryError::NotUpstream { .. } => t!("retrack.err_entry_not_upstream_help"),
        RetrackEntryError::UnknownUuid { .. } => t!("retrack.err_entry_unknown_uuid_help"),
        RetrackEntryError::SameUuid { .. } => t!("retrack.err_entry_same_uuid_help"),
        RetrackEntryError::Named { .. } => t!("retrack.err_entry_named_help"),
        RetrackEntryError::NoVersion { .. } => t!("retrack.err_entry_no_version_help"),
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: the two versions are on different chains, and the run asked to stay on the chain.
#[derive(Grouped)]
pub struct ErrorRetrackSwitch {
    /// Where the entry sits in the Layout.
    path: String,
}

impl Failure for ErrorRetrackSwitch {
    fn name(&self) -> &'static str {
        "error_retrack_switch_not_on_chain"
    }

    fn reason(&self) -> String {
        t!("retrack.err_switch_not_on_chain", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackSwitch);

#[renderer(buffer)]
pub fn render_error_retrack_switch(error: ErrorRetrackSwitch, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("retrack.err_switch_not_on_chain_help").trim())
    );
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: no file was named.
#[derive(Grouped)]
pub struct ErrorRetrackNoFile;

impl Failure for ErrorRetrackNoFile {
    fn name(&self) -> &'static str {
        "error_retrack_no_file"
    }

    fn reason(&self) -> String {
        t!("retrack.err_no_file").trim().to_string()
    }
}

failure!(ErrorRetrackNoFile);

#[renderer(buffer)]
pub fn render_error_retrack_no_file(_: ErrorRetrackNoFile, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("retrack.err_no_file").trim()));
    r_eprintln!("{}", help_line!(t!("retrack.err_no_file_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: more words were named than a file and a target.
#[derive(Grouped)]
pub struct ErrorRetrackManyWords {
    /// How many were named.
    count: usize,
}

impl Failure for ErrorRetrackManyWords {
    fn name(&self) -> &'static str {
        "error_retrack_many_words"
    }

    fn reason(&self) -> String {
        t!("retrack.err_many_words", count = self.count)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackManyWords);

#[renderer(buffer)]
pub fn render_error_retrack_many_words(error: ErrorRetrackManyWords, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_many_words_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: `--force` and `--only-index` were given together.
#[derive(Grouped)]
pub struct ErrorRetrackFlags;

impl Failure for ErrorRetrackFlags {
    fn name(&self) -> &'static str {
        "error_retrack_flags"
    }

    fn reason(&self) -> String {
        t!("retrack.err_flags").trim().to_string()
    }
}

failure!(ErrorRetrackFlags);

#[renderer(buffer)]
pub fn render_error_retrack_flags(_: ErrorRetrackFlags, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("retrack.err_flags").trim()));
    r_eprintln!("{}", help_line!(t!("retrack.err_flags_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: a file named cannot be retracked.
#[derive(Grouped)]
pub struct ErrorRetrackFile {
    /// What was named.
    path: String,
    /// What is wrong with it.
    kind: RetrackFileError,
}

impl ErrorRetrackFile {
    /// The failure of retracking `path`, for the reason `kind` names.
    fn new(path: &str, kind: RetrackFileError) -> Self {
        Self {
            path: path.to_owned(),
            kind,
        }
    }
}

/// The ways a named file cannot be retracked.
pub enum RetrackFileError {
    /// It is a `Uuid`, a hash or a remote name rather than a local path.
    NotLocal,
    /// It is not inside the Workspace.
    OutsideWorkspace,
    /// It is part of the Workspace's own data rather than its work.
    InsideData,
    /// The Layout names nothing there.
    NotRecorded,
}

impl Failure for ErrorRetrackFile {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackFileError::NotLocal => "error_retrack_not_local",
            RetrackFileError::OutsideWorkspace => "error_retrack_outside_workspace",
            RetrackFileError::InsideData => "error_retrack_inside_data",
            RetrackFileError::NotRecorded => "error_retrack_not_recorded",
        }
    }

    fn reason(&self) -> String {
        let said = match self.kind {
            RetrackFileError::NotLocal => t!("retrack.err_not_local", path = self.path),
            RetrackFileError::OutsideWorkspace => {
                t!("retrack.err_outside_workspace", path = self.path)
            }
            RetrackFileError::InsideData => t!("retrack.err_inside_data", path = self.path),
            RetrackFileError::NotRecorded => t!("retrack.err_not_recorded", path = self.path),
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackFile);

#[renderer(buffer)]
pub fn render_error_retrack_file(error: ErrorRetrackFile, ec: &mut ResExitCode) {
    let help = match error.kind {
        RetrackFileError::NotLocal | RetrackFileError::OutsideWorkspace => {
            t!("retrack.err_not_local_help")
        }
        RetrackFileError::InsideData => t!("retrack.err_inside_data_help"),
        RetrackFileError::NotRecorded => t!("retrack.err_not_recorded_help"),
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: the version named was never recorded, so there is nowhere to go back to.
#[derive(Grouped)]
pub struct ErrorRetrackNothing {
    /// Where the entry sits in the Layout.
    path: String,
}

impl Failure for ErrorRetrackNothing {
    fn name(&self) -> &'static str {
        "error_retrack_nothing"
    }

    fn reason(&self) -> String {
        t!("retrack.err_nothing", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackNothing);

#[renderer(buffer)]
pub fn render_error_retrack_nothing(error: ErrorRetrackNothing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_nothing_help").trim()));
    ec.exit_code = EC_ERR_RETRACK;
}

/// Error: the file has changes the Layout does not name.
#[derive(Grouped)]
pub struct ErrorRetrackModified {
    /// Where the file sits in the Layout.
    path: String,
}

impl Failure for ErrorRetrackModified {
    fn name(&self) -> &'static str {
        "error_retrack_modified"
    }

    fn reason(&self) -> String {
        t!("retrack.err_modified", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackModified);

#[renderer(buffer)]
pub fn render_error_retrack_modified(error: ErrorRetrackModified, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_modified_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_DIRTY;
}

/// Error: a variant is waiting to be joined into the file.
#[derive(Grouped)]
pub struct ErrorRetrackMerging {
    /// Where the file sits in the Layout.
    path: String,
    /// The variant that is waiting, as hex.
    variant: String,
}

impl Failure for ErrorRetrackMerging {
    fn name(&self) -> &'static str {
        "error_retrack_merging"
    }

    fn reason(&self) -> String {
        t!(
            "retrack.err_merging",
            path = self.path,
            variant = self.variant
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorRetrackMerging);

#[renderer(buffer)]
pub fn render_error_retrack_merging(error: ErrorRetrackMerging, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_merging_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_DIRTY;
}

/// Error: the target names nothing the entry can be moved to.
#[derive(Grouped)]
pub struct ErrorRetrackTarget {
    /// What was given.
    target: String,
    /// What is wrong with it.
    kind: RetrackTargetError,
}

/// The ways a target names no version to move to.
pub enum RetrackTargetError {
    /// It is no target at all.
    NotATarget,
    /// It is a step count written with a dash, which the command line reads as a flag.
    DashNumber,
    /// It is a step or a number past an end of the chain the entry is the top of.
    OutOfChain {
        /// Where the entry sits in the Layout.
        path: String,
    },
    /// The hash names no version in the index.
    NotAVersion,
    /// The hash names more than one version in the index.
    AmbiguousHash {
        /// The versions it names, as hex.
        candidates: Vec<String>,
    },
    /// The hash names a version that is not on the way back from where the entry is.
    NotAncestor {
        /// Where the entry sits in the Layout.
        path: String,
    },
}

impl Failure for ErrorRetrackTarget {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackTargetError::NotATarget => "error_retrack_not_a_target",
            RetrackTargetError::DashNumber => "error_retrack_target_dash",
            RetrackTargetError::OutOfChain { .. } => "error_retrack_out_of_chain",
            RetrackTargetError::NotAVersion => "error_retrack_not_a_version",
            RetrackTargetError::AmbiguousHash { .. } => "error_retrack_ambiguous_hash",
            RetrackTargetError::NotAncestor { .. } => "error_retrack_not_ancestor",
        }
    }

    fn reason(&self) -> String {
        let said = match &self.kind {
            RetrackTargetError::NotATarget => {
                t!("retrack.err_not_a_target", target = self.target)
            }
            RetrackTargetError::DashNumber => {
                t!("retrack.err_target_dash", target = self.target)
            }
            RetrackTargetError::OutOfChain { path } => {
                t!(
                    "retrack.err_out_of_chain",
                    target = self.target,
                    path = path
                )
            }
            RetrackTargetError::NotAVersion => {
                t!("retrack.err_not_a_version", target = self.target)
            }
            RetrackTargetError::AmbiguousHash { candidates } => t!(
                "common.err_hash_ambiguous",
                hash = self.target,
                candidates = candidates.join(", ")
            ),
            RetrackTargetError::NotAncestor { path } => {
                t!(
                    "retrack.err_not_ancestor",
                    target = self.target,
                    path = path
                )
            }
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackTarget);

#[renderer(buffer)]
pub fn render_error_retrack_target(error: ErrorRetrackTarget, ec: &mut ResExitCode) {
    let (help, code) = match error.kind {
        RetrackTargetError::NotATarget => {
            (t!("retrack.err_not_a_target_help"), EC_ERR_RETRACK_ARGUMENT)
        }
        RetrackTargetError::DashNumber => {
            (t!("retrack.err_target_dash_help"), EC_ERR_RETRACK_ARGUMENT)
        }
        RetrackTargetError::OutOfChain { .. } => {
            (t!("retrack.err_out_of_chain_help"), EC_ERR_RETRACK)
        }
        RetrackTargetError::NotAVersion => (t!("retrack.err_not_a_version_help"), EC_ERR_RETRACK),
        RetrackTargetError::AmbiguousHash { .. } => (t!("common.err_hash_help"), EC_ERR_RETRACK),
        RetrackTargetError::NotAncestor { .. } => {
            (t!("retrack.err_not_ancestor_help"), EC_ERR_RETRACK)
        }
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
    ec.exit_code = code;
}

/// Error: a store, an index or a Layout would not do what retracking needs.
#[derive(Grouped)]
pub struct ErrorRetrackFailed {
    /// Why it would not.
    cause: String,
}

impl Failure for ErrorRetrackFailed {
    fn name(&self) -> &'static str {
        "error_retrack_failed"
    }

    fn reason(&self) -> String {
        t!("retrack.err_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackFailed);

#[renderer(buffer)]
pub fn render_error_retrack_failed(error: ErrorRetrackFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_failed_help").trim()));
    ec.exit_code = EC_ERR_RETRACK;
}

/// What a retrack of a tree was asked to do.
// The flags are the independent answers a run gave, and folding them into an enum would only have to
// be unfolded at every use.
#[allow(clippy::struct_excessive_bools)]
struct RetrackTree<'a> {
    /// What the run named the pointer go to.
    recording: &'a Recording,
    /// The target as it was written, for the words a refusal uses.
    written: &'a str,
    /// The Layout being worked in.
    layout: &'a Layout,
    /// The Workspace the run is in.
    held: &'a Workspace,
    /// The Workspace's root, where the work is.
    root: &'a Path,
    /// The tree's path in the Layout.
    path: &'a LayoutPath,
    /// The entry the tree records now.
    id: Uuid,
    /// What the entry records now.
    data: &'a MutableData,
    /// The index the versions are read from.
    vcs: &'a VCSIndex,
    /// The runtime the reads run on.
    runtime: &'a tokio::runtime::Runtime,
    /// The store the tree's content is read from.
    store: &'a RorolalaStorage,
    /// The Vault the run may fetch an entry or a version from.
    remote: &'a mut LazyRes<ResCurrentRemoteVault>,
    /// The account a fetch would act as.
    current: &'a mut LazyRes<ResCurrentAccount>,
    /// Whether a fetch is refused.
    offline: bool,
    /// Whether only the record moves and the work is left alone.
    only_index: bool,
    /// Whether work that was never recorded is written over.
    force: bool,
    /// Whether a version off the way back is refused.
    deny_jump: bool,
}

/// Moves a tree's record to another version, and puts the work back from it.
///
/// What is put back is the target version's structure: a path it holds is written as it holds it,
/// and a path the tree recorded that the target does not hold is taken out of the work. What the
/// tree never recorded is left where it is, so a retrack does not reach beyond the version unit.
fn retrack_tree(asked: &mut RetrackTree<'_>) -> Next {
    match asked.recording {
        Recording::Fork => return retrack_tree_fork(asked),
        Recording::Entry(entry) => return retrack_tree_entry(asked, entry),
        Recording::Version(_) => {}
    }

    let Recording::Version(level) = asked.recording else {
        // UNWRAP: the match above returns for every other recording.
        unreachable!("a tree retrack records a version, an entry or a fork")
    };

    let current = match read_version(asked.vcs, asked.runtime, asked.data.version()) {
        Ok(current) => current,
        Err(next) => return next,
    };
    let chain = match ancestors(asked.vcs, asked.runtime, &current) {
        Ok(chain) => chain,
        Err(next) => return next,
    };
    let (target_hash, _number) = match resolve_target(
        asked.vcs,
        asked.runtime,
        &chain,
        &Asked {
            text: asked.written,
            level,
            path: asked.path.as_str(),
            deny_jump: asked.deny_jump,
        },
    ) {
        Ok(resolved) => resolved,
        Err(next) => return next,
    };

    let tree_root = asked.root.join(asked.path.to_path_buf());
    let tree = match Tree::open(asked.held.get_root(), &tree_root) {
        Ok(tree) => tree,
        Err(error) => return failed(error.to_string()),
    };

    let target = match asked
        .runtime
        .block_on(read_structure(asked.vcs, asked.store, &target_hash))
    {
        Ok(target) => target,
        Err(error) => return failed(error.to_string()),
    };

    let recorded =
        match asked
            .runtime
            .block_on(tree.recorded_structure(asked.layout, asked.vcs, asked.store))
        {
            Ok(recorded) => recorded,
            Err(error) => return failed(error.to_string()),
        };

    let staging = Staging::new(&tree_root);
    let staged = match staging.structure() {
        Ok(Some(staged)) => staged,
        Ok(None) => recorded.clone(),
        Err(error) => return failed(error.to_string()),
    };
    let conflicts = match staging.conflicts() {
        Ok(conflicts) => conflicts,
        Err(error) => return failed(error.to_string()),
    };

    let rules = match tree.rules(crate::ignore::rules(asked.held, false)) {
        Ok(rules) => rules,
        Err(error) => return failed(error.to_string()),
    };
    let scanned = match tree.scan(&rules, false) {
        Ok(scanned) => scanned,
        Err(error) => return failed(error.to_string()),
    };
    let status = librorolala::tree::compare(&recorded, &staged, &scanned.structure, &conflicts);

    let dirty =
        !status.staged.is_empty() || !status.unstaged.is_empty() || !status.conflicts.is_empty();

    if dirty && !asked.only_index && !asked.force {
        return ErrorRetrackTreeDirty {
            path: asked.path.as_str().to_owned(),
        }
        .into();
    }

    if !asked.only_index {
        if let Err(error) = bring_tree_content(asked, &target, &recorded, !asked.force) {
            return error;
        }
        if let Err(error) = put_back(
            &tree_root,
            &target,
            &recorded,
            asked.store,
            asked.runtime,
            !asked.force,
        ) {
            return error;
        }

        let conflicts = match staging.conflicts() {
            Ok(conflicts) => conflicts,
            Err(error) => return failed(error.to_string()),
        };
        for path in &conflicts {
            let _ = fs::remove_file(tree_root.join(librorolala::tree::conflict_copy(path)));
        }
        if let Err(error) = staging.clear() {
            return failed(error.to_string());
        }
    }

    let data = MutableData::new(
        asked.data.owner().map(str::to_owned),
        target_hash,
        asked.data.description().to_owned(),
    )
    .with_kind(LayoutEntryKind::Tree)
    .with_preference(asked.data.preference().map(str::to_owned));

    let mut batch = asked.layout.batch();
    if let Err(error) = batch.update_entry(asked.id, data) {
        return layout_failed(&error);
    }
    if let Err(error) = batch.commit() {
        return layout_failed(&error);
    }

    let scanned = match tree.scan(&rules, false) {
        Ok(scanned) => scanned,
        Err(error) => return failed(error.to_string()),
    };
    let baseline = tree.baseline();
    if let Err(error) = baseline.write(&target, &target_hash) {
        return failed(error.to_string());
    }
    if let Err(error) = baseline.write_entries(&scanned.cache) {
        return failed(error.to_string());
    }
    // A tree recorded before the mark held its id gets one the next time it is recorded.
    if let Err(error) = tree.write_local_id(asked.id) {
        return failed(error.to_string());
    }

    ResultRetrackTree {
        path: asked.path.as_str().to_owned(),
        version: crate::vcs_index::hex(&target_hash),
    }
    .into()
}

/// Writes `target` over the work at `root`, and takes out what `old` recorded and `target` does not.
fn put_back(
    root: &Path,
    target: &TreeStructure,
    old: &TreeStructure,
    store: &RorolalaStorage,
    runtime: &tokio::runtime::Runtime,
    skip_unchanged: bool,
) -> Result<(), Next> {
    if let Err(error) = fs::create_dir_all(root) {
        return Err(failed(error.to_string()));
    }

    for entry in target.entries() {
        // What the old structure already held is what the work already holds: a move of the record
        // that leaves a path as it was does not read it, write it, or make its directory again.
        if skip_unchanged && old.entry(entry.path()) == Some(entry) {
            continue;
        }

        let disk = root.join(entry.path());

        match entry.kind() {
            TreeEntryKind::Directory => {
                if let Err(error) = fs::create_dir_all(&disk) {
                    return Err(failed(error.to_string()));
                }
            }
            TreeEntryKind::File => {
                let Some(storage) = entry.storage() else {
                    continue;
                };
                if let Some(parent) = disk.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(failed(error.to_string()));
                }
                if let Err(error) = runtime.block_on(store.extract_file(storage, &disk)) {
                    return Err(failed(error.to_string()));
                }
            }
            TreeEntryKind::Symlink => {
                let Some(target_path) = entry.target() else {
                    continue;
                };
                if disk.exists() {
                    let _ = fs::remove_file(&disk);
                }
                if let Some(parent) = disk.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    return Err(failed(error.to_string()));
                }
                #[cfg(unix)]
                {
                    if let Err(error) = std::os::unix::fs::symlink(target_path, &disk) {
                        return Err(failed(error.to_string()));
                    }
                }
                #[cfg(windows)]
                {
                    if let Err(error) = std::os::windows::fs::symlink_file(target_path, &disk) {
                        return Err(failed(error.to_string()));
                    }
                }
            }
        }
    }

    for entry in old.entries() {
        if target.entry(entry.path()).is_some() {
            continue;
        }

        let disk = root.join(entry.path());
        match entry.kind() {
            TreeEntryKind::Directory => {
                let _ = fs::remove_dir(&disk);
            }
            TreeEntryKind::File | TreeEntryKind::Symlink => {
                let _ = fs::remove_file(&disk);
            }
        }
    }

    Ok(())
}

/// Gives a tree's Layout entry a fresh `Uuid`, with what it records now.
///
/// The tree's own mark keeps the id the Layout names it by, so it is written with the new one: a
/// directory whose mark still named the old entry would be read as a loss and a gain.
fn retrack_tree_fork(asked: &RetrackTree<'_>) -> Next {
    let tree_root = asked.root.join(asked.path.to_path_buf());
    let tree = match Tree::open(asked.held.get_root(), &tree_root) {
        Ok(tree) => tree,
        Err(error) => return failed(error.to_string()),
    };
    let target = Uuid::new_v4();
    let data = asked.data.clone();

    if let Err(error) = switch_entry(asked.layout, asked.path, target, data) {
        return layout_failed(&error);
    }
    if let Err(error) = tree.write_local_id(target) {
        return failed(error.to_string());
    }

    ResultRetrackTree {
        path: asked.path.as_str().to_owned(),
        version: hex(asked.data.version()),
    }
    .into()
}

/// Makes a tree record the entry a run names, in place of the one it records now.
fn retrack_tree_entry(asked: &mut RetrackTree<'_>, entry: &EntryTarget) -> Next {
    let tree_root = asked.root.join(asked.path.to_path_buf());
    let tree = match Tree::open(asked.held.get_root(), &tree_root) {
        Ok(tree) => tree,
        Err(error) => return failed(error.to_string()),
    };
    let target = match recorded_entry(
        entry,
        asked.written,
        asked.layout,
        asked.held,
        asked.id,
        asked.remote,
        asked.current,
        asked.offline,
    ) {
        Ok(target) => target,
        Err(next) => return next,
    };

    if target.data.kind() != LayoutEntryKind::Tree {
        return failed(format!(
            "`{}` names a file, and `{}` is a tree",
            asked.written,
            asked.path.as_str()
        ));
    }

    let current = asked.data.version();
    let there = target.data.version();
    let relation = if current == there {
        Relation::Equal
    } else {
        let here = match read_version(asked.vcs, asked.runtime, current) {
            Ok(version) => version,
            Err(next) => return next,
        };
        let wanted = match read_version(asked.vcs, asked.runtime, there) {
            Ok(version) => version,
            Err(next) => return next,
        };
        let here_chain = digests(&match ancestors(asked.vcs, asked.runtime, &here) {
            Ok(chain) => chain,
            Err(next) => return next,
        });
        let there_chain = digests(&match ancestors(asked.vcs, asked.runtime, &wanted) {
            Ok(chain) => chain,
            Err(next) => return next,
        });

        relation(current, there, &here_chain, &there_chain)
    };
    let action = match switch_action(relation, asked.only_index, asked.deny_jump) {
        Ok(action) => action,
        Err(SwitchRefusal::NotOnChain) => {
            return ErrorRetrackSwitch {
                path: asked.path.as_str().to_owned(),
            }
            .into();
        }
    };

    if action == SwitchAction::Content && !asked.force && tree_has_work(asked, &tree) {
        return ErrorRetrackTreeDirty {
            path: asked.path.as_str().to_owned(),
        }
        .into();
    }

    let version = match action {
        SwitchAction::Keep => current,
        SwitchAction::Pointer | SwitchAction::Content => there,
    };
    let data = switched_data(&target.data, asked.data, version).with_kind(LayoutEntryKind::Tree);

    let wanted = if action == SwitchAction::Keep {
        None
    } else {
        match asked
            .runtime
            .block_on(read_structure(asked.vcs, asked.store, &there))
        {
            Ok(structure) => Some(structure),
            Err(error) => return failed(error.to_string()),
        }
    };
    let recorded = if action == SwitchAction::Content {
        match asked
            .runtime
            .block_on(tree.recorded_structure(asked.layout, asked.vcs, asked.store))
        {
            Ok(structure) => Some(structure),
            Err(error) => return failed(error.to_string()),
        }
    } else {
        None
    };

    if action == SwitchAction::Content {
        let Some(wanted) = wanted.as_ref() else {
            return failed("the target structure was not read");
        };
        let Some(recorded) = recorded.as_ref() else {
            return failed("the recorded structure was not read");
        };

        if let Err(error) = bring_tree_content(asked, wanted, recorded, !asked.force) {
            return error;
        }
    }

    if let Err(error) = switch_entry(asked.layout, asked.path, target.id, data) {
        return layout_failed(&error);
    }

    if action == SwitchAction::Content {
        // UNWRAP: both structures are read above exactly when the content is to be written.
        let (Some(wanted), Some(recorded)) = (wanted.as_ref(), recorded.as_ref()) else {
            return failed("the target structure was not read");
        };

        if let Err(next) = put_back(
            &tree_root,
            wanted,
            recorded,
            asked.store,
            asked.runtime,
            !asked.force,
        ) {
            return next;
        }
        let staging = Staging::new(&tree_root);
        let conflicts = match staging.conflicts() {
            Ok(conflicts) => conflicts,
            Err(error) => return failed(error.to_string()),
        };
        for path in &conflicts {
            let _ = fs::remove_file(tree_root.join(librorolala::tree::conflict_copy(path)));
        }
        if let Err(error) = staging.clear() {
            return failed(error.to_string());
        }
    } else if action == SwitchAction::Pointer {
        // UNWRAP: the target structure is read above for every switch that is not a keep.
        let Some(wanted) = wanted.as_ref() else {
            return failed("the target structure was not read");
        };

        if let Err(error) = tree.baseline().write(wanted, &there) {
            return failed(error.to_string());
        }
    }

    if action == SwitchAction::Content
        && let Some(wanted) = wanted.as_ref()
        && let Err(error) = tree.baseline().write(wanted, &there)
    {
        return failed(error.to_string());
    }
    if let Err(error) = tree.write_local_id(target.id) {
        return failed(error.to_string());
    }

    ResultRetrackTree {
        path: asked.path.as_str().to_owned(),
        version: hex(version),
    }
    .into()
}

/// Brings the content a tree structure names from a Vault when the store here is short of it.
///
/// A tree's work is many files, and the version moved to may be one this side has never carried. What
/// is fetched is every file's content key, one request rather than one per file, before anything is
/// written over the work.
// The account failure is the command's own kind, and this is where it becomes the run's answer; `?` has no
// conversion from it.
#[allow(clippy::question_mark)]
fn bring_tree_content(
    asked: &mut RetrackTree<'_>,
    structure: &TreeStructure,
    old: &TreeStructure,
    changed_only: bool,
) -> Result<(), Next> {
    let keys: Vec<Key> = structure
        .entries()
        .iter()
        .filter(|entry| !changed_only || old.entry(entry.path()) != Some(entry))
        .filter_map(|entry| entry.storage().copied())
        .collect();

    if keys.is_empty() {
        return Ok(());
    }

    let Some(bound) = asked.current.get_ref().must_bind().ok() else {
        return Ok(());
    };
    let account = match account_named(&bound, Some(asked.held), None) {
        Ok(account) => account,
        Err(error) => return Err(error.into()),
    };
    let tracked = tracked_vault(asked.held);
    let primary = fetch::primary(asked.remote.get_ref(), tracked.as_deref());
    let sources = match Sources::new(
        asked.held,
        &account,
        asked.remote.get_ref(),
        primary,
        asked.offline,
    ) {
        Ok(sources) => sources,
        Err(cause) => return Err(failed(cause)),
    };

    sources.bring(asked.store, &keys);
    sources.report();

    Ok(())
}

/// Whether the tree holds work a move of its record would write over.
fn tree_has_work(asked: &RetrackTree<'_>, tree: &Tree) -> bool {
    asked
        .runtime
        .block_on(tree.status(
            asked.layout,
            asked.vcs,
            asked.store,
            crate::ignore::rules(asked.held, false),
            false,
        ))
        .map_or(true, |status| {
            !status.staged.is_empty() || !status.unstaged.is_empty() || !status.conflicts.is_empty()
        })
}

/// Result: a tree's record moved to another version.
#[derive(Grouped)]
pub struct ResultRetrackTree {
    /// The tree as a person knows it.
    path: String,
    /// The version it records now.
    version: String,
}

#[renderer(buffer)]
pub fn render_result_retrack_tree(result: ResultRetrackTree, ec: &mut ResExitCode) {
    r_println!(
        "{}",
        t!(
            "retrack.result_tree",
            path = result.path,
            version = result.version
        )
        .trim()
    );

    let _ = ec;
}

/// Error: a tree holds work that was never recorded.
#[derive(Grouped)]
pub struct ErrorRetrackTreeDirty {
    /// The tree.
    path: String,
}

impl Failure for ErrorRetrackTreeDirty {
    fn name(&self) -> &'static str {
        "error_retrack_tree_dirty"
    }

    fn reason(&self) -> String {
        t!("retrack.err_tree_dirty", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackTreeDirty);

#[renderer(buffer)]
pub fn render_error_retrack_tree_dirty(error: ErrorRetrackTreeDirty, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_tree_dirty_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_DIRTY;
}

#[cfg(test)]
mod tests {
    use super::{Recording, RetrackTargetError, VersionTarget, recording_of};
    use librorolala::storage::Key;

    /// Saying nothing is one version back, and a step count is how that is said as a target.
    #[test]
    fn one_step_back_is_the_default_and_the_word_for_it() {
        assert!(matches!(
            recording_of(None, Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(1)))
        ));
        assert!(matches!(
            recording_of(Some("~1"), Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(1)))
        ));
    }

    /// A step count is `~N`, a version number is digits, a hash is a hash, and anything left that a
    /// Layout may hold is a path of the Vault's own Layout.
    #[test]
    fn each_shape_is_read_as_the_target_it_is() {
        assert!(matches!(
            recording_of(Some("~3"), Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(3)))
        ));
        assert!(matches!(
            recording_of(Some("~0"), Vec::new),
            Ok(Recording::Version(VersionTarget::Steps(0)))
        ));
        assert!(matches!(
            recording_of(Some("0"), Vec::new),
            Ok(Recording::Version(VersionTarget::Number(0)))
        ));
        assert!(matches!(
            recording_of(Some("12"), Vec::new),
            Ok(Recording::Version(VersionTarget::Number(12)))
        ));
        assert!(matches!(
            recording_of(Some("art/hero.psd"), Vec::new),
            Ok(Recording::Entry(super::EntryTarget::Upstream(path)))
                if path.as_str() == "art/hero.psd"
        ));

        let hex = "0f".repeat(32);
        let Ok(Recording::Version(VersionTarget::Hash(key))) = recording_of(Some(&hex), Vec::new)
        else {
            panic!("a hash is read as a hash");
        };
        assert_eq!(key, Key::new([0x0f; 32]));
    }

    /// A `Uuid` names an entry, and is read as one before anything else is tried: it is also a path
    /// a Layout may hold, so reading it as one would send the run looking upstream for an entry this
    /// Layout may hold at no path.
    #[test]
    fn a_uuid_is_read_as_an_entry_before_it_is_read_as_a_path() {
        use super::EntryTarget;
        use uuid::Uuid;

        let id = Uuid::from_u128(7);

        assert!(matches!(
            recording_of(Some(&id.to_string()), Vec::new),
            Ok(Recording::Entry(EntryTarget::Uuid(read))) if read == id
        ));
    }

    /// The head of a hash is read against the versions there are: one match is that version, none
    /// is a version that is not here, and more than one is a question that needs more digits.
    #[test]
    fn a_head_is_read_against_the_versions_there_are() {
        let one = Key::new([0x0f; 32]);
        let mut digest = [0x0f; 32];
        digest[31] = 0x1f;
        let two = Key::new(digest);

        assert!(matches!(
            recording_of(Some("0f0f"), || vec![one]),
            Ok(Recording::Version(VersionTarget::Hash(key))) if key == one
        ));
        assert!(matches!(
            recording_of(Some("0f0f"), Vec::new),
            Err(super::ErrorRetrackTarget {
                kind: RetrackTargetError::NotAVersion,
                ..
            })
        ));
        assert!(matches!(
            recording_of(Some("0f0f"), || vec![one, two]),
            Err(super::ErrorRetrackTarget {
                kind: RetrackTargetError::AmbiguousHash { .. },
                ..
            })
        ));
    }

    /// What is no target is refused, with a step count written with a dash refused for the spelling
    /// it is: the command line reads `-N` as a flag, so `~N` is the word for one.
    #[test]
    fn a_target_that_is_not_one_is_refused() {
        assert!(recording_of(Some("~"), Vec::new).is_err());
        assert!(recording_of(Some("~x"), Vec::new).is_err());
        assert!(recording_of(Some("+1"), Vec::new).is_err());
        assert!(recording_of(Some(""), Vec::new).is_err());
        assert!(matches!(
            recording_of(Some("-1"), Vec::new),
            Err(super::ErrorRetrackTarget {
                kind: RetrackTargetError::DashNumber,
                ..
            })
        ));
    }

    /// Where two versions stand is read from the chains each is the top of: the same version, one
    /// behind the other on its own chain, or two chains that meet nowhere.
    #[test]
    fn the_relation_of_two_versions_is_read_from_their_chains() {
        use super::{Relation, relation};

        let old = [1; 32];
        let cur = [3; 32];
        let other = [9; 32];
        let cur_chain = [[2; 32], old];
        let target_chain = [[8; 32], old];

        assert_eq!(relation(cur, cur, &[], &[]), Relation::Equal);
        assert_eq!(
            relation(cur, old, &cur_chain, &target_chain),
            Relation::Ahead
        );
        assert_eq!(relation(old, cur, &[], &target_chain), Relation::Behind);
        assert_eq!(
            relation(cur, other, &cur_chain, &[[7; 32]]),
            Relation::Apart
        );
    }

    /// What a change of `Uuid` does to the file is decided by how the two versions stand and by the
    /// flags the run gave: the same version or a later one keeps the file, an earlier one has its
    /// content written over it unless the record alone is asked for, and another chain is a jump
    /// unless the run refused one.
    #[test]
    fn what_a_change_of_uuid_does_to_the_file_follows_the_versions_and_the_flags() {
        use super::{Relation, SwitchAction, SwitchRefusal, switch_action};

        assert_eq!(
            switch_action(Relation::Equal, false, false),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Equal, true, true),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Ahead, false, false),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Ahead, true, true),
            Ok(SwitchAction::Keep)
        );

        // A version behind the one the entry records has its content written over the file: what
        // decides whether the file may be written is whether it stands as recorded, not the flag.
        assert_eq!(
            switch_action(Relation::Behind, false, false),
            Ok(SwitchAction::Content)
        );
        assert_eq!(
            switch_action(Relation::Behind, true, false),
            Ok(SwitchAction::Pointer)
        );
        assert_eq!(
            switch_action(Relation::Behind, true, true),
            Ok(SwitchAction::Pointer)
        );

        // Another chain is a jump, and a jump is what a run makes unless it asked not to.
        assert_eq!(
            switch_action(Relation::Apart, false, false),
            Ok(SwitchAction::Content)
        );
        assert_eq!(
            switch_action(Relation::Apart, true, false),
            Ok(SwitchAction::Pointer)
        );
        // A run that refused the jump is refused however else it was asked to go.
        assert_eq!(
            switch_action(Relation::Apart, false, true),
            Err(SwitchRefusal::NotOnChain)
        );
        assert_eq!(
            switch_action(Relation::Apart, true, true),
            Err(SwitchRefusal::NotOnChain)
        );
    }

    /// The entry a path comes to record holds what the entry it records holds, and is cut the way
    /// the path's own entry asked for: what is written is about the file, not about its `Uuid`.
    #[test]
    fn the_entry_recorded_holds_what_it_held_and_the_preference_of_the_old_one() {
        use super::switched_data;
        use librorolala::layout::MutableData;

        let source = MutableData::new(Some("bob".to_owned()), [2; 32], "upstream".to_owned())
            .with_preference(Some("upstream-policy".to_owned()));
        let old = MutableData::new(Some("alice".to_owned()), [1; 32], "mine".to_owned())
            .with_preference(Some("maya-binary".to_owned()));

        let data = switched_data(&source, &old, [5; 32]);

        assert_eq!(data.owner(), Some("bob"));
        assert_eq!(data.description(), "upstream");
        assert_eq!(data.version(), [5; 32]);
        assert_eq!(data.preference(), Some("maya-binary"));
    }
}

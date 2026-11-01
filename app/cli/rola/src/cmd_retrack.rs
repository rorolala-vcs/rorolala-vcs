//! The `rola retrack` command: move the version a file's Layout entry is at.
//!
//! A Layout names one version for each path, and the content behind a version stays in the index
//! whether or not anything points at it. So putting a file's record back where it was is one
//! pointer moving: the `Uuid`, the path and the description are left as they are, and nothing is
//! removed from the index.
//!
//! Moving the pointer and leaving the file alone would leave the two apart — the file holds what a
//! newer version did, and the Layout now names an older one — so the version's content is written
//! back over the file as well, and the reading is told the two agree again, the way
//! [`crate::checkout`] tells it for a pull. `--only-index` is what a run gives when it wants the
//! record moved and the work left alone; what the tree then holds is what `rola status` reports as
//! a change that was never recorded.
//!
//! A file with changes that were never recorded, and one a variant is waiting to be joined into,
//! are not states a retrack may write over: both are refused. `--force` is how a run says to go
//! ahead anyway — for the first, the changes are discarded and the content switched; for the
//! second, the merge is ended, its variant file removed, and the content switched.

// A retrack reads a level, a Layout, a chain and a version before it moves one pointer, and every
// step of that is a refusal of its own to say; keeping them in one function is what lets the order
// they are read in be read off the function rather than assembled from helpers.
#![allow(clippy::too_many_lines)]
// The handler's parameters are the resources the framework injects, so how many of them there are
// is what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments)]

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr as _;

use librorolala::auth::Account;
use librorolala::daemon::{Chain, action_bring_remote_index_async, action_fetch_layout};
use librorolala::ignore::IgnoreRuleSet;
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::status::{tracked_vault, variant_moves};
use librorolala::storage::{Blake3Hash, Key, StorageBackend as _};
use librorolala::tree_analyze::{Cache, cache_path, entry_of, tree_diff};
use librorolala::vcs::{VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version};
use librorolala::workspace::{Merging, Pending, Workspace};
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
    ResConcurrency, ResCurrentRemoteVault, ResForce, ResOffline, ResRorolalaStorage, ResVCSIndex,
    ResVault, ResWorkspace,
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
    IndexObject, change_upstream_words, filling_flag, index_hashes, index_keys, offer, positional,
    strip_written, typing_flag, vault_names,
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

/// How alike two text files have to be to count as the same file moved.
///
/// It is the same default the reading uses, since what this acts on is what the reading found.
const DEFAULT_ALIKE: f32 = 0.6;

/// Where the pointer goes, one version back unless said.
const ARG_UNTIL: PickerArg<'static, Option<String>> = arg![until: Option<String>];

/// Move the pointer and leave the file on disk alone.
const ARG_ONLY_INDEX: PickerArg<'static, Flag> = arg![only_index: Flag];

/// Move the pointer to a version that is not on the way back to where it is now.
const ARG_ALLOW_JUMP: PickerArg<'static, Flag> = arg![allow_jump: Flag];

/// Record another `Uuid` for the file, named by a `Uuid` or a path of the Vault's own Layout.
const ARG_CHANGE_UPSTREAM: PickerArg<'static, Option<String>> =
    arg![change_upstream: Option<String>];

/// Record a fresh `Uuid` for the file, with what it records now.
const ARG_FORK: PickerArg<'static, Flag> = arg![fork: Flag];

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
/// `FILE` is a local path the Layout records, so the filesystem answers it. `--until` names where
/// the pointer goes: a level, a version the index holds, or a Vault whose own Layout records one.
/// A level such as `-1` begins with a dash, so it is written with the flag — `--until=-1` — rather
/// than as a word of its own, which would be read as an option; what is offered here are the words
/// that do stand on their own, the versions and the Vaults. `--change-upstream` names the entry to
/// record instead: one this Layout holds at no path, or a path or entry of the fetched copy of the
/// Vault's Layout it tracks.
#[completion(EntryRetrack)]
pub fn complete_retrack(
    ctx: ShellContext,
    workspace: &mut LazyRes<ResWorkspace>,
    index: &mut LazyRes<ResVCSIndex>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if filling_flag(&ctx, &ARG_UNTIL) {
        let mut names: Vec<String> = index_hashes(index.get_ref().as_ref(), IndexObject::Version);
        names.extend(vault_names(remote.get_ref()));

        return offer(&ctx, names);
    }

    if filling_flag(&ctx, &ARG_CHANGE_UPSTREAM) {
        return offer(&ctx, change_upstream_words(workspace.get_ref()));
    }

    if typing_flag(&ctx) {
        return strip_written(
            &ctx,
            suggest! {
                ARG_UNTIL: t!("retrack.complete.until"),
                ARG_ONLY_INDEX: t!("retrack.complete.only_index"),
                ARG_ALLOW_JUMP: t!("retrack.complete.allow_jump"),
                ARG_CHANGE_UPSTREAM: t!("retrack.complete.change_upstream"),
                ARG_FORK: t!("retrack.complete.fork"),
            },
        );
    }

    if positional(&ctx, "retrack") == 0 {
        Suggest::file_comp()
    } else {
        suggest!()
    }
}

/// Moves a file's recorded version back, in the Layout being worked in
///
/// The entry's version pointer moves: its `Uuid`, its path and its description stay as they are,
/// and nothing is removed from the index. `FILE` is one local path in this Workspace — a `Uuid`, a
/// hash and a remote name are refused — and it has to be a path the Layout records.
///
/// `--until` is where the pointer goes, and is one version back when it is not given: `-1` one
/// version back, `~N` N versions back, `N` the version numbered N, a hash a version on the way
/// back from where the entry is now, and a Vault's name the version that Vault's own Layout
/// records for the file. A level that begins with a dash is written with the flag, `--until=-1`,
/// since a word of its own would be read as an option. A hash that is not on the way back is
/// refused unless `--allow-jump` is given; `~N` and `N` past either end of the chain are always
/// refused.
///
/// A version this index does not hold is brought from the Vault the Layout tracks before the
/// pointer moves, so a run may go to a version this side never fetched and leave a chain that can
/// be read behind. `--offline` refuses that instead of reaching for it.
///
/// The version's content is written back over the file with the pointer, so the two agree again.
/// `--only-index` moves the pointer and leaves the file alone, which is what a run wants when the
/// work on disk is not to be touched: `rola status` then reports the file as changed.
///
/// A file with changes that were never recorded is refused, and so is one a variant is waiting to
/// be joined into. `--force` goes ahead anyway: the changes are discarded and the content switched,
/// and a merge is ended, its variant file removed, and the content switched. `--force` and
/// `--only-index` say opposite things and cannot be given together.
///
/// # Errors
///
/// Renders [`ErrorRetrackNoFile`] when no file or several are named, [`ErrorRetrackFlags`] when
/// `--force` and `--only-index` are given together, [`ErrorRetrackFile`] when what was named is not
/// a local path the Layout records, [`ErrorRetrackLevel`] when `--until` names no version the entry
/// can reach, [`ErrorRetrackNothing`] when the entry was never recorded, [`ErrorRetrackModified`]
/// when the file has changes the Layout does not name, [`ErrorRetrackMerging`] when a variant is
/// waiting for the file, [`ErrorRetrackUpstream`] when the Vault's Layout records no version for
/// it, [`ErrorOffline`] when the version is not here and the run is offline,
/// [`ErrorLayoutShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorRetrackFailed`] when the store, the index or the Layout refuses.
#[command(node = "retrack", entry = EntryRetrack)]
pub fn retrack(args: EntryRetrack, force: &ResForce) -> Next {
    let picked = args
        .pick(ARG_UNTIL)
        .pick(ARG_ONLY_INDEX)
        .pick(ARG_ALLOW_JUMP)
        .pick(ARG_CHANGE_UPSTREAM)
        .pick(ARG_FORK)
        .pick_or_route(arg![Vec<String>], || ErrorRetrackNoFile.to_chain())
        .pick(&UNMATCHED)
        .to_result();
    let (until, only_index, allow_jump, change_upstream, fork, mut files, unmatched) = match picked
    {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "retrack") {
        return next;
    }

    // One file is the whole of what this moves, so naming more is not a run this reads as one of
    // its own: it is refused rather than acted on for the first and quietly ignoring the rest.
    if files.len() > 1 {
        return ErrorRetrackManyFiles { count: files.len() }.into();
    }

    let Some(file) = files.pop() else {
        return ErrorRetrackNoFile.into();
    };

    let only_index = matches!(only_index, Flag::Active);

    // `--force` and `--only-index` say opposite things about the file on disk, so a run that gives
    // both is refused before anything is read rather than one of them winning silently.
    if **force && only_index {
        return ErrorRetrackFlags.into();
    }

    // `--change-upstream` and `--fork` are two ways of changing the `Uuid` a path records, and a
    // version level is a way of moving within one entry, so at most one of the three is a run.
    let mode = match (change_upstream, matches!(fork, Flag::Active)) {
        (Some(_), true) => return ErrorRetrackMode.into(),
        (Some(reference), false) => Some(UuidMode::Change { reference }),
        (None, true) => Some(UuidMode::Fork),
        (None, false) => None,
    };
    if mode.is_some() && until.is_some() {
        return ErrorRetrackMode.into();
    }

    StateRetrack {
        file,
        until,
        mode,
        only_index,
        allow_jump: matches!(allow_jump, Flag::Active),
        force: **force,
    }
    .into()
}

/// The state a retracking starts in.
#[derive(Grouped)]
pub struct StateRetrack {
    /// The one file named, as it was written.
    file: String,
    /// Where the pointer goes, as it was written; one version back when it was not said.
    until: Option<String>,
    /// Whether the run changes the `Uuid` the path records, and how; none when it moves a version.
    mode: Option<UuidMode>,
    /// Whether the pointer moves and the file on disk is left alone.
    only_index: bool,
    /// Whether a version off the way back may be jumped to.
    allow_jump: bool,
    /// Whether changes that were never recorded, and a merge in progress, are gone past anyway.
    force: bool,
}

/// What a run that changes the `Uuid` a path records was asked for.
enum UuidMode {
    /// Record the entry `reference` names: a `Uuid`, or a path of the Vault's own Layout.
    Change {
        /// What was written after `--change-upstream`.
        reference: String,
    },
    /// Record a fresh `Uuid`, holding what the path records now.
    Fork,
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
) -> Next {
    let StateRetrack {
        file,
        until,
        mode,
        only_index,
        allow_jump,
        force,
    } = state;

    // The level is read before anything is looked at, so a run that misspelled it is told that
    // rather than a complaint about a file it would have refused for another reason. A run that
    // changes the `Uuid` a path records has no level to read.
    let level = match &mode {
        None => Some(until_of(until.as_deref(), || {
            index_keys(index.get_ref().as_ref(), IndexObject::Version)
        })?),
        Some(_) => None,
    };
    let text = until.unwrap_or_else(|| "-1".to_owned());

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

    // A run that changes the `Uuid` a path records takes the entry it is to record instead of a
    // level, and is done before anything a level is read for.
    if let Some(mode) = mode {
        return switch_uuid(Switching {
            mode: &mode,
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
            only_index,
            allow_jump,
            force,
        });
    }

    // UNWRAP: a run that names no `Uuid` mode is a run that names a level, and one was read above.
    let level = level.expect("a version-mode run reads its level");

    let here = read_version(vcs, &runtime, data.version())?;
    let chain = ancestors(vcs, &runtime, &here)?;

    // The one level that leaves the local index for what the Vault's own Layout says. It is read
    // before the rest only for this level, since reading it may mean reaching for a Vault.
    let upstream = match &level {
        Until::Vault(alias) => Some(upstream_version(
            held, remote, current, alias, &path, id, **offline,
        )?),
        _ => None,
    };

    // The version the pointer goes to may be one this index never fetched. It is checked here
    // before the level is resolved, so that a hash the Vault holds is a version to go to rather
    // than one this side cannot name, and so that the chain left behind is one that can be read.
    let wanted = match &level {
        Until::Hash(key) => Some((None, *key.digest())),
        Until::Vault(alias) => {
            // UNWRAP: the Vault's version is read for exactly this level just above.
            let hash = upstream.expect("the Vault's version is read for the Vault level");

            Some((Some(alias.as_str()), hash))
        }
        _ => None,
    };

    if let Some((vault, hash)) = wanted {
        match presence(vcs, &runtime, hash) {
            Presence::Version => {}
            Presence::Elsewhere => {
                return ErrorRetrackLevel {
                    until: text,
                    kind: RetrackLevelError::NotAVersion,
                }
                .into();
            }
            Presence::Missing => {
                // Nothing the run asked for is here, and an offline run may not go looking for it.
                if **offline {
                    return ErrorOffline.into();
                }

                let bringer = Bringer {
                    workspace: held,
                    runtime: &runtime,
                    end: vault_end(held, remote, current, vault)?,
                    text: &text,
                };

                bringer.bring(hash)?;
            }
        }
    }

    let asked = Asked {
        text: &text,
        level: &level,
        path: path.as_str(),
        allow_jump,
    };
    let (target, number) = resolve_target(vcs, &runtime, &chain, &asked, upstream)?;

    let current_hash = data.version();
    let moved = target != current_hash;

    // What the run is to do to the file is worked out before anything is written, so a run that is
    // refused leaves the Layout, the tree and the merge exactly as they were.
    let mut merging = Merging::read(&root);
    let pending = merging.at(id).cloned();
    // Retracking moves a version pointer of an entry the Layout names, which the ignore rules
    // have no jurisdiction over, so the reading is taken unfiltered.
    let diff = match tree_diff(&layout, held, DEFAULT_ALIKE, &IgnoreRuleSet::empty()) {
        Ok(diff) => diff,
        Err(error) => return failed(error.to_string()),
    };

    let action = match &pending {
        // A variant waiting for the file is a merge that was never recorded, and a pointer moved
        // under it would leave the merge with a file it does not match. `--only-index` cannot end
        // one, so it is refused rather than answered with a merge still in progress.
        Some(pending) if only_index || !force => {
            return ErrorRetrackMerging {
                path: path.as_str().to_owned(),
                variant: hex(*pending.variant()),
            }
            .into();
        }
        None if only_index => Action::Pointer,
        // Changes the Layout does not name are the run's to discard or to leave alone, and they are
        // not something a retrack may decide by itself.
        None if !force && diff.modified.contains(&path) => {
            return ErrorRetrackModified {
                path: path.as_str().to_owned(),
            }
            .into();
        }
        _ => Action::Content,
    };

    if moved {
        let owner = data.owner().map(str::to_owned);

        if let Err(error) = layout.update_entry(id, data.renewed(owner, target)) {
            return layout_failed(&error);
        }
    }

    // Content is read, and a Vault is bound to, only when the run writes it back — and only when
    // the store here does not already hold it: a retrack of content this Workspace has is local
    // work, and asking for an account would make it fail where there is nothing to fetch.
    let content = match action {
        Action::Pointer => None,
        Action::Content => Some(content_of(vcs, &runtime, target)?),
    };
    let account = match content {
        Some(recorded) if !holds(storage, &runtime, recorded) => {
            let bound = current.get_ref().must_bind()?;
            Some(account_named(&bound, Some(held), None)?)
        }
        _ => None,
    };
    let sources = match &account {
        Some(account) => {
            let tracked = tracked_vault(held);
            let primary = fetch::primary(remote.get_ref(), tracked.as_deref());
            let sources = Sources::new(held, account, remote.get_ref(), primary, **offline)
                .map_err(failed)?
                .with_concurrency(**concurrency);

            Some(sources)
        }
        None => None,
    };

    if let Some(recorded) = content {
        restore(
            storage,
            &runtime,
            &layout,
            &root,
            &path,
            recorded,
            sources.as_ref(),
        )?;

        // Only a forced run reaches here with a merge waiting, and forcing it is what ends it: the
        // variant file is the content that was just replaced, so the record and the file go with it.
        if let Some(pending) = &pending {
            let disk = variant_disk(
                &merging,
                &layout,
                &root,
                vcs,
                &runtime,
                &diff.untagged,
                pending,
            );

            end_merge(&mut merging, &root, id, disk)?;
        }

        if let Some(sources) = &sources {
            sources.report();
        }
    } else if moved {
        // What the file is to agree with now, which is the content the version moved to holds. One
        // the index does not hold is nothing to read, and the agreement is then taken back.
        let agreed = read_version(vcs, &runtime, target)
            .ok()
            .and_then(|version| stored_hash(vcs, &runtime, &version).ok());

        disagree(&layout, &root, &path, agreed)?;
    }

    ResultRetrack {
        path: path.as_str().to_owned(),
        version: hex(target),
        number,
        moved,
        switched: content.is_some(),
    }
    .into()
}

/// What a run is to do to the file on disk, once the reading says it may be acted on.
enum Action {
    /// Move the pointer and leave the file alone.
    Pointer,
    /// Write the version's content back over the file, the pointer moving with it.
    Content,
}

/// What a run that changes the `Uuid` a path records was asked for, and what it was asked about.
// What the run is to do is a handful of independent answers it was given — leaving the file alone,
// jumping between chains, going past unrecorded changes — and making each a two-variant enum would
// say no more than the bool does while writing every use of it out twice.
#[allow(clippy::struct_excessive_bools)]
struct Switching<'a> {
    /// The mode the run named.
    mode: &'a UuidMode,
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
    /// Whether the file on disk is to be left alone.
    only_index: bool,
    /// Whether a version off the way to the other one may be jumped to.
    allow_jump: bool,
    /// Whether unrecorded changes and a merge in progress are gone past anyway.
    force: bool,
}

/// Records another `Uuid` for the path in `asked`, from `--change-upstream` or `--fork`.
///
/// What the path records is dropped and the entry it is to record takes its place, in one batch: an
/// entry is its `Uuid`, so the two are one change and not a removal and a creation a reader could
/// meet between. Nothing else is touched — the index, the stores and the working tree know nothing
/// of a Layout's paths — except the file the run is told to write back or to leave alone.
///
/// What is read is read before anything is written, so a refused run leaves the Layout, the tree and
/// the merge as they were.
///
/// # Errors
///
/// Renders [`ErrorRetrackReference`] when the entry to record is not one the run can name,
/// [`ErrorRetrackSwitch`] when how the two versions stand refuses the change, [`ErrorRetrackModified`]
/// when the file has changes the Layout does not name, [`ErrorRetrackMerging`] when a variant is
/// waiting for the file, [`ErrorOffline`] when a version or a copy is not here and the run is
/// offline, [`ErrorRetrackFailed`] when the store, the index or the Layout refuses, and
/// [`ErrorRetrackLevel`] when a version the run needs cannot be brought.
fn switch_uuid(asked: Switching<'_>) -> Next {
    let Switching {
        mode,
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
        only_index,
        allow_jump,
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
    // records now, or the entry a `--change-upstream` word names.
    let (target, source) = match mode {
        UuidMode::Fork => (Uuid::new_v4(), data.clone()),
        UuidMode::Change { reference } => {
            match target_of(reference, layout, held, old, remote, current, offline) {
                Ok(target) => (target.id, target.data),
                Err(next) => return next,
            }
        }
    };

    // The file is left alone unless the versions say otherwise: an entry that records the same
    // version, or one ahead of what the target records, keeps the work on disk as it is.
    let action = match mode {
        UuidMode::Fork => SwitchAction::Keep,
        UuidMode::Change { .. } => {
            match stand_of(data, &source, vcs, runtime, held, remote, current, offline) {
                Ok(relation) => match switch_action(relation, only_index, allow_jump, force) {
                    Ok(action) => action,
                    Err(SwitchRefusal::Behind) => {
                        return ErrorRetrackSwitch {
                            path: path.as_str().to_owned(),
                            kind: RetrackSwitchError::Behind,
                        }
                        .into();
                    }
                    Err(SwitchRefusal::NotOnChain) => {
                        return ErrorRetrackSwitch {
                            path: path.as_str().to_owned(),
                            kind: RetrackSwitchError::NotOnChain,
                        }
                        .into();
                    }
                },
                Err(next) => return next,
            }
        }
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
    // naming a version the file does not hold — which `rola status` reports as a change, and a
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
                            Ok(sources) => sources.with_concurrency(concurrency),
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

    ResultRetrackUuid {
        path: path.as_str().to_owned(),
        from: old.to_string(),
        to: target.to_string(),
        version: hex(version),
        number,
        forked: matches!(mode, UuidMode::Fork),
        switched,
    }
    .into()
}

/// The `Uuid` a run that changes an entry's `Uuid` is to record, and what that entry holds.
struct Target {
    /// The `Uuid` the path is to record.
    id: Uuid,
    /// What it holds, from the copy of the Vault's Layout that has it or from this Layout.
    data: MutableData,
}

/// Reads what the entry a `--change-upstream` word names is to be recorded as.
///
/// A word is a `Uuid` first, and a path of the Vault's own Layout after — a local path is not a way
/// to name an entry here, since what is being changed is which entry the local path records. A
/// `Uuid` needs no path and no fetch: one this Layout holds at no path is the entry to record, and
/// the copy of the Vault's Layout is read only when it is here already. A path is read from that
/// copy, fetched first when it is not here.
fn target_of(
    text: &str,
    layout: &Layout,
    held: &Workspace,
    old: Uuid,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: bool,
) -> Result<Target, Next> {
    let vault = tracked_vault(held);
    let reference = reference_of(text).map_err(Into::<Next>::into)?;

    let (id, source) = match reference {
        Reference::Uuid(id) => {
            // The copy is used when it is here, and never fetched for a `Uuid`: a `Uuid` gives no
            // reason to reach anywhere, and the entry may be one this Layout holds at no path.
            let copy = match &vault {
                Some(name) => open_copy(held, name)?,
                None => None,
            };
            let source = copy
                .as_ref()
                .and_then(|copy| copy.entry(id))
                .or_else(|| layout.entry(id));

            (id, source)
        }
        Reference::Path(path) => {
            let Some(name) = vault else {
                return Err(ErrorRetrackReference {
                    reference: text.to_owned(),
                    kind: RetrackReferenceError::NoVault,
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

            let Some(id) = copy.id_of(&path) else {
                return Err(ErrorRetrackReference {
                    reference: text.to_owned(),
                    kind: RetrackReferenceError::NotUpstream { vault: name },
                }
                .into());
            };

            let source = copy.entry(id);

            (id, source)
        }
    };

    // The entry the path records already is the one case every other check would be told apart
    // from: it is named by a path — this one — so it is refused for being the entry it is.
    if id == old {
        return Err(ErrorRetrackReference {
            reference: text.to_owned(),
            kind: RetrackReferenceError::SameUuid {
                uuid: id.to_string(),
            },
        }
        .into());
    }
    if let Some(named) = layout.path_of(id) {
        return Err(ErrorRetrackReference {
            reference: text.to_owned(),
            kind: RetrackReferenceError::Named {
                uuid: id.to_string(),
                path: named.as_str().to_owned(),
            },
        }
        .into());
    }

    let Some(data) = source else {
        return Err(ErrorRetrackReference {
            reference: text.to_owned(),
            kind: RetrackReferenceError::UnknownUuid {
                uuid: id.to_string(),
            },
        }
        .into());
    };

    // The root version is what an entry is at before anything was recorded for it, so an entry that
    // records it records no version to switch to.
    if data.version() == [0; 32] {
        return Err(ErrorRetrackReference {
            reference: text.to_owned(),
            kind: RetrackReferenceError::NoVersion {
                uuid: id.to_string(),
            },
        }
        .into());
    }

    Ok(Target { id, data })
}

/// What a `--change-upstream` word was read as.
enum Reference {
    /// A `Uuid`, which needs no path to name an entry.
    Uuid(Uuid),
    /// A path of the Vault's own Layout.
    Path(LayoutPath),
}

/// Reads a `--change-upstream` word as the one thing it names.
///
/// A `Uuid` is read first because it is the shape that names an entry without a path at all; every
/// other word is a path of the Vault's own Layout, and one that is not a path either is refused
/// rather than looked for.
fn reference_of(text: &str) -> Result<Reference, ErrorRetrackReference> {
    if let Ok(id) = Uuid::from_str(text) {
        return Ok(Reference::Uuid(id));
    }

    LayoutPath::new(text)
        .map(Reference::Path)
        .map_err(|_| ErrorRetrackReference {
            reference: text.to_owned(),
            kind: RetrackReferenceError::NotAUuidOrPath,
        })
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
/// cannot be read cannot be compared — which is the one step of this that may leave the machine.
fn stand_of(
    data: &MutableData,
    target: &MutableData,
    vcs: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    held: &Workspace,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: bool,
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
                end: vault_end(held, remote, current, None)?,
                text: &text,
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
    /// The path records a version behind the one it would record.
    Behind,
    /// The two are on different chains, and jumping between chains was not allowed.
    NotOnChain,
}

/// What changing the `Uuid` a path records does, for how the two versions stand.
///
/// The two versions being on one chain is what decides how little is done: the file already holds
/// what the path records, so an entry that records the same version or a later one is a change of
/// identity and nothing more, and one that records an earlier version is a change of what the file
/// holds — refused unless the run says to switch the content or to leave the file behind. Different
/// chains are not a step back at all, so they are refused until the run says it means to jump.
fn switch_action(
    relation: Relation,
    only_index: bool,
    allow_jump: bool,
    force: bool,
) -> Result<SwitchAction, SwitchRefusal> {
    match relation {
        Relation::Equal | Relation::Ahead => Ok(SwitchAction::Keep),
        Relation::Behind if only_index => Ok(SwitchAction::Pointer),
        Relation::Behind if force => Ok(SwitchAction::Content),
        Relation::Behind => Err(SwitchRefusal::Behind),
        Relation::Apart if !allow_jump => Err(SwitchRefusal::NotOnChain),
        Relation::Apart if only_index => Ok(SwitchAction::Pointer),
        Relation::Apart => Ok(SwitchAction::Content),
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
/// only a path the tree holds untagged — and the target's content is replaced by the caller.
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
    /// The level as it was written, for the words a refusal uses.
    text: &'a str,
    /// Where the pointer is to go.
    level: &'a Until,
    /// The path the entry is at, for the words a refusal uses.
    path: &'a str,
    /// Whether a version off the way back may be jumped to.
    allow_jump: bool,
}

/// The version the pointer is to be moved to, and its number when it is known.
///
/// `upstream` is the Vault's version for the [`Until::Vault`] level, read before this because only
/// that level leaves the local index; every other level is worked out from the chain the entry is
/// the top of.
fn resolve_target(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    chain: &[Version],
    asked: &Asked<'_>,
    upstream: Option<Blake3Hash>,
) -> Result<(Blake3Hash, Option<u64>), Next> {
    let top = chain.len() - 1;
    let out_of_chain = || {
        ErrorRetrackLevel {
            until: asked.text.to_owned(),
            kind: RetrackLevelError::OutOfChain {
                path: asked.path.to_owned(),
            },
        }
        .into()
    };

    match asked.level {
        Until::Steps(back) => {
            let Ok(back) = usize::try_from(*back) else {
                return Err(out_of_chain());
            };
            let Some(version) = chain.get(back) else {
                return Err(out_of_chain());
            };

            Ok((*version.hash().digest(), Some(number_at(top, back))))
        }
        Until::Number(number) => {
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
        Until::Hash(key) => {
            if let Some((at, version)) = chain
                .iter()
                .enumerate()
                .find(|(_, version)| version.hash().digest() == key.digest())
            {
                return Ok((*version.hash().digest(), Some(number_at(top, at))));
            }

            let Ok(version) = read_version(index, runtime, *key.digest()) else {
                return Err(ErrorRetrackLevel {
                    until: asked.text.to_owned(),
                    kind: RetrackLevelError::NotAVersion,
                }
                .into());
            };

            if !asked.allow_jump {
                return Err(ErrorRetrackLevel {
                    until: asked.text.to_owned(),
                    kind: RetrackLevelError::NotAncestor {
                        path: asked.path.to_owned(),
                    },
                }
                .into());
            }

            let number = runtime.block_on(index.version_num(&version)).ok();
            Ok((*key.digest(), number))
        }
        Until::Vault(_) => {
            // UNWRAP: the caller reads the Vault's version for exactly this level, so one is here.
            let hash = upstream.expect("the Vault's version is read for the Vault level");
            let number = read_version(index, runtime, hash)
                .ok()
                .and_then(|version| runtime.block_on(index.version_num(&version)).ok());

            Ok((hash, number))
        }
    }
}

/// The version the Vault `alias` names records for the entry `id`.
///
/// What is read is the copy of the Vault's own Layout the Workspace keeps, and one that was never
/// fetched is fetched first — the only step of a retrack that may leave the machine, and one an
/// offline run does not take: it reads a copy that is here or fails when it opens one that is not.
/// Which Vault `alias` names is the Workspace's to say, and no check is made that the Layout being
/// worked in tracks that Vault: the Vault named is the upstream, whichever it is.
fn upstream_version(
    held: &Workspace,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    alias: &str,
    path: &LayoutPath,
    id: Uuid,
    offline: bool,
) -> Result<Blake3Hash, Next> {
    let name = remote
        .get_ref()
        .name_or_default(alias)
        .map_err(mingling::Routable::to_chain)?;

    let mut copy = readonly_layout_copy(held, &name, VAULT_LAYOUT_NAME);

    // A copy that is here is read as it is, however old: reaching for the Vault is what a run that
    // has not fetched is for, not something every run pays for — and an offline run does not reach
    // for it at all, so a copy that is not here fails at the open below.
    if copy.is_none() && !offline {
        let target = remote
            .get_ref()
            .vault_or_default(name.clone())
            .map_err(mingling::Routable::to_chain)?;
        let account_name = current.get_ref().must_bind().map_err(Into::<Next>::into)?;
        let account = account_named(&account_name, Some(held), None).map_err(Into::<Next>::into)?;

        action_fetch_layout(held, &account, target.to_string(), name.clone())
            .map_err(mingling::Routable::to_chain)?;

        copy = readonly_layout_copy(held, &name, VAULT_LAYOUT_NAME);
    }

    let Some(dir) = copy else {
        return Err(failed(
            "the Vault's Layout is not here, and this run may not fetch it".to_owned(),
        ));
    };

    let copy = Layout::open_existing(&dir).map_err(|error| failed(error.to_string()))?;

    let Some(data) = copy.entry(id) else {
        return Err(ErrorRetrackUpstream {
            vault: name,
            path: path.as_str().to_owned(),
        }
        .into());
    };

    // The root version is what an entry is at before anything was recorded, so an upstream that
    // names it records no version either: there is nothing to be consistent with.
    if data.version() == [0; 32] {
        return Err(ErrorRetrackUpstream {
            vault: name,
            path: path.as_str().to_owned(),
        }
        .into());
    }

    Ok(data.version())
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
/// A retrack may go to a version the Vault holds and this side never fetched. The chain under it —
/// the version, the variant that produced it, the words they are named by, and so on down to a version
/// this index already holds — is asked for in one exchange, and what the Vault walks is written here
/// as one pack: what is left behind is a chain as readable here as one a sync brought.
struct Bringer<'a> {
    /// The Workspace the exchange is spoken from.
    workspace: &'a Workspace,
    /// The runtime the exchange is waited on.
    runtime: &'a tokio::runtime::Runtime,
    /// Where the objects are read from.
    end: VaultEnd,
    /// The level as the run wrote it, for the refusal that names it.
    text: &'a str,
}

impl Bringer<'_> {
    /// Walks the chain from `hash` down to a version this index already holds, writing each object.
    ///
    /// One exchange rather than one per object: the Vault walks the chain itself, down to the first
    /// version this side already holds, and sends the whole of what is missing — which is one
    /// connection and one pack here however long the chain is.
    ///
    /// # Errors
    ///
    /// [`ErrorRetrackLevel`] when the Vault answers with something that is not a version, and
    /// [`ErrorRetrackFailed`] when the exchange, the index or the chain refuses.
    fn bring(&self, hash: Blake3Hash) -> Result<(), Next> {
        let said = self
            .runtime
            .block_on(action_bring_remote_index_async(
                self.workspace,
                &self.end.account,
                self.end.address.clone(),
                Key::new(hash).hex(),
                Progress::silent(),
            ))
            .map_err(|error| failed(error.to_string()))?;

        let chain: Chain =
            serde_json::from_str(&said).map_err(|error| failed(error.to_string()))?;

        match chain {
            Chain::Brought => Ok(()),
            Chain::NotAVersion => Err(ErrorRetrackLevel {
                until: self.text.to_owned(),
                kind: RetrackLevelError::NotAVersion,
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
/// The Vault named is the one the version came from; a hash names none, so the Vault the Layout
/// tracks is the one asked.
///
/// # Errors
///
/// [`ErrorRetrackFailed`] when the Layout tracks no Vault or the Vault or account cannot be read.
fn vault_end(
    held: &Workspace,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    vault: Option<&str>,
) -> Result<VaultEnd, Next> {
    let name = match vault {
        Some(name) => name.to_owned(),
        None => tracked_vault(held)
            .ok_or_else(|| failed(t!("retrack.err_no_vault").trim().to_owned()))?,
    };
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
/// retrack that may leave the machine, and `sources` is the run that may do it — nothing when the
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
/// Moving the pointer and leaving the file alone leaves the two apart — the file is what the newer
/// version held, and the Layout names an older one — but the reading does not tell that from the
/// pointer: what it compares is the file against what the Layout was last known to agree with. So
/// the agreement is written afresh here rather than left standing, or the very next `rola status`
/// would call a file with changes never recorded one with nothing to record.
///
/// What is written is the content the new version holds, so a file that is then put back to it by
/// hand reads as agreed again — a plain "nothing" would leave such a file reported as changed with
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
/// command that takes one. A name holds `@` often enough — `logo@2x.png` — that a plain one is
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

/// Where a run's `--until` is to move the pointer to.
enum Until {
    /// N versions back.
    Steps(u64),
    /// The version numbered N.
    Number(u64),
    /// The version a hash names.
    Hash(Key),
    /// The version the Vault named records.
    Vault(String),
}

/// Reads what a run gave `--until` as where the pointer is to go.
///
/// One step back is what saying nothing means. `~N` is a step count, digits are a version number,
/// a hash is a version, and anything else is taken as a Vault's name — the order a run is likely
/// to have meant, with the two shapes a Vault's name can never be settled first. The hash may be
/// only the head of one, and `versions` is what it is settled against; a name of hex digits is
/// therefore read as a hash where it would once have been a Vault's name.
///
/// # Errors
///
/// [`ErrorRetrackLevel`] when `text` is no level, and when it is a hash that names no one version
/// or more than one.
fn until_of(
    text: Option<&str>,
    versions: impl FnOnce() -> Vec<Key>,
) -> Result<Until, ErrorRetrackLevel> {
    let Some(text) = text else {
        return Ok(Until::Steps(1));
    };

    let not_a_level = || ErrorRetrackLevel {
        until: text.to_owned(),
        kind: RetrackLevelError::NotALevel,
    };

    if text == "-1" {
        return Ok(Until::Steps(1));
    }

    // Nothing at all names no Vault either: a name is something, and `--until=` is not.
    if text.is_empty() {
        return Err(not_a_level());
    }

    if let Some(rest) = text.strip_prefix('~') {
        return rest
            .parse::<u64>()
            .map(Until::Steps)
            .map_err(|_| not_a_level());
    }

    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        return text
            .parse::<u64>()
            .map(Until::Number)
            .map_err(|_| not_a_level());
    }

    // A sign that is not the `-1` above reads as an option or a mistyped step count, never as a
    // Vault's name: a name is not what a run wrote.
    if text.starts_with('-') || text.starts_with('+') {
        return Err(not_a_level());
    }

    match resolve_hash(text, versions) {
        Ok(key) => Ok(Until::Hash(key)),
        // What is not hash-shaped at all is a Vault's name, which is read later.
        Err(HashMiss::Malformed) => Ok(Until::Vault(text.to_owned())),
        Err(HashMiss::Unknown) => Err(ErrorRetrackLevel {
            until: text.to_owned(),
            kind: RetrackLevelError::NotAVersion,
        }),
        Err(HashMiss::Ambiguous(candidates)) => Err(ErrorRetrackLevel {
            until: text.to_owned(),
            kind: RetrackLevelError::AmbiguousHash { candidates },
        }),
    }
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

/// Result: a file's version pointer was moved, or was already where it was asked to go.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultRetrack {
    /// Where the entry sits in the Layout.
    path: String,
    /// The version it is at now, as hex.
    version: String,
    /// That version's number in its chain, when it is known.
    number: Option<u64>,
    /// Whether the pointer moved.
    moved: bool,
    /// Whether the version's content was written back over the file.
    switched: bool,
}

#[renderer(buffer)]
pub fn render_result_retrack(result: ResultRetrack) {
    let version = named(&result.version, result.number);
    let said = match (result.moved, result.switched) {
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
    };

    r_println!("{}", said.trim());
}

/// Result: the `Uuid` a file's entry is recorded under was changed.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultRetrackUuid {
    /// Where the entry sits in the Layout.
    path: String,
    /// The `Uuid` it was recorded under.
    from: String,
    /// The `Uuid` it is recorded under now.
    to: String,
    /// The version it records now, as hex.
    version: String,
    /// That version's number in its chain, when it is known.
    number: Option<u64>,
    /// Whether the run gave it a fresh `Uuid` rather than recording another entry's.
    forked: bool,
    /// Whether the version's content was written back over the file.
    switched: bool,
}

#[renderer(buffer)]
pub fn render_result_retrack_uuid(result: ResultRetrackUuid) {
    let version = named(&result.version, result.number);
    let said = if result.forked {
        t!(
            "retrack.result_uuid_forked",
            path = result.path,
            to = result.to,
            version = version
        )
    } else if result.switched {
        t!(
            "retrack.result_uuid_switched",
            path = result.path,
            from = result.from,
            to = result.to,
            version = version
        )
    } else {
        t!(
            "retrack.result_uuid_kept",
            path = result.path,
            from = result.from,
            to = result.to,
            version = version
        )
    };

    r_println!("{}", said.trim());
}

/// Error: `--change-upstream` and `--fork` say different things, or one of them was given beside a
/// version level.
#[derive(Grouped)]
pub struct ErrorRetrackMode;

impl Failure for ErrorRetrackMode {
    fn name(&self) -> &'static str {
        "error_retrack_mode"
    }

    fn reason(&self) -> String {
        t!("retrack.err_mode").trim().to_string()
    }
}

failure!(ErrorRetrackMode);

#[renderer(buffer)]
pub fn render_error_retrack_mode(_: ErrorRetrackMode, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("retrack.err_mode").trim()));
    r_eprintln!("{}", help_line!(t!("retrack.err_mode_help").trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: a `--change-upstream` word names no entry the run can record.
#[derive(Grouped)]
pub struct ErrorRetrackReference {
    /// What was written after `--change-upstream`.
    reference: String,
    /// What is wrong with it.
    kind: RetrackReferenceError,
}

/// The ways a `--change-upstream` word names no entry to record.
pub enum RetrackReferenceError {
    /// It is neither a `Uuid` nor a path a Layout may hold.
    NotAUuidOrPath,
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

impl Failure for ErrorRetrackReference {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackReferenceError::NotAUuidOrPath => "error_retrack_reference_not_a_uuid_or_path",
            RetrackReferenceError::NoVault => "error_retrack_reference_no_vault",
            RetrackReferenceError::NotUpstream { .. } => "error_retrack_reference_not_upstream",
            RetrackReferenceError::UnknownUuid { .. } => "error_retrack_reference_unknown_uuid",
            RetrackReferenceError::SameUuid { .. } => "error_retrack_reference_same_uuid",
            RetrackReferenceError::Named { .. } => "error_retrack_reference_named",
            RetrackReferenceError::NoVersion { .. } => "error_retrack_reference_no_version",
        }
    }

    fn reason(&self) -> String {
        let said = match &self.kind {
            RetrackReferenceError::NotAUuidOrPath => {
                t!(
                    "retrack.err_ref_not_a_uuid_or_path",
                    reference = self.reference
                )
            }
            RetrackReferenceError::NoVault => {
                t!("retrack.err_ref_no_vault", reference = self.reference)
            }
            RetrackReferenceError::NotUpstream { vault } => t!(
                "retrack.err_ref_not_upstream",
                reference = self.reference,
                vault = vault
            ),
            RetrackReferenceError::UnknownUuid { uuid } => {
                t!("retrack.err_ref_unknown_uuid", uuid = uuid)
            }
            RetrackReferenceError::SameUuid { uuid } => {
                t!("retrack.err_ref_same_uuid", uuid = uuid)
            }
            RetrackReferenceError::Named { uuid, path } => {
                t!("retrack.err_ref_named", uuid = uuid, path = path)
            }
            RetrackReferenceError::NoVersion { uuid } => {
                t!("retrack.err_ref_no_version", uuid = uuid)
            }
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackReference);

#[renderer(buffer)]
pub fn render_error_retrack_reference(error: ErrorRetrackReference, ec: &mut ResExitCode) {
    let help = match error.kind {
        RetrackReferenceError::NotAUuidOrPath => t!("retrack.err_ref_not_a_uuid_or_path_help"),
        RetrackReferenceError::NoVault => t!("retrack.err_ref_no_vault_help"),
        RetrackReferenceError::NotUpstream { .. } => t!("retrack.err_ref_not_upstream_help"),
        RetrackReferenceError::UnknownUuid { .. } => t!("retrack.err_ref_unknown_uuid_help"),
        RetrackReferenceError::SameUuid { .. } => t!("retrack.err_ref_same_uuid_help"),
        RetrackReferenceError::Named { .. } => t!("retrack.err_ref_named_help"),
        RetrackReferenceError::NoVersion { .. } => t!("retrack.err_ref_no_version_help"),
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
    ec.exit_code = EC_ERR_RETRACK_ARGUMENT;
}

/// Error: the version a path records and the version its entry is to record refuse the change.
#[derive(Grouped)]
pub struct ErrorRetrackSwitch {
    /// Where the entry sits in the Layout.
    path: String,
    /// Why the change is refused.
    kind: RetrackSwitchError,
}

/// The ways two versions refuse a change of `Uuid`.
pub enum RetrackSwitchError {
    /// The version the path records is behind the one the entry records.
    Behind,
    /// The two versions are on different chains.
    NotOnChain,
}

impl Failure for ErrorRetrackSwitch {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackSwitchError::Behind => "error_retrack_switch_behind",
            RetrackSwitchError::NotOnChain => "error_retrack_switch_not_on_chain",
        }
    }

    fn reason(&self) -> String {
        let said = match self.kind {
            RetrackSwitchError::Behind => t!("retrack.err_switch_behind", path = self.path),
            RetrackSwitchError::NotOnChain => {
                t!("retrack.err_switch_not_on_chain", path = self.path)
            }
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackSwitch);

#[renderer(buffer)]
pub fn render_error_retrack_switch(error: ErrorRetrackSwitch, ec: &mut ResExitCode) {
    let help = match error.kind {
        RetrackSwitchError::Behind => t!("retrack.err_switch_behind_help"),
        RetrackSwitchError::NotOnChain => t!("retrack.err_switch_not_on_chain_help"),
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
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

/// Error: more than one file was named.
#[derive(Grouped)]
pub struct ErrorRetrackManyFiles {
    /// How many were named.
    count: usize,
}

impl Failure for ErrorRetrackManyFiles {
    fn name(&self) -> &'static str {
        "error_retrack_many_files"
    }

    fn reason(&self) -> String {
        t!("retrack.err_many_files", count = self.count)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackManyFiles);

#[renderer(buffer)]
pub fn render_error_retrack_many_files(error: ErrorRetrackManyFiles, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_many_files_help").trim()));
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

/// Error: `--until` names nothing the entry can be moved to.
#[derive(Grouped)]
pub struct ErrorRetrackLevel {
    /// What was given.
    until: String,
    /// What is wrong with it.
    kind: RetrackLevelError,
}

/// The ways `--until` names no version.
pub enum RetrackLevelError {
    /// It is not a level, a hash or a Vault's name.
    NotALevel,
    /// It is a level past an end of the chain the entry is the top of.
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

impl Failure for ErrorRetrackLevel {
    fn name(&self) -> &'static str {
        match self.kind {
            RetrackLevelError::NotALevel => "error_retrack_not_a_level",
            RetrackLevelError::OutOfChain { .. } => "error_retrack_out_of_chain",
            RetrackLevelError::NotAVersion => "error_retrack_not_a_version",
            RetrackLevelError::AmbiguousHash { .. } => "error_retrack_ambiguous_hash",
            RetrackLevelError::NotAncestor { .. } => "error_retrack_not_ancestor",
        }
    }

    fn reason(&self) -> String {
        let said = match &self.kind {
            RetrackLevelError::NotALevel => t!("retrack.err_not_a_level", until = self.until),
            RetrackLevelError::OutOfChain { path } => {
                t!("retrack.err_out_of_chain", until = self.until, path = path)
            }
            RetrackLevelError::NotAVersion => {
                t!("retrack.err_not_a_version", until = self.until)
            }
            RetrackLevelError::AmbiguousHash { candidates } => t!(
                "common.err_hash_ambiguous",
                hash = self.until,
                candidates = candidates.join(", ")
            ),
            RetrackLevelError::NotAncestor { path } => {
                t!("retrack.err_not_ancestor", until = self.until, path = path)
            }
        };

        said.trim().to_string()
    }
}

failure!(ErrorRetrackLevel);

#[renderer(buffer)]
pub fn render_error_retrack_level(error: ErrorRetrackLevel, ec: &mut ResExitCode) {
    let (help, code) = match error.kind {
        RetrackLevelError::NotALevel => {
            (t!("retrack.err_not_a_level_help"), EC_ERR_RETRACK_ARGUMENT)
        }
        RetrackLevelError::OutOfChain { .. } => {
            (t!("retrack.err_out_of_chain_help"), EC_ERR_RETRACK)
        }
        RetrackLevelError::NotAVersion => (t!("retrack.err_not_a_version_help"), EC_ERR_RETRACK),
        RetrackLevelError::AmbiguousHash { .. } => (t!("common.err_hash_help"), EC_ERR_RETRACK),
        RetrackLevelError::NotAncestor { .. } => {
            (t!("retrack.err_not_ancestor_help"), EC_ERR_RETRACK)
        }
    };

    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(help.trim()));
    ec.exit_code = code;
}

/// Error: the Vault's Layout records no version for the file.
#[derive(Grouped)]
pub struct ErrorRetrackUpstream {
    /// The Vault whose Layout records none.
    vault: String,
    /// Where the entry sits in this Layout.
    path: String,
}

impl Failure for ErrorRetrackUpstream {
    fn name(&self) -> &'static str {
        "error_retrack_upstream"
    }

    fn reason(&self) -> String {
        t!("retrack.err_upstream", vault = self.vault, path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorRetrackUpstream);

#[renderer(buffer)]
pub fn render_error_retrack_upstream(error: ErrorRetrackUpstream, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("retrack.err_upstream_help").trim()));
    ec.exit_code = EC_ERR_RETRACK;
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

#[cfg(test)]
mod tests {
    use super::{RetrackLevelError, Until, until_of};
    use librorolala::storage::Key;

    /// The default and the two spellings of one step back are the same level.
    #[test]
    fn one_step_back_is_the_default_and_the_two_spellings_of_it() {
        assert!(matches!(until_of(None, Vec::new), Ok(Until::Steps(1))));
        assert!(matches!(
            until_of(Some("-1"), Vec::new),
            Ok(Until::Steps(1))
        ));
        assert!(matches!(
            until_of(Some("~1"), Vec::new),
            Ok(Until::Steps(1))
        ));
    }

    /// A step count is `~N`, a version number is digits, a hash is a hash, and anything else is
    /// the name of a Vault.
    #[test]
    fn each_shape_is_read_as_the_level_it_is() {
        assert!(matches!(
            until_of(Some("~3"), Vec::new),
            Ok(Until::Steps(3))
        ));
        assert!(matches!(
            until_of(Some("~0"), Vec::new),
            Ok(Until::Steps(0))
        ));
        assert!(matches!(
            until_of(Some("0"), Vec::new),
            Ok(Until::Number(0))
        ));
        assert!(matches!(
            until_of(Some("12"), Vec::new),
            Ok(Until::Number(12))
        ));
        assert!(
            matches!(until_of(Some("origin"), Vec::new), Ok(Until::Vault(name)) if name == "origin")
        );

        let hex = "0f".repeat(32);
        let Ok(Until::Hash(key)) = until_of(Some(&hex), Vec::new) else {
            panic!("a hash is read as a hash");
        };
        assert_eq!(key, Key::new([0x0f; 32]));
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
            until_of(Some("0f0f"), || vec![one]),
            Ok(Until::Hash(key)) if key == one
        ));
        assert!(matches!(
            until_of(Some("0f0f"), Vec::new),
            Err(super::ErrorRetrackLevel {
                kind: RetrackLevelError::NotAVersion,
                ..
            })
        ));
        assert!(matches!(
            until_of(Some("0f0f"), || vec![one, two]),
            Err(super::ErrorRetrackLevel {
                kind: RetrackLevelError::AmbiguousHash { .. },
                ..
            })
        ));
    }

    /// What is neither a level nor a hash is refused where it cannot be a name at all: a sign
    /// that is not `-1`, a step count with no number, and something hash-shaped that names nothing.
    #[test]
    fn a_level_that_is_not_one_is_refused() {
        assert!(until_of(Some("~"), Vec::new).is_err());
        assert!(until_of(Some("~x"), Vec::new).is_err());
        assert!(until_of(Some("-5"), Vec::new).is_err());
        assert!(until_of(Some("+1"), Vec::new).is_err());
        assert!(until_of(Some(""), Vec::new).is_err());
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
    /// flags the run gave: a version the same or a later one keeps the file, an earlier one needs
    /// the run to say what happens, and another chain needs the jump allowed first.
    #[test]
    fn what_a_change_of_uuid_does_to_the_file_follows_the_versions_and_the_flags() {
        use super::{Relation, SwitchAction, SwitchRefusal, switch_action};

        assert_eq!(
            switch_action(Relation::Equal, false, false, false),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Equal, true, true, true),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Ahead, false, false, false),
            Ok(SwitchAction::Keep)
        );
        assert_eq!(
            switch_action(Relation::Ahead, true, true, true),
            Ok(SwitchAction::Keep)
        );

        assert_eq!(
            switch_action(Relation::Behind, false, false, false),
            Err(SwitchRefusal::Behind)
        );
        assert_eq!(
            switch_action(Relation::Behind, true, false, false),
            Ok(SwitchAction::Pointer)
        );
        assert_eq!(
            switch_action(Relation::Behind, false, false, true),
            Ok(SwitchAction::Content)
        );

        assert_eq!(
            switch_action(Relation::Apart, false, false, false),
            Err(SwitchRefusal::NotOnChain)
        );
        // A jump the run did not allow is refused however else it was asked to go.
        assert_eq!(
            switch_action(Relation::Apart, true, false, true),
            Err(SwitchRefusal::NotOnChain)
        );
        assert_eq!(
            switch_action(Relation::Apart, false, true, false),
            Ok(SwitchAction::Content)
        );
        assert_eq!(
            switch_action(Relation::Apart, false, true, true),
            Ok(SwitchAction::Content)
        );
        assert_eq!(
            switch_action(Relation::Apart, true, true, false),
            Ok(SwitchAction::Pointer)
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

    /// A `--change-upstream` word is read as a `Uuid` before it is read as a path: a `Uuid` is also a
    /// path a Layout may hold, so reading it as one would send the run looking in a Vault for an
    /// entry it may hold here.
    #[test]
    fn a_change_upstream_word_is_a_uuid_before_it_is_a_path() {
        use super::{Reference, RetrackReferenceError, reference_of};
        use uuid::Uuid;

        let id = Uuid::from_u128(7);

        assert!(matches!(reference_of(&id.to_string()), Ok(Reference::Uuid(read)) if read == id));
        assert!(
            matches!(reference_of("art/hero.psd"), Ok(Reference::Path(path)) if path.as_str() == "art/hero.psd")
        );
        assert!(matches!(
            reference_of(""),
            Err(super::ErrorRetrackReference {
                kind: RetrackReferenceError::NotAUuidOrPath,
                ..
            })
        ));
    }
}

//! The `rola retrack` command: set what a file's Layout entry records.
//!
//! A Layout names one version for each path, and the content behind a version stays in the index
//! whether or not anything points at it. What a run names after the file is what the path is to
//! record: a version (a step back, a number, or a hash) which moves the entry's pointer, or another
//! entry (a `Uuid`, or a path of the Vault's own Layout) which makes the path record that entry in
//! place of the one it recorded.
//!
//! The order of the work itself is the library's, see [`librorolala::write::retrack`]: what is read
//! before what is written, which refusal stops the run, and where the content is written back over
//! the file. What is left here is the shape of the command: the words it takes, the resources it
//! resolves, the words it prints and the exit codes it ends on.

// A retrack resolves a target, a Layout and the account and Vault it may reach before it hands the
// work over, and every step of that is a refusal of its own to say. The handler's parameters are the
// resources the framework injects, so how many of them there are is what the command needs of the
// run rather than a signature this program shaped, and the progress setting is one of those
// resources, which is what the injection gives by reference.
#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::trivially_copy_pass_by_ref
)]

use std::path::Path;

use librorolala::auth::Account;
use librorolala::ignore::IgnoreRuleSet;
use librorolala::layout::{Layout, LayoutPath};
use librorolala::protocol::VaultAddress;
use librorolala::status::tracked_vault;
use librorolala::workspace::Workspace;
use librorolala::write::WriteError;
use librorolala::write::path::{PathRefusal, local_path};
use librorolala::write::retrack::{
    EntryRefusal, Recorded, Recording, Request, RetrackMessages, RetrackOptions, RetrackOutcome,
    RetrackRefusal, TargetRefusal, recording_of, retrack_path, tree_above,
};
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
    ErrorRemoteVault, ResConcurrency, ResCurrentRemoteVault, ResForce, ResOffline,
    ResProgressSetting, ResStorageSet, ResVCSIndex, ResVault, ResWorkspace,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd, warn_line};
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{
    IndexObject, index_keys, offer, positional, strip_written, target_words, typing_flag,
};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_RETRACK, EC_ERR_RETRACK_ARGUMENT, EC_ERR_RETRACK_DIRTY, EC_HELP};
use crate::failure::failure;
use crate::fetch;
use crate::keys::account_named;
use crate::layout::{ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutShouldInWorkspace, chosen};
use crate::progress::Reporting;

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
/// file, [`ErrorRetrackTreeDirty`] when a tree holds work that was never recorded, [`ErrorOffline`]
/// when a version or a copy is not here and the run is offline, [`ErrorLayoutShouldInWorkspace`]
/// when the run is not inside a Workspace, the Vault and account failures of the run
/// ([`ErrorRemoteVault`] and [`crate::account::ErrorNoAccount`]) when it reaches for one and the
/// Workspace cannot say which, and [`ErrorRetrackFailed`] when the store, the index or the Layout
/// refuses.
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

/// The failures the run read before the library could ask for them.
///
/// A run that needs no Vault and no account never reads what is here: the library says which of them
/// it needed, and only then is the answer it would have given the one the run ends on.
#[derive(Default)]
struct Held {
    /// What binding an account came to, when it could not.
    account: Option<Next>,
    /// What naming the Vault the Layout tracks came to, when it could not.
    vault: Option<Next>,
}

#[chain(routeify)]
pub fn handle_retrack(
    state: StateRetrack,
    workspace: &mut LazyRes<ResWorkspace>,
    vault: &mut LazyRes<ResVault>,
    storage: &mut LazyRes<ResStorageSet>,
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
        match recording_of(target.as_deref(), || {
            index_keys(index.get_ref().as_ref(), IndexObject::Version)
        }) {
            Ok(recording) => recording,
            Err(kind) => {
                return ErrorRetrackTarget {
                    target: written,
                    kind: target_kind(kind),
                }
                .into();
            }
        }
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

    // How the Workspace reaches a Vault and which account it acts as are answered here, before the
    // library asks: each is a failure of its own with its own exit code, and a library cannot speak
    // one. A run that asks for neither is never told what came of reading them.
    let mut answered = Held::default();
    let tracked = tracked_vault(held);
    let reach = reachable_vault(remote.get_ref(), tracked.as_deref(), &mut answered);
    let account = bound_account(current.get_ref(), held, &mut answered);
    // What may be asked for content is every Vault the Workspace has bound, the one it named first.
    let candidates = fetch::candidates(remote.get_ref(), reach.clone());
    let messages = RetrackMessages::default();
    let request = Request {
        workspace: held,
        layout: &layout,
        index: index.get_ref().as_ref(),
        store: storage.get_ref().storage(),
        account: account.as_ref(),
        vault: tracked.as_deref(),
        reach: reach.as_ref(),
        candidates: &candidates,
        offline: **offline,
        concurrency: **concurrency,
        rules: read_by(held, &layout, &path),
        messages: &messages,
    };
    let options = RetrackOptions {
        fork,
        only_index,
        force,
        deny_jump,
    };

    let reporting = Reporting::start(*progress);
    let cancellation = crate::cancel::on_ctrl_c();
    let answer = {
        let progress = reporting.progress();

        retrack_path(
            &request,
            &path,
            &recording,
            options,
            &progress,
            cancellation,
        )
    };

    // What the Vaults handed over is said while the watching still stands, which is where the run
    // said it before the workflow moved: a line said after the bar comes down reads as belonging to
    // whatever is printed next.
    if let Ok(outcome) = &answer {
        said_handed_over(&outcome.took);
    }
    reporting.finish();

    match answer {
        Ok(outcome) => moved_state(outcome),
        Err(refusal) => refusal_state(refusal, &file, &written, &mut answered),
    }
}

/// The rules a run reads the work by: the ones a tree is read with, and the empty set for a file.
///
/// A retrack moves a version pointer of an entry the Layout names, which the ignore rules have no
/// jurisdiction over, so a file's reading is taken unfiltered. Only a tree is read by the rules, and
/// only a tree pays for loading them. A path inside a tree is that tree, which is what the library
/// reads the work as too.
fn read_by(held: &Workspace, layout: &Layout, path: &LayoutPath) -> IgnoreRuleSet {
    if tree_above(layout, path).is_some() {
        crate::ignore::rules(held, false)
    } else {
        IgnoreRuleSet::empty()
    }
}

/// The name and the address of the Vault the Layout tracks, when the Workspace can say.
///
/// A name is read before an address, which is the order the steps that reach it read them, so the
/// failure a run would have been told is the one it is told here.
fn reachable_vault(
    remote: &ResCurrentRemoteVault,
    tracked: Option<&str>,
    answered: &mut Held,
) -> Option<(String, VaultAddress)> {
    let tracked = tracked?;

    match name_and_address(remote, tracked) {
        Ok(reached) => Some(reached),
        Err(error) => {
            answered.vault = Some(Routable::to_chain(error));

            None
        }
    }
}

/// The name and the address of the Vault `name` names.
fn name_and_address(
    remote: &ResCurrentRemoteVault,
    name: &str,
) -> Result<(String, VaultAddress), ErrorRemoteVault> {
    let vault = remote.name_or_default(name)?;
    let address = remote.vault_or_default(vault.clone())?;

    Ok((vault, address))
}

/// The account the run acts as, when the Workspace has one.
fn bound_account(
    current: &ResCurrentAccount,
    held: &Workspace,
    answered: &mut Held,
) -> Option<Account> {
    match current.must_bind() {
        Ok(name) => match account_named(&name, Some(held), None) {
            Ok(account) => Some(account),
            Err(error) => {
                answered.account = Some(Routable::to_chain(error));

                None
            }
        },
        Err(error) => {
            answered.account = Some(Routable::to_chain(error));

            None
        }
    }
}

/// The failure a refusal the library answered with is told as.
fn refusal_state(refusal: RetrackRefusal, file: &str, written: &str, answered: &mut Held) -> Next {
    match refusal {
        RetrackRefusal::NotRecorded { .. } => {
            ErrorRetrackFile::new(file, RetrackFileError::NotRecorded).into()
        }
        RetrackRefusal::Nothing { path } => ErrorRetrackNothing { path }.into(),
        RetrackRefusal::Modified { path } => ErrorRetrackModified { path }.into(),
        RetrackRefusal::Merging { path, variant } => ErrorRetrackMerging { path, variant }.into(),
        RetrackRefusal::TreeDirty { path } => ErrorRetrackTreeDirty { path }.into(),
        RetrackRefusal::Jump { path } => ErrorRetrackSwitch { path }.into(),
        RetrackRefusal::Target(kind) => ErrorRetrackTarget {
            target: written.to_owned(),
            kind: target_kind(kind),
        }
        .into(),
        RetrackRefusal::Entry(kind) => ErrorRetrackEntry {
            entry: written.to_owned(),
            kind: entry_kind(kind),
        }
        .into(),
        RetrackRefusal::Action(error) => Routable::to_chain(error),
        RetrackRefusal::Failed(error) => write_error_state(error, answered),
    }
}

/// What a write the library stopped on is told as.
///
/// The categories are the library's, and the words and exit codes are the command's: a store, an
/// index or a Vault the run had to read is named as the missing thing it is, and everything else is
/// the failure the run ends on, which is what it was told before the workflow moved.
fn write_error_state(error: WriteError, answered: &mut Held) -> Next {
    let why = error.to_string();

    match error {
        WriteError::NoStore => ErrorRetrackFailed {
            cause: t!("retrack.err_no_store").trim().to_owned(),
        }
        .into(),
        WriteError::NoIndex => ErrorRetrackFailed {
            cause: t!("retrack.err_no_index").trim().to_owned(),
        }
        .into(),
        WriteError::NoTrack => ErrorRetrackFailed {
            cause: t!("retrack.err_no_vault").trim().to_owned(),
        }
        .into(),
        // The Vault is one the run had to reach, and one the Workspace could not name is the
        // failure it would have been told had it been asked for before the work began.
        WriteError::NoVault => answered.vault.take().unwrap_or_else(|| {
            ErrorRetrackFailed {
                cause: t!("retrack.err_no_vault").trim().to_owned(),
            }
            .into()
        }),
        WriteError::NoAccount => answered
            .account
            .take()
            .unwrap_or_else(|| ErrorRetrackFailed { cause: why.clone() }.into()),
        WriteError::Offline => ErrorOffline.into(),
        WriteError::NotFound { .. } => ErrorLayoutMissing.into(),
        WriteError::Layout { reason } => ErrorLayoutFailed::new(reason).into(),
        other => ErrorRetrackFailed {
            cause: other.to_string(),
        }
        .into(),
    }
}

/// The result a run that moved a record answers with.
fn moved_state(outcome: RetrackOutcome) -> Next {
    match outcome.record {
        Recorded::Path(moved) => ResultRetrack {
            path: moved.path,
            version: moved.version,
            number: moved.number,
            changed: moved.changed,
            switched: moved.switched,
            from: moved.from,
            to: moved.to,
            forked: moved.forked,
        }
        .into(),
        Recorded::Tree(tree) => ResultRetrackTree {
            path: tree.path,
            version: tree.version,
        }
        .into(),
    }
}

/// Says what the Vaults handed over while the run went on, in one line.
///
/// It is the line a fetch says, since what it describes is the fetch: a run that brought nothing is
/// one with nothing to report.
fn said_handed_over(took: &[(String, usize)]) {
    if took.is_empty() {
        return;
    }

    let count: usize = took.iter().map(|(_, count)| count).sum();
    let vaults: Vec<&str> = took.iter().map(|(name, _)| name.as_str()).collect();

    eprintln!(
        "{}",
        warn_line!(t!("fetch.brought", count = count, vaults = vaults.join(", ")).trim())
    );
}

/// The way a refusal of the warehouse names a version, in the command's own kinds.
fn target_kind(kind: TargetRefusal) -> RetrackTargetError {
    match kind {
        TargetRefusal::NotATarget => RetrackTargetError::NotATarget,
        TargetRefusal::DashNumber => RetrackTargetError::DashNumber,
        TargetRefusal::OutOfChain { path } => RetrackTargetError::OutOfChain { path },
        TargetRefusal::NotAVersion => RetrackTargetError::NotAVersion,
        TargetRefusal::AmbiguousHash { candidates } => {
            RetrackTargetError::AmbiguousHash { candidates }
        }
        TargetRefusal::NotAncestor { path } => RetrackTargetError::NotAncestor { path },
    }
}

/// The way a refusal of the warehouse names an entry, in the command's own kinds.
fn entry_kind(kind: EntryRefusal) -> RetrackEntryError {
    match kind {
        EntryRefusal::NoVaultTracked => RetrackEntryError::NoVault,
        EntryRefusal::NotUpstream { vault } => RetrackEntryError::NotUpstream { vault },
        EntryRefusal::UnknownUuid { uuid } => RetrackEntryError::UnknownUuid { uuid },
        EntryRefusal::SameUuid { uuid } => RetrackEntryError::SameUuid { uuid },
        EntryRefusal::Named { uuid, path } => RetrackEntryError::Named { uuid, path },
        EntryRefusal::NoVersion { uuid } => RetrackEntryError::NoVersion { uuid },
    }
}

/// The path the run named, read as one the Layout being worked in names.
///
/// Only a local path is taken. A `Uuid`, a hash and a remote name are refused by shape rather than
/// answered as a path that happens not to be there, since a run that wrote one meant the other
/// command that takes one. A name holds `@` often enough, `logo@2x.png`, that a plain one is
/// left to be a path: what is refused is the Vault-side marker and the one name a Vault's Layout
/// is kept under.
fn locate(root: &Path, cwd: &Path, given: &str) -> Result<LayoutPath, Next> {
    local_path(root, cwd, given).map_err(|refusal| {
        ErrorRetrackFile::new(
            given,
            match refusal {
                PathRefusal::NotLocal => RetrackFileError::NotLocal,
                PathRefusal::OutsideWorkspace => RetrackFileError::OutsideWorkspace,
                PathRefusal::InsideData => RetrackFileError::InsideData,
            },
        )
        .into()
    })
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

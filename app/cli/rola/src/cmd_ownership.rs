//! The `rola hold` and `rola giveup` commands: taking entries and letting them go, safely.
//!
//! `rola layout req-ownership` and `rola layout giveup-ownership` are the Vault's own doors to
//! ownership, and they are the whole of what either moves. What they cannot do is look at the work
//! this side is standing on: whether what is here is the version the Vault names, whether it has
//! been changed since, and whether the run is about to hand over something it meant to keep. That
//! is what these two are for.
//!
//! Each fetches the Vault's Layout first, since every gate is read against what the Vault holds
//! now, and each writes what the Vault agreed to into that copy afterwards, so a reading next
//! shows the ownership as it now is. What they move is every entry the names stand for — a file is
//! itself, a directory is every file under it, and `.` is the directory the run was made in — and
//! the gates are read for all of them before any ownership moves: a batch that cannot be done whole
//! is not half done, unless `--allow-partial` says the doable part is worth doing anyway.
//!
//! The gates themselves are the same for both, save for who is expected to hold the entry — and
//! `--force` moves past the version, never past the holder.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use librorolala::auth::Account;
use librorolala::daemon::{
    Ownership, action_fetch_layout, action_giveup_ownership, action_request_ownership,
};
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::protocol::VaultAddress;
use librorolala::tree_analyze::{TreeDiff, tree_diff, walk};
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResForce, ResOffline, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{strip_written, typing_flag};
use crate::error::ErrorOffline;
use crate::exit_codes::{EC_ERR_LAYOUT_ARGUMENT, EC_ERR_LAYOUT_OWNERSHIP, EC_HELP};
use crate::failure::failure;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutFailed, ErrorLayoutMissing, ErrorLayoutNotCached, readonly_layout_dir,
    set_cached_owner,
};

/// How alike two text files have to be to count as the same file moved, when nothing is said.
const ALIKE: f32 = 0.6;

/// Which way one entry's ownership is moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Own {
    /// Claimed for this account.
    Hold,
    /// Let go of.
    Giveup,
}

/// The word the command that moves ownership this way is known by.
const fn own_name(kind: Own) -> &'static str {
    match kind {
        Own::Hold => "hold",
        Own::Giveup => "giveup",
    }
}

/// The flags `rola hold` and `rola giveup` take.
#[derive(Pickable)]
struct OwnershipFlags {
    /// Move what can be moved when not everything named can be.
    #[arg(long)]
    allow_partial: Flag,
}

#[help(buffer)]
pub fn help_hold(_: EntryHold, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("hold.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryHold)]
pub fn desc_hold() -> Description {
    t!("hold.description").to_string().into()
}

/// Completes what `rola hold` can be given next.
#[completion(EntryHold)]
pub fn complete_hold(ctx: ShellContext) -> Suggest {
    complete_ownership(&ctx)
}

/// Completes what `rola giveup` can be given next.
#[completion(EntryGiveup)]
pub fn complete_giveup(ctx: ShellContext) -> Suggest {
    complete_ownership(&ctx)
}

/// The completion the two commands that move ownership share.
///
/// Every word that is not a flag names where a file sits, so the filesystem answers it; the one
/// flag says whether a batch that cannot be done whole may be done in part.
fn complete_ownership(ctx: &ShellContext) -> Suggest {
    if typing_flag(ctx) {
        return strip_written(
            ctx,
            suggest! {
                "--allow-partial": t!("hold.complete.allow_partial"),
            },
        );
    }

    Suggest::file_comp()
}

/// Claims entries for this run's account
///
/// Each `PATH` is where a file sits in the Layout being worked in; ownership is about the `Uuid`
/// that Layout names the path by, in the Vault the Layout tracks. A `PATH` that is a directory is
/// taken recursively — every file under it — and `.` is the directory the run was made in, so
/// `rola hold .` claims everything in the work that is not already claimed. Files that directory
/// brings in which the Layout does not name have no `Uuid` to be about, and are passed over.
///
/// That Vault's Layout is fetched first, since every gate below is read against what it holds now,
/// and every gate is read for every entry before any ownership moves:
///
/// - the entry must be one the Vault already holds: one it never took is nothing to claim;
/// - what is here must be the version the Vault names, and nothing in it may be unrecorded —
///   `--force` moves past this;
/// - the entry must be held by nobody: claiming what another account holds is not a claim, and
///   `--force` does not move this.
///
/// `--allow-partial` is what a run that knows some of them will not pass asks for: the ones that do
/// are claimed, the ones that do not are said, and the run ends unhappily either way.
///
/// # Errors
///
/// Renders [`ErrorOwnershipNoTrack`] when the Layout tracks no Vault, [`ErrorOwnershipName`] when a
/// name is not a file or directory inside the Workspace, and, unless `--allow-partial` is asked
/// for, whatever one of the gates refused — [`ErrorOwnershipPath`], [`ErrorOwnershipUpstream`],
/// [`ErrorOwnershipVersion`], [`ErrorOwnershipChanged`] or [`ErrorOwnershipTaken`] — for the first
/// one, or [`ErrorOwnershipRefused`] when several were refused at once.
#[command(node = "hold", entry = EntryHold)]
pub fn hold(args: EntryHold, force: &ResForce, offline: &ResOffline) -> Next {
    // What a claim writes is the Vault's own Layout, and the fetch before it is not the point of
    // the command but a step of it: there is no offline claim to make.
    if **offline {
        return ErrorOffline.into();
    }

    let picked = args
        .pick(&arg![OwnershipFlags])
        .pick_or_route(&arg![Vec<String>], || {
            ErrorOwnershipArgument {
                command: "hold".to_owned(),
            }
            .into()
        })
        .to_result();
    let (flags, paths) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    // Picking several cannot fail for want of them — an absent list is an empty one — so a run
    // that named nothing is told so here rather than reading as a run with nothing to do.
    if paths.is_empty() {
        return ErrorOwnershipArgument {
            command: "hold".to_owned(),
        }
        .into();
    }

    StateHold {
        paths,
        force: **force,
        allow_partial: matches!(flags.allow_partial, Flag::Active),
    }
    .into()
}

/// The state a claim starts in.
#[derive(Grouped)]
pub struct StateHold {
    /// Where the files sit in the Layout being worked in.
    paths: Vec<String>,
    /// Whether the version and the tree are to be gone past.
    force: bool,
    /// Whether what can be claimed is claimed when not everything can be.
    allow_partial: bool,
}

#[chain(routeify)]
pub fn handle_hold(
    state: StateHold,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let StateHold {
        paths,
        force,
        allow_partial,
    } = state;

    change(
        Own::Hold,
        &paths,
        force,
        allow_partial,
        workspace,
        remote,
        current,
    )
}

#[help(buffer)]
pub fn help_giveup(_: EntryGiveup, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("giveup.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryGiveup)]
pub fn desc_giveup() -> Description {
    t!("giveup.description").to_string().into()
}

/// Lets go of entries this run's account holds
///
/// Each `PATH` is where a file sits in the Layout being worked in; ownership is about the `Uuid`
/// that Layout names the path by, in the Vault the Layout tracks. A `PATH` that is a directory is
/// taken recursively — every file under it — and `.` is the directory the run was made in, so
/// `rola giveup .` lets go of everything in the work this account holds. Files that directory
/// brings in which the Layout does not name have no `Uuid` to be about, and are passed over.
///
/// That Vault's Layout is fetched first, since every gate below is read against what it holds now,
/// and every gate is read for every entry before any ownership moves:
///
/// - the entry must be one the Vault already holds: one it never took is nothing to let go of;
/// - what is here must be the version the Vault names, and nothing in it may be unrecorded —
///   `--force` moves past this;
/// - the entry must be held by this account: another account's is not this run's to let go of, and
///   one nobody holds is nothing to let go of at all. `--force` does not move this.
///
/// `--allow-partial` is what a run that knows some of them will not pass asks for: the ones that do
/// are let go of, the ones that do not are said, and the run ends unhappily either way.
///
/// # Errors
///
/// Renders [`ErrorOwnershipNoTrack`] when the Layout tracks no Vault, [`ErrorOwnershipName`] when a
/// name is not a file or directory inside the Workspace, and, unless `--allow-partial` is asked
/// for, whatever one of the gates refused — [`ErrorOwnershipPath`], [`ErrorOwnershipUpstream`],
/// [`ErrorOwnershipVersion`], [`ErrorOwnershipChanged`] or [`ErrorOwnershipNotHeld`] — for the
/// first one, or [`ErrorOwnershipRefused`] when several were refused at once.
#[command(node = "giveup", entry = EntryGiveup)]
pub fn giveup(args: EntryGiveup, force: &ResForce, offline: &ResOffline) -> Next {
    // What letting go writes is the Vault's own Layout, and the fetch before it is not the point
    // of the command but a step of it: there is no offline release to make.
    if **offline {
        return ErrorOffline.into();
    }

    let picked = args
        .pick(&arg![OwnershipFlags])
        .pick_or_route(&arg![Vec<String>], || {
            ErrorOwnershipArgument {
                command: "giveup".to_owned(),
            }
            .into()
        })
        .to_result();
    let (flags, paths) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    // Picking several cannot fail for want of them — an absent list is an empty one — so a run
    // that named nothing is told so here rather than reading as a run with nothing to do.
    if paths.is_empty() {
        return ErrorOwnershipArgument {
            command: "giveup".to_owned(),
        }
        .into();
    }

    StateGiveup {
        paths,
        force: **force,
        allow_partial: matches!(flags.allow_partial, Flag::Active),
    }
    .into()
}

/// The state a release starts in.
#[derive(Grouped)]
pub struct StateGiveup {
    /// Where the files sit in the Layout being worked in.
    paths: Vec<String>,
    /// Whether the version and the tree are to be gone past.
    force: bool,
    /// Whether what can be let go of is let go of when not everything can be.
    allow_partial: bool,
}

#[chain(routeify)]
pub fn handle_giveup(
    state: StateGiveup,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let StateGiveup {
        paths,
        force,
        allow_partial,
    } = state;

    change(
        Own::Giveup,
        &paths,
        force,
        allow_partial,
        workspace,
        remote,
        current,
    )
}

/// Moves the ownership of every entry the names stand for, once the gates have all been read.
///
/// `routeify` is what lets the two handlers above be one: what this returns is the same `Next` a
/// handler would, so a failure the framework knows — no Workspace, no Vault to reach, no account —
/// is carried out of here just as it would be out of either of them.
#[routeify]
fn change(
    kind: Own,
    names: &[String],
    force: bool,
    allow_partial: bool,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let layouts = held.layouts();
    let name = match layouts.current() {
        Ok(Some(name)) => name,
        Ok(None) => {
            return ErrorOwnershipArgument {
                command: own_name(kind).to_owned(),
            }
            .into();
        }
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };
    let layout = match layouts.get(&name) {
        Ok(Some(layout)) => layout,
        Ok(None) => return ErrorLayoutMissing.into(),
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    let Some(track) = (match layouts.track(&name) {
        Ok(track) => track,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    }) else {
        return ErrorOwnershipNoTrack.into();
    };

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return ErrorOwnershipFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    let vault_name = remote.get_ref().name_or_default(track)?;
    let target = remote.get_ref().vault_or_default(vault_name.clone())?;
    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    // Every gate is read against what the Vault holds now, so its copy is brought up to date first
    // — and that copy is what is written afterwards, so a reading agrees with the Vault.
    action_fetch_layout(held, &account, target.to_string(), vault_name.clone())?;

    let dir = readonly_layout_dir(held, &vault_name, VAULT_LAYOUT_NAME);
    if !dir.is_dir() {
        return ErrorLayoutNotCached {
            layout: VAULT_LAYOUT_NAME.to_owned(),
            vault: vault_name,
        }
        .into();
    }
    let copy = match Layout::open(&dir) {
        Ok(layout) => layout,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    let selected = match expanded(held, &cwd, names) {
        Ok(selected) => selected,
        Err(error) => return error.into(),
    };
    let diff = if force {
        None
    } else {
        match tree_diff(&layout, held, ALIKE) {
            Ok(diff) => Some(diff),
            Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
        }
    };

    let (wanted, mut failed, skipped) = plan(
        kind,
        force,
        &layout,
        &copy,
        &account_name,
        &selected,
        diff.as_ref(),
    );

    // A batch that cannot be done whole is not half done: every gate was read before any ownership
    // moved, and what did not pass is the whole of what the run is told about.
    if !failed.is_empty() && !allow_partial {
        if failed.len() == 1 {
            return refusal_state(failed.remove(0));
        }

        return ErrorOwnershipRefused {
            command: own_name(kind).to_owned(),
            failures: failed.iter().map(describe).collect(),
        }
        .into();
    }

    let (entries, refused) = act(kind, wanted, held, &account, &target, &vault_name);
    failed.extend(refused);

    ResultOwnership {
        kind,
        entries,
        skipped,
        failed: failed.iter().map(describe).collect(),
    }
    .into()
}

/// One path a run named, and whether it was named outright or brought in by a directory.
struct Named {
    /// Where it sits in the Layout.
    path: LayoutPath,
    /// Whether the run named this path itself rather than a directory over it.
    direct: bool,
}

/// One entry a run is about to move.
struct Wanted {
    /// Where it sits in the Layout.
    path: LayoutPath,
    /// The `Uuid` ownership is about.
    id: Uuid,
}

/// The paths the names stand for.
///
/// A name that is a file is that file; one that is a directory is every file under it, read the way
/// the tree reading reads the tree, so what the Workspace keeps for itself is left out. A name
/// that is both — one spelled twice, or a file under a directory named as well — is one path.
fn expanded(
    held: &Workspace,
    cwd: &Path,
    names: &[String],
) -> Result<Vec<Named>, ErrorOwnershipName> {
    let root = normalize(held.get_root());
    let mut paths: BTreeMap<LayoutPath, bool> = BTreeMap::new();
    let mut walked = None;

    for given in names {
        let disk = normalize(&resolve(cwd, given));

        let relative = disk.strip_prefix(&root).map_err(|_| ErrorOwnershipName {
            name: given.clone(),
        })?;

        if disk.is_file() {
            let path = LayoutPath::from_relative(relative).map_err(|_| ErrorOwnershipName {
                name: given.clone(),
            })?;

            paths.insert(path, true);
            continue;
        }

        if !disk.is_dir() {
            return Err(ErrorOwnershipName {
                name: given.clone(),
            });
        }

        // The walk is the whole of what a directory may bring in, so it is read once however many
        // directories are named — and only when one is.
        let found = walked.get_or_insert_with(|| walk(&root).0);
        let prefix = if relative.as_os_str().is_empty() {
            String::new()
        } else {
            LayoutPath::from_relative(relative)
                .map_err(|_| ErrorOwnershipName {
                    name: given.clone(),
                })?
                .as_str()
                .to_owned()
        };

        for file in found.iter() {
            if under(&prefix, &file.path) {
                paths.entry(file.path.clone()).or_insert(false);
            }
        }
    }

    Ok(paths
        .into_iter()
        .map(|(path, direct)| Named { path, direct })
        .collect())
}

/// Why an entry's ownership may not be moved.
enum Refusal {
    /// The Layout names nothing at the path.
    Path(String),
    /// The Vault's Layout names nothing for the `Uuid` the path is named by.
    Upstream(String),
    /// What is here is not the version the Vault names.
    Version(String),
    /// What is here has changes the Layout does not name.
    Changed(String),
    /// The entry is held by another account.
    Taken { path: String, owner: String },
    /// The entry is not this account's to let go of.
    NotHeld { path: String, owner: Option<String> },
    /// The exchange could not be made.
    Failed(String),
}

/// Reads every gate for every path, and says what may be moved and what may not.
///
/// Nothing is moved here: a batch is judged whole before any of it is done.
fn plan(
    kind: Own,
    force: bool,
    layout: &Layout,
    copy: &Layout,
    me: &str,
    named: &[Named],
    diff: Option<&TreeDiff>,
) -> (Vec<Wanted>, Vec<Refusal>, usize) {
    let mut wanted = Vec::new();
    let mut failed = Vec::new();
    let mut skipped = 0;

    for Named { path, direct } in named {
        // A file a directory brought in that the Layout does not name is not something to hold or
        // let go of; one named outright is a mistake worth being told about.
        let Some(id) = layout.id_of(path) else {
            if *direct {
                failed.push(Refusal::Path(path.as_str().to_owned()));
            } else {
                skipped += 1;
            }

            continue;
        };

        let Some(upstream) = copy.entry(id) else {
            failed.push(Refusal::Upstream(path.as_str().to_owned()));

            continue;
        };

        let settled = if force {
            Ok(())
        } else {
            diff.map_or(Ok(()), |diff| settled(path, layout, id, &upstream, diff))
        };

        match settled.and_then(|()| free(kind, path, &upstream, me)) {
            Ok(()) => wanted.push(Wanted {
                path: path.clone(),
                id,
            }),
            Err(refusal) => failed.push(refusal),
        }
    }

    (wanted, failed, skipped)
}

/// Reads the version and the tree: what is here has to be what the Vault names, and settled.
fn settled(
    path: &LayoutPath,
    layout: &Layout,
    id: Uuid,
    upstream: &MutableData,
    diff: &TreeDiff,
) -> Result<(), Refusal> {
    if layout.entry(id).map(|data| data.version()) != Some(upstream.version()) {
        return Err(Refusal::Version(path.as_str().to_owned()));
    }

    if diff.modified.contains(path) {
        return Err(Refusal::Changed(path.as_str().to_owned()));
    }

    Ok(())
}

/// Reads who holds it, which is the gate `--force` does not move.
fn free(kind: Own, path: &LayoutPath, upstream: &MutableData, me: &str) -> Result<(), Refusal> {
    let named = path.as_str().to_owned();

    match (kind, upstream.owner()) {
        (Own::Hold, None) => Ok(()),
        (Own::Hold, Some(owner)) => Err(Refusal::Taken {
            path: named,
            owner: owner.to_owned(),
        }),
        (Own::Giveup, Some(owner)) if owner == me => Ok(()),
        (Own::Giveup, owner) => Err(Refusal::NotHeld {
            path: named,
            owner: owner.map(str::to_owned),
        }),
    }
}

/// Moves the ownership of everything the gates allowed, and says what the Vault refused anyway.
fn act(
    kind: Own,
    wanted: Vec<Wanted>,
    held: &Workspace,
    account: &Account,
    target: &VaultAddress,
    vault_name: &str,
) -> (Vec<OwnershipItem>, Vec<Refusal>) {
    let mut entries = Vec::with_capacity(wanted.len());
    let mut failed = Vec::new();

    for item in wanted {
        match move_one(kind, item.id, &item.path, held, account, target, vault_name) {
            Ok(owner) => entries.push(OwnershipItem {
                path: item.path.as_str().to_owned(),
                uuid: item.id.to_string(),
                owner,
            }),
            Err(refusal) => failed.push(refusal),
        }
    }

    (entries, failed)
}

/// Asks the Vault to move one entry's ownership, and writes what it agrees to into the copy.
fn move_one(
    kind: Own,
    id: Uuid,
    path: &LayoutPath,
    held: &Workspace,
    account: &Account,
    target: &VaultAddress,
    vault_name: &str,
) -> Result<Option<String>, Refusal> {
    let uuid = id.to_string();
    let answer = match kind {
        Own::Hold => action_request_ownership(held, account, target.to_string(), uuid),
        Own::Giveup => action_giveup_ownership(held, account, target.to_string(), uuid),
    }
    .map_err(|error| Refusal::Failed(error.to_string()))?;

    let outcome: Ownership =
        serde_json::from_str(&answer).map_err(|error| Refusal::Failed(error.to_string()))?;

    match outcome {
        Ownership::Owner(owner) => {
            set_cached_owner(held, vault_name, id, owner.clone())
                .map_err(|error| Refusal::Failed(error.to_string()))?;

            Ok(owner)
        }
        Ownership::Missing => Err(Refusal::Upstream(path.as_str().to_owned())),
        Ownership::HeldBy(owner) => Err(Refusal::Taken {
            path: path.as_str().to_owned(),
            owner,
        }),
    }
}

/// What was refused, in the words the run is told it in.
fn describe(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Path(path) => format!("{path}: {}", t!("ownership.why_path").trim()),
        Refusal::Upstream(path) => format!("{path}: {}", t!("ownership.why_upstream").trim()),
        Refusal::Version(path) => format!("{path}: {}", t!("ownership.why_version").trim()),
        Refusal::Changed(path) => format!("{path}: {}", t!("ownership.why_changed").trim()),
        Refusal::Taken { path, owner } => {
            format!(
                "{path}: {}",
                t!("ownership.why_taken", owner = owner).trim()
            )
        }
        Refusal::NotHeld {
            path,
            owner: Some(owner),
        } => format!(
            "{path}: {}",
            t!("ownership.why_not_yours", owner = owner).trim()
        ),
        Refusal::NotHeld { path, owner: None } => {
            format!("{path}: {}", t!("ownership.why_nobody").trim())
        }
        Refusal::Failed(cause) => cause.clone(),
    }
}

/// Turns one refusal into what the run is told.
#[routeify]
fn refusal_state(refusal: Refusal) -> Next {
    match refusal {
        Refusal::Path(path) => ErrorOwnershipPath { path }.into(),
        Refusal::Upstream(path) => ErrorOwnershipUpstream { path }.into(),
        Refusal::Version(path) => ErrorOwnershipVersion { path }.into(),
        Refusal::Changed(path) => ErrorOwnershipChanged { path }.into(),
        Refusal::Taken { path, owner } => ErrorOwnershipTaken { path, owner }.into(),
        Refusal::NotHeld { path, owner } => ErrorOwnershipNotHeld { path, owner }.into(),
        Refusal::Failed(cause) => ErrorOwnershipFailed { cause }.into(),
    }
}

/// The path `name` names, made absolute against the directory the run was made in.
fn resolve(cwd: &Path, name: &str) -> PathBuf {
    if Path::new(name).is_absolute() {
        PathBuf::from(name)
    } else {
        cwd.join(name)
    }
}

/// `path` with its `.` dropped and its `..` climbed, worked out lexically rather than on disk.
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

/// Whether `path` sits under the directory whose prefix is `prefix`.
fn under(prefix: &str, path: &LayoutPath) -> bool {
    if prefix.is_empty() {
        return true;
    }

    path.as_str()
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Error: `rola hold` or `rola giveup` was given arguments it cannot use.
#[derive(Grouped)]
pub struct ErrorOwnershipArgument {
    /// Which of the two was run.
    pub command: String,
}

impl Failure for ErrorOwnershipArgument {
    fn name(&self) -> &'static str {
        "error_ownership_argument"
    }

    fn reason(&self) -> String {
        t!("ownership.err_argument", command = self.command)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipArgument);

#[renderer(buffer)]
pub fn render_error_ownership_argument(error: ErrorOwnershipArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_argument_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: a name is not a file or a directory inside the Workspace.
#[derive(Grouped)]
pub struct ErrorOwnershipName {
    /// What was named, or why the tree could not be read.
    pub name: String,
}

impl Failure for ErrorOwnershipName {
    fn name(&self) -> &'static str {
        "error_ownership_name"
    }

    fn reason(&self) -> String {
        t!("ownership.err_name", name = self.name)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipName);

#[renderer(buffer)]
pub fn render_error_ownership_name(error: ErrorOwnershipName, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_name_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: the Layout being worked in tracks no Vault.
#[derive(Grouped)]
pub struct ErrorOwnershipNoTrack;

impl Failure for ErrorOwnershipNoTrack {
    fn name(&self) -> &'static str {
        "error_ownership_no_track"
    }

    fn reason(&self) -> String {
        t!("ownership.err_no_track").trim().to_string()
    }
}

failure!(ErrorOwnershipNoTrack);

#[renderer(buffer)]
pub fn render_error_ownership_no_track(error: ErrorOwnershipNoTrack, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_no_track_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the Layout names nothing at the path given.
#[derive(Grouped)]
pub struct ErrorOwnershipPath {
    /// The path the Layout names nothing at.
    pub path: String,
}

impl Failure for ErrorOwnershipPath {
    fn name(&self) -> &'static str {
        "error_ownership_path"
    }

    fn reason(&self) -> String {
        t!("ownership.err_path", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipPath);

#[renderer(buffer)]
pub fn render_error_ownership_path(error: ErrorOwnershipPath, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_path_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the Vault's Layout names nothing for the `Uuid` the path is named by.
#[derive(Grouped)]
pub struct ErrorOwnershipUpstream {
    /// The path the Vault names nothing for.
    pub path: String,
}

impl Failure for ErrorOwnershipUpstream {
    fn name(&self) -> &'static str {
        "error_ownership_upstream"
    }

    fn reason(&self) -> String {
        t!("ownership.err_upstream", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipUpstream);

#[renderer(buffer)]
pub fn render_error_ownership_upstream(error: ErrorOwnershipUpstream, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_upstream_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: what is here is not the version the Vault's Layout names.
#[derive(Grouped)]
pub struct ErrorOwnershipVersion {
    /// The path that is at another version.
    pub path: String,
}

impl Failure for ErrorOwnershipVersion {
    fn name(&self) -> &'static str {
        "error_ownership_version"
    }

    fn reason(&self) -> String {
        t!("ownership.err_version", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipVersion);

#[renderer(buffer)]
pub fn render_error_ownership_version(error: ErrorOwnershipVersion, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_version_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: what is here has changes the Layout does not name.
#[derive(Grouped)]
pub struct ErrorOwnershipChanged {
    /// The path that was changed.
    pub path: String,
}

impl Failure for ErrorOwnershipChanged {
    fn name(&self) -> &'static str {
        "error_ownership_changed"
    }

    fn reason(&self) -> String {
        t!("ownership.err_changed", path = self.path)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipChanged);

#[renderer(buffer)]
pub fn render_error_ownership_changed(error: ErrorOwnershipChanged, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_changed_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the entry is held by another account, so it is not free to claim.
#[derive(Grouped)]
pub struct ErrorOwnershipTaken {
    /// The path that is held.
    pub path: String,
    /// The account that holds it.
    pub owner: String,
}

impl Failure for ErrorOwnershipTaken {
    fn name(&self) -> &'static str {
        "error_ownership_taken"
    }

    fn reason(&self) -> String {
        t!("ownership.err_held", path = self.path, owner = self.owner)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipTaken);

#[renderer(buffer)]
pub fn render_error_ownership_taken(error: ErrorOwnershipTaken, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_held_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the entry is not this account's to let go of.
#[derive(Grouped)]
pub struct ErrorOwnershipNotHeld {
    /// The path that is not this account's.
    pub path: String,
    /// The account that holds it, or nothing when nobody does.
    pub owner: Option<String>,
}

impl Failure for ErrorOwnershipNotHeld {
    fn name(&self) -> &'static str {
        "error_ownership_not_held"
    }

    fn reason(&self) -> String {
        self.owner.as_ref().map_or_else(
            || {
                t!("ownership.err_nobody", path = self.path)
                    .trim()
                    .to_string()
            },
            |owner| {
                t!("ownership.err_not_yours", path = self.path, owner = owner)
                    .trim()
                    .to_string()
            },
        )
    }
}

failure!(ErrorOwnershipNotHeld);

#[renderer(buffer)]
pub fn render_error_ownership_not_held(error: ErrorOwnershipNotHeld, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_not_held_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: several of the entries named cannot have their ownership moved.
#[derive(Grouped)]
pub struct ErrorOwnershipRefused {
    /// Which command was run.
    pub command: String,
    /// What could not be moved, one line each.
    pub failures: Vec<String>,
}

impl Failure for ErrorOwnershipRefused {
    fn name(&self) -> &'static str {
        "error_ownership_refused"
    }

    fn reason(&self) -> String {
        t!(
            "ownership.err_refused",
            command = self.command,
            count = self.failures.len()
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorOwnershipRefused);

#[renderer(buffer)]
pub fn render_error_ownership_refused(error: ErrorOwnershipRefused, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));

    for failure in &error.failures {
        r_eprintln!("{}", err_line!(failure));
    }

    r_eprintln!("{}", help_line!(t!("ownership.err_refused_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// Error: the exchange could not be made.
#[derive(Grouped)]
pub struct ErrorOwnershipFailed {
    /// What the exchange failed with.
    pub cause: String,
}

impl Failure for ErrorOwnershipFailed {
    fn name(&self) -> &'static str {
        "error_ownership_failed"
    }

    fn reason(&self) -> String {
        t!("ownership.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorOwnershipFailed);

#[renderer(buffer)]
pub fn render_error_ownership_failed(error: ErrorOwnershipFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("ownership.err_failed_help").trim()));
    ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
}

/// One entry whose ownership was moved.
#[derive(Serialize)]
pub struct OwnershipItem {
    /// Where the file sits in the Layout.
    path: String,
    /// The `Uuid` ownership was moved for.
    uuid: String,
    /// The account that holds it now, or nothing when nobody does.
    owner: Option<String>,
}

/// Result: the ownership of the entries named was moved.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultOwnership {
    /// Which way it moved.
    kind: Own,
    /// What moved, in path order.
    entries: Vec<OwnershipItem>,
    /// How many files a directory brought in that the Layout does not name.
    skipped: usize,
    /// What was not moved, one line each.
    failed: Vec<String>,
}

#[renderer(buffer)]
pub fn render_result_ownership(result: ResultOwnership, ec: &mut ResExitCode) {
    for item in &result.entries {
        let said = match result.kind {
            Own::Hold => t!("ownership.result_held", path = item.path),
            Own::Giveup => t!("ownership.result_given", path = item.path),
        };

        r_println!("{}", said.trim());
    }

    if result.skipped > 0 {
        r_println!("{}", t!("ownership.skipped", count = result.skipped).trim());
    }

    for failure in &result.failed {
        r_eprintln!("{}", err_line!(failure));
    }

    if !result.failed.is_empty() {
        ec.exit_code = EC_ERR_LAYOUT_OWNERSHIP;
    }
}

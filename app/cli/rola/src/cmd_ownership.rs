//! The `rola hold` and `rola giveup` commands: taking an entry and letting one go, safely.
//!
//! `rola layout req-ownership` and `rola layout giveup-ownership` are the Vault's own doors to
//! ownership, and they are the whole of what either moves. What they cannot do is look at the work
//! this side is standing on: whether what is here is the version the Vault names, whether it has
//! been changed since, and whether the run is about to hand over something it meant to keep. That
//! is what these two are for.
//!
//! Each fetches the Vault's Layout first, since every gate is read against what the Vault holds
//! now, and each writes what the Vault agreed to into that copy afterwards, so a reading next
//! shows the ownership as it now is. The gates themselves are the same for both, save for who is
//! expected to hold the entry — and `--force` moves past the version, never past the holder.

use librorolala::auth::Account;
use librorolala::daemon::{
    Ownership, action_fetch_layout, action_giveup_ownership, action_request_ownership,
};
use librorolala::layout::{Layout, LayoutPath, MutableData};
use librorolala::protocol::VaultAddress;
use librorolala::tree_analyze::tree_diff;
use librorolala::workspace::Workspace;
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::{EntryPicker, Pickable, value::Flag},
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
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
    /// Move the ownership even when what is here is not the version the Vault names.
    #[arg(long)]
    force: Flag,
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

/// Claims an entry for this run's account
///
/// `PATH` is where the file sits in the Layout being worked in; the `Uuid` ownership is about is
/// the one that Layout names the path by. The Vault the Layout tracks is the one reached, and its
/// Layout is fetched first, since every gate is read against what it holds now:
///
/// - the entry must be one the Vault already holds, since one it never took is nothing to claim;
/// - what is here must be the version the Vault names, and nothing in it may be unrecorded —
///   `--force` moves past this;
/// - the entry must be held by nobody: claiming what another account holds is not a claim, and
///   `--force` does not move this.
///
/// What the Vault agrees to is written into the fetched copy as well, so a reading afterwards
/// shows the new holder.
///
/// # Errors
///
/// Renders [`ErrorOwnershipNoTrack`] when the Layout tracks no Vault, [`ErrorOwnershipPath`] when
/// the Layout names nothing at that path, [`ErrorOwnershipUpstream`] when the Vault holds no such
/// entry, [`ErrorOwnershipVersion`] or [`ErrorOwnershipChanged`] when what is here is not what the
/// Vault names, [`ErrorOwnershipTaken`] when another account holds it, [`ErrorOwnershipFailed`]
/// when the exchange could not be made, and the exchange's own failures otherwise.
#[command(node = "hold", entry = EntryHold)]
pub fn hold(args: EntryHold) -> Next {
    let picked = args
        .pick(&arg![OwnershipFlags])
        .pick_or_route(&arg![String], || {
            ErrorOwnershipArgument {
                command: "hold".to_owned(),
            }
            .into()
        })
        .to_result();
    let (flags, path) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateHold {
        path,
        force: matches!(flags.force, Flag::Active),
    }
    .into()
}

/// The state a claim starts in.
#[derive(Grouped)]
pub struct StateHold {
    /// Where the file sits in the Layout being worked in.
    path: String,
    /// Whether the version and the tree are to be gone past.
    force: bool,
}

#[chain(routeify)]
pub fn handle_hold(
    state: StateHold,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let StateHold { path, force } = state;

    change(Own::Hold, &path, force, workspace, remote, current)
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

/// Lets go of an entry this run's account holds
///
/// `PATH` is where the file sits in the Layout being worked in; the `Uuid` ownership is about is
/// the one that Layout names the path by. The Vault the Layout tracks is the one reached, and its
/// Layout is fetched first, since every gate is read against what it holds now:
///
/// - the entry must be one the Vault already holds, since one it never took is nothing to let go
///   of;
/// - what is here must be the version the Vault names, and nothing in it may be unrecorded —
///   `--force` moves past this;
/// - the entry must be held by this account: another account's is not this run's to let go of,
///   and one nobody holds is nothing to let go of at all. `--force` does not move this.
///
/// What the Vault agrees to is written into the fetched copy as well, so a reading afterwards
/// shows the entry as held by nobody.
///
/// # Errors
///
/// Renders [`ErrorOwnershipNoTrack`] when the Layout tracks no Vault, [`ErrorOwnershipPath`] when
/// the Layout names nothing at that path, [`ErrorOwnershipUpstream`] when the Vault holds no such
/// entry, [`ErrorOwnershipVersion`] or [`ErrorOwnershipChanged`] when what is here is not what the
/// Vault names, [`ErrorOwnershipNotHeld`] when the entry is another account's or nobody's,
/// [`ErrorOwnershipFailed`] when the exchange could not be made, and the exchange's own failures
/// otherwise.
#[command(node = "giveup", entry = EntryGiveup)]
pub fn giveup(args: EntryGiveup) -> Next {
    let picked = args
        .pick(&arg![OwnershipFlags])
        .pick_or_route(&arg![String], || {
            ErrorOwnershipArgument {
                command: "giveup".to_owned(),
            }
            .into()
        })
        .to_result();
    let (flags, path) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    StateGiveup {
        path,
        force: matches!(flags.force, Flag::Active),
    }
    .into()
}

/// The state a release starts in.
#[derive(Grouped)]
pub struct StateGiveup {
    /// Where the file sits in the Layout being worked in.
    path: String,
    /// Whether the version and the tree are to be gone past.
    force: bool,
}

#[chain(routeify)]
pub fn handle_giveup(
    state: StateGiveup,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    let StateGiveup { path, force } = state;

    change(Own::Giveup, &path, force, workspace, remote, current)
}

/// Moves one entry's ownership, once every gate the Vault's own Layout can answer has been read.
///
/// `routeify` is what lets the two handlers above be one: what this returns is the same `Next` a
/// handler would, so a failure the framework knows — no Workspace, no Vault to reach, no account —
/// is carried out of here just as it would be out of either of them.
#[routeify]
fn change(
    kind: Own,
    path: &str,
    force: bool,
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

    let Ok(path) = LayoutPath::new(path) else {
        return ErrorOwnershipArgument {
            command: own_name(kind).to_owned(),
        }
        .into();
    };
    let Some(id) = layout.id_of(&path) else {
        return ErrorOwnershipPath {
            path: path.as_str().to_owned(),
        }
        .into();
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

    // A `Uuid` the Vault does not name is one it never took: there is nothing there to hold, and
    // nothing to let go of either.
    let Some(upstream) = copy.entry(id) else {
        return ErrorOwnershipUpstream {
            path: path.as_str().to_owned(),
        }
        .into();
    };

    // What is here has to be what the Vault names, and has to be settled. A run that hands an entry
    // over is confirming the version it is handing over, not one it happens to have; the same is
    // true of claiming one, since what is claimed is then what may be pushed.
    //
    // Who holds it is the one gate `--force` does not move: claiming what another account holds is
    // not a claim, and letting go of what is not this account's — or of what nobody holds — is not
    // a release.
    let allowed = if force {
        free(kind, &path, &upstream, &account_name)
    } else {
        settled(&path, &layout, held, id, &upstream)
            .and_then(|()| free(kind, &path, &upstream, &account_name))
    };

    if let Err(refusal) = allowed {
        return match refusal {
            Refusal::Version(path) => ErrorOwnershipVersion { path }.into(),
            Refusal::Changed(path) => ErrorOwnershipChanged { path }.into(),
            Refusal::Unreadable(cause) => ErrorLayoutFailed::new(cause).into(),
            Refusal::Taken { path, owner } => ErrorOwnershipTaken { path, owner }.into(),
            Refusal::NotHeld { path, owner } => ErrorOwnershipNotHeld { path, owner }.into(),
        };
    }

    move_ownership(kind, id, &path, held, &account, &target, &vault_name)
}

/// Why an entry's ownership may not be moved.
enum Refusal {
    /// What is here is not the version the Vault names.
    Version(String),
    /// What is here has changes the Layout does not name.
    Changed(String),
    /// The tree could not be read.
    Unreadable(String),
    /// The entry is held by another account.
    Taken { path: String, owner: String },
    /// The entry is not this account's to let go of.
    NotHeld { path: String, owner: Option<String> },
}

/// Reads the version and the tree: what is here has to be what the Vault names, and settled.
fn settled(
    path: &LayoutPath,
    layout: &Layout,
    held: &Workspace,
    id: Uuid,
    upstream: &MutableData,
) -> Result<(), Refusal> {
    if layout.entry(id).map(|data| data.version()) != Some(upstream.version()) {
        return Err(Refusal::Version(path.as_str().to_owned()));
    }

    match tree_diff(layout, held, ALIKE) {
        Ok(diff) if diff.modified.contains(path) => Err(Refusal::Changed(path.as_str().to_owned())),
        Ok(_) => Ok(()),
        Err(error) => Err(Refusal::Unreadable(error.to_string())),
    }
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

/// Asks the Vault to move the ownership, and writes what it agrees to into the fetched copy.
#[routeify]
fn move_ownership(
    kind: Own,
    id: Uuid,
    path: &LayoutPath,
    held: &Workspace,
    account: &Account,
    target: &VaultAddress,
    vault_name: &str,
) -> Next {
    let uuid = id.to_string();
    let answer = match kind {
        Own::Hold => action_request_ownership(held, account, target.to_string(), uuid.clone())?,
        Own::Giveup => action_giveup_ownership(held, account, target.to_string(), uuid.clone())?,
    };
    let outcome: Ownership = match serde_json::from_str(&answer) {
        Ok(outcome) => outcome,
        Err(error) => {
            return ErrorOwnershipFailed {
                cause: error.to_string(),
            }
            .into();
        }
    };

    match outcome {
        Ownership::Owner(owner) => {
            if let Err(error) = set_cached_owner(held, vault_name, id, owner.clone()) {
                return ErrorOwnershipFailed {
                    cause: error.to_string(),
                }
                .into();
            }

            ResultOwnership {
                path: path.as_str().to_owned(),
                uuid,
                kind,
                owner,
            }
            .into()
        }
        Ownership::Missing => ErrorOwnershipUpstream {
            path: path.as_str().to_owned(),
        }
        .into(),
        Ownership::HeldBy(owner) => ErrorOwnershipTaken {
            path: path.as_str().to_owned(),
            owner,
        }
        .into(),
    }
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

/// Result: an entry's ownership was moved.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultOwnership {
    /// Where the file sits in the Layout.
    path: String,
    /// The `Uuid` ownership was moved for.
    uuid: String,
    /// Which way it moved.
    kind: Own,
    /// The account that holds it now, or nothing when nobody does.
    owner: Option<String>,
}

#[renderer(buffer)]
pub fn render_result_ownership(result: ResultOwnership) {
    let said = match result.kind {
        Own::Hold => t!("ownership.result_held", path = result.path),
        Own::Giveup => t!("ownership.result_given", path = result.path),
    };

    r_println!("{}", said.trim());
}

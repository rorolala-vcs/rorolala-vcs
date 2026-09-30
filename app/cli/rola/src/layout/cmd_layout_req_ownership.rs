//! The `rola layout req-ownership` command: taking an entry from the Vault's Layout.
//!
//! An entry is held by one account, and the Vault's Layout is where that is written down. Asking
//! for it is asking the Vault to name the run's account as the holder, which it does when nobody
//! else holds the entry. What the Workspace keeps as a copy is changed to match, so a copy read
//! afterwards agrees with the Vault.

use librorolala::daemon::{Ownership, action_request_ownership};
use mingling::{
    Grouped, LazyRes, StructuralData,
    macros::{
        arg, buffer, chain, command, help, metadata, r_eprintln, r_println, renderer, routeify,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;
use uuid::Uuid;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::EC_HELP;
use crate::keys::account_named;
use crate::layout::{
    ErrorLayoutArgument, ErrorLayoutFailed, ErrorOwnershipHeld, ErrorOwnershipMissing,
    set_cached_owner, vault_and_uuids,
};

#[help(buffer)]
pub fn help_layout_req_ownership(_: EntryLayoutReqOwnership, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_req_ownership.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutReqOwnership)]
pub fn desc_layout_req_ownership() -> Description {
    t!("cmd_layout_req_ownership.description")
        .to_string()
        .into()
}

/// Asks the Vault to name this run's account as an entry's holder
///
/// `VAULT` names the Vault to reach, as [`rola layout fetch`](crate::layout::cmd_layout_fetch)
/// reads it: a name the Workspace has bound. Naming none reaches for the one the Workspace reaches
/// for by default. Each `UUID` is the `Uuid` of an entry to take, and they are taken in the order
/// they are named.
///
/// An entry nobody holds is named as this account's. One another account holds is left alone, and
/// the run is told who holds it — where that happens nothing after it is taken, so a run that names
/// several stops at the first one it cannot have. The copy fetched here, when there is one, is
/// changed to agree with the Vault; a run that never fetched is not refused for it.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries
/// it out: the run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]), the
/// Workspace has no Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or as one
/// no scope holds ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), the arguments name no entry
/// ([`ErrorLayoutArgument`]), the Vault holds no such entry ([`ErrorOwnershipMissing`]), another
/// account holds it ([`ErrorOwnershipHeld`]), or the exchange itself failed ([`ActionError`]).
///
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "layout.req-ownership", entry = EntryLayoutReqOwnership)]
pub fn layout_req_ownership(args: EntryLayoutReqOwnership) -> Next {
    let words: Vec<String> = args.pick(&arg![Vec<String>]).unwrap_or_default();

    let Some((vault, ids)) = vault_and_uuids(&words) else {
        return ErrorLayoutArgument.into();
    };

    StateLayoutReqOwnership { vault, ids }.into()
}

/// The state of asking for entries.
#[derive(Grouped)]
pub struct StateLayoutReqOwnership {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
    /// The entries to take, in the order they were named.
    ids: Vec<Uuid>,
}

#[chain(routeify)]
pub fn handle_layout_req_ownership(
    state: StateLayoutReqOwnership,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    let name = remote
        .get_ref()
        .name_or_default(state.vault.unwrap_or_default())?;
    let target = remote.get_ref().vault_or_default(name.clone())?;

    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    let mut entries = Vec::with_capacity(state.ids.len());

    for id in state.ids {
        let uuid = id.to_string();

        // What crosses back is the outcome as text, since an action's output is what the C ABI hands
        // back; what a run was told is read here, where the words it is said in are.
        let outcome = action_request_ownership(held, &account, target.to_string(), uuid.clone())?;
        let outcome: Ownership = match serde_json::from_str(&outcome) {
            Ok(outcome) => outcome,
            Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
        };

        match outcome {
            Ownership::Owner(owner) => {
                // What the Vault agreed to is written into the copy as well, so reading it next does
                // not show an owner the Vault no longer has. A copy that cannot be written is the
                // one failure here: the change itself is already made.
                if let Err(error) = set_cached_owner(held, &name, id, owner.clone()) {
                    return ErrorLayoutFailed::new(error.to_string()).into();
                }

                entries.push(OwnedItem { uuid, owner });
            }
            Ownership::Missing => return ErrorOwnershipMissing { uuid }.into(),
            Ownership::HeldBy(owner) => return ErrorOwnershipHeld { uuid, owner }.into(),
        }
    }

    ResultLayoutOwned { entries }.into()
}

/// One entry that was taken.
#[derive(Serialize)]
pub struct OwnedItem {
    /// The `Uuid` that was taken.
    uuid: String,
    /// The account that now holds it.
    owner: Option<String>,
}

/// Result: the entries a run asked for were named to its account.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutOwned {
    /// Each entry that was taken, in the order the `Uuid`s were named.
    entries: Vec<OwnedItem>,
}

#[renderer(buffer)]
pub fn render_result_layout_owned(result: ResultLayoutOwned) {
    for entry in &result.entries {
        let said = entry.owner.as_ref().map_or_else(
            || {
                t!(
                    "cmd_layout_req_ownership.result_taken",
                    uuid = entry.uuid.as_str()
                )
            },
            |owner| {
                t!(
                    "cmd_layout_req_ownership.result_owned",
                    uuid = entry.uuid.as_str(),
                    owner = owner
                )
            },
        );

        r_println!("{}", said.trim());
    }
}

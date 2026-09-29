//! The `rola layout giveup-ownership` command: letting go of an entry.
//!
//! The other half of [`req-ownership`](crate::layout::cmd_layout_req_ownership): an account that is
//! done with an entry asks the Vault to name nobody, which it does when this account holds it. What
//! the Workspace keeps as a copy is changed to match.

use librorolala::daemon::{Ownership, action_giveup_ownership};
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
    set_cached_owner, vault_and_uuid,
};

#[help(buffer)]
pub fn help_layout_giveup_ownership(_: EntryLayoutGiveupOwnership, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_giveup_ownership.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutGiveupOwnership)]
pub fn desc_layout_giveup_ownership() -> Description {
    t!("cmd_layout_giveup_ownership.description")
        .to_string()
        .into()
}

/// Asks the Vault to name nobody as an entry's holder
///
/// `VAULT` names the Vault to reach, as [`rola layout req-ownership`](crate::layout::cmd_layout_req_ownership)
/// reads it. Naming none reaches for the one the Workspace reaches for by default. `UUID` is the
/// `Uuid` of the entry to let go of.
///
/// An entry this account holds is named as nobody's. One another account holds is left alone, and
/// the run is told who holds it. One nobody holds is already what was asked for. The copy fetched
/// here, when there is one, is changed to agree with the Vault; a run that never fetched is not
/// refused for it.
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
#[command(node = "layout.giveup-ownership", entry = EntryLayoutGiveupOwnership)]
pub fn layout_giveup_ownership(args: EntryLayoutGiveupOwnership) -> Next {
    let words: Vec<String> = args.pick(&arg![Vec<String>]).unwrap_or_default();

    let Some((vault, id)) = vault_and_uuid(&words) else {
        return ErrorLayoutArgument.into();
    };

    StateLayoutGiveupOwnership { vault, id }.into()
}

/// The state of letting go of an entry.
#[derive(Grouped)]
pub struct StateLayoutGiveupOwnership {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
    /// The entry to let go of.
    id: Uuid,
}

#[chain(routeify)]
pub fn handle_layout_giveup_ownership(
    state: StateLayoutGiveupOwnership,
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

    let uuid = state.id.to_string();

    // What crosses back is the outcome as text, since an action's output is what the C ABI hands
    // back; what a run was told is read here, where the words it is said in are.
    let outcome = action_giveup_ownership(held, &account, target.to_string(), uuid.clone())?;
    let outcome: Ownership = match serde_json::from_str(&outcome) {
        Ok(outcome) => outcome,
        Err(error) => return ErrorLayoutFailed::new(error.to_string()).into(),
    };

    match outcome {
        Ownership::Owner(_) => {
            // What the Vault agreed to is written into the copy as well, so reading it next does
            // not show an owner the Vault no longer has. A copy that cannot be written is the
            // one failure here: the change itself is already made.
            if let Err(error) = set_cached_owner(held, &name, state.id, None) {
                return ErrorLayoutFailed::new(error.to_string()).into();
            }

            ResultLayoutGivenUp { uuid }.into()
        }
        Ownership::Missing => ErrorOwnershipMissing { uuid }.into(),
        Ownership::HeldBy(owner) => ErrorOwnershipHeld { uuid, owner }.into(),
    }
}

/// Result: an entry was let go of.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutGivenUp {
    /// The entry that was let go of.
    uuid: String,
}

#[renderer(buffer)]
pub fn render_result_layout_given_up(result: ResultLayoutGivenUp) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_giveup_ownership.result_given_up",
            uuid = result.uuid
        )
        .trim()
    );
}

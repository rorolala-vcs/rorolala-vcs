//! The `rola vault handshake` command: speak the handshake action to a Vault.
//!
//! It is the handshake on its own, with none of the work in the way: the Vault is reached
//! where the Workspace says it answers, or at an address the caller gives, and the action runs
//! as the account the work acts as, so a daemon can be checked from anywhere a Workspace and a
//! key are.

use librorolala::daemon::action_handshake;
use mingling::{
    Grouped, LazyRes, ShellContext, StructuralData, Suggest, Wrap,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::EC_HELP;
use crate::keys::account_named;

#[help(buffer)]
pub fn help_vault_handshake(_: EntryVaultHandshake, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("vault_handshake.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryVaultHandshake)]
pub fn desc_vault_handshake() -> Description {
    t!("vault_handshake.cmd_vault_handshake_description")
        .to_string()
        .into()
}

/// Speaks the handshake action to a Vault, and prints what it answers.
///
/// `VAULT` names the Vault to reach: a name the Workspace has [bound](crate::vault::cmd_vault), or an
/// ip and a port. Naming none reaches for the one the Workspace
/// [reaches for by default](crate::vault::cmd_vault_set_default). The action runs as the
/// account the work acts as — the one `rola account` names, which is the same choice every
/// other command makes — and what it introduces itself with is that account's name, so the
/// call needs nothing else.
///
/// The exchange is between the work and its remote, so it is spoken from inside a Workspace:
/// that is what says which Vault is reached, and whose keys the account comes from.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify`
/// carries it out: the run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]),
/// the Workspace has no Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or
/// as one no scope holds ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), or the exchange itself
/// failed ([`ActionError`]).
///
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "vault.handshake")]
pub fn vault_handshake(args: EntryVaultHandshake) -> StateVaultHandshake {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what
    // lets the Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateVaultHandshake::from(named)
}

/// The state of speaking the handshake to a Vault.
///
/// Which Vault is reached is the whole of what is said: naming none reaches for the one the
/// Workspace reaches for by default.
#[derive(Grouped, Wrap)]
pub struct StateVaultHandshake {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_vault_handshake(
    state: StateVaultHandshake,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    // Everything below works through the Workspace, so it is asked once, here, and taken for
    // granted after: `?` is `routeify`'s, and a run that is inside no Workspace leaves through
    // it. `held` is the copy the exchange is spoken from, and the same one the action is
    // handed, so both sides mean the same place by it.
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run
    // did not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    // `?` here is `routeify`'s: a run with nothing to reach for leaves through it.
    let target = remote
        .get_ref()
        .vault_or_default(state.vault.unwrap_or_default())?;

    // `?` here is `routeify`'s as well: the account the work acts as is the resource's to
    // hand over, and a run that acts as none leaves through it.
    let name = current.get_ref().must_bind()?;

    // No Vault is held: the one being reached for is elsewhere, so a key kept in a local
    // Vault is not this run's to act as. `?` here is `routeify`'s once more: a name no scope
    // holds is the lookup's own to report.
    let account = account_named(&name, Some(held), None)?;

    // What the Workspace side holds and sends is who it is, so what is printed is the daemon
    // greeting the account this runs as.
    let input = account.name();

    // The daemon is dialled at the address the link names, and the link is handed over whole:
    // which Vault under the daemon is asked for travels with the request, so passing only the
    // authority would reach whichever Vault the daemon serves rather than the one named here.
    let output = action_handshake(held, &account, target.to_string(), input)?;

    ResultHandshake { output }.into()
}

/// Completes what `rola vault handshake` can be given next.
///
/// What can be reached by name is what the Workspace has bound, which is the same set the
/// command itself resolves; an address is not, since there is nothing here that knows which
/// ones are worth offering.
#[completion(EntryVaultHandshake)]
pub fn complete_vault_handshake(
    ctx: ShellContext,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if ctx.current_word.starts_with('-') {
        return suggest!();
    }

    let Ok(names) = remote.get_ref().names() else {
        return suggest!();
    };

    let mut names: Vec<String> = names.into_iter().map(str::to_string).collect();
    names.sort();
    names.retain(|name| name.starts_with(&ctx.current_word));

    suggest! { names }
}

/// Result: the daemon answered.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultHandshake {
    /// What the daemon answered.
    output: String,
}

#[renderer(buffer)]
pub fn render_result_handshake(result: ResultHandshake) {
    r_println!("{}", result.output);
}

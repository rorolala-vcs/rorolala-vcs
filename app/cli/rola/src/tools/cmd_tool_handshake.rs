//! The `rola tool-handshake` command: speak the handshake action to a Vault.
//!
//! It is the handshake on its own, with none of the work in the way: the Vault is reached
//! where the Workspace says it answers, or at an address the caller gives, and the action runs
//! as the account the work acts as, so a daemon can be checked from anywhere a Workspace and a
//! key are.

use librorolala::daemon::action_handshake;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest,
    macros::{
        arg, buffer, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::EC_HELP;
use crate::keys::account_named;

#[help(buffer)]
pub fn help_tool_handshake(_: EntryToolHandshake, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool_handshake.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryToolHandshake)]
pub fn desc_tool_handshake() -> Description {
    t!("tool_handshake.cmd_tool_handshake_description")
        .to_string()
        .into()
}

/// Speaks the handshake action to a Vault, and prints what it answers.
///
/// `VAULT` names the Vault to reach: a name the Workspace has [bound](crate::cmd_vault), or an
/// ip and a port. Naming none reaches for the one the Workspace
/// [reaches for by default](crate::cmd_vault::vault_set_default). The action runs as the
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
#[command(node = "tool-handshake", routeify)]
pub fn tool_handshake(
    args: EntryToolHandshake,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    // Everything below works through the Workspace, so it is asked once, here, and taken for
    // granted after.
    workspace.get_ref().check()?;

    // Picking cannot fail: a positional that is absent is `None`, and naming none is what
    // lets the Workspace's own choice be the one that is reached for. `?` here is
    // `routeify`'s: a run with nothing to reach for leaves through it.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();
    let target = remote
        .get_ref()
        .vault_or_default(named.unwrap_or_default())?;

    // `?` here is `routeify`'s as well: the account the work acts as is the resource's to
    // hand over, and a run that acts as none leaves through it.
    let name = current.get_ref().must_bind()?;

    // No Vault is held: the one being reached for is elsewhere, so a key kept in a local
    // Vault is not this run's to act as. `?` here is `routeify`'s once more: a name no scope
    // holds is the lookup's own to report.
    let account = account_named(&name, workspace.get_ref().as_ref(), None)?;

    // What the Workspace side holds and sends is who it is, so what is printed is the daemon
    // greeting the account this runs as.
    let input = account.name();

    // The daemon is dialled at the address the link names. Which Vault under it is asked for is
    // not part of that yet: the request carries the action and the account and nothing about
    // where the daemon should look, so the Vault reached is the one it serves.
    let output = action_handshake(&account, target.authority(), input)?;

    ResultHandshake { output }.into()
}

/// Completes what `rola tool-handshake` can be given next.
///
/// What can be reached by name is what the Workspace has bound, which is the same set the
/// command itself resolves; an address is not, since there is nothing here that knows which
/// ones are worth offering.
#[completion(EntryToolHandshake)]
pub fn complete_tool_handshake(
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
#[derive(Grouped)]
pub struct ResultHandshake {
    /// What the daemon answered.
    output: String,
}

#[renderer(buffer)]
pub fn render_result_handshake(result: ResultHandshake) {
    r_println!("{}", result.output);
}

//! The `rola tool-handshake` command: speak the handshake action to a Vault.
//!
//! It is the handshake on its own, with no Workspace in the way: a daemon is reached directly
//! at an ip and a port and the action runs as the account the caller names, so a daemon can be
//! checked from anywhere a key is.

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
use rorolala_cli_setups::{ResVault, ResWorkspace, ResWorkspaceConfig};
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::error::ErrorConfigUnreadable;
use crate::exit_codes::{
    EC_ERR_TOOL_HANDSHAKE_ARGUMENT, EC_ERR_TOOL_HANDSHAKE_NO_ACCOUNT, EC_HELP,
};
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
/// `VAULT` names the daemon to reach: a name the Workspace has [bound](crate::cmd_vault), or
/// an ip and a port. Naming none reaches for the one the Workspace
/// [reaches for by default](crate::cmd_vault::vault_set_default), which is the same thing
/// `rola vault set-default` chose. The action runs as the account the work acts as — the one
/// `rola account` names, which is the same choice every other command makes — and what it
/// introduces itself with is that account's name, so the call needs nothing else.
///
/// # Errors
///
/// Renders [`ErrorHandshakeArguments`] when no Vault is named and none is reached for by
/// default, [`ErrorConfigUnreadable`] when the Workspace's configuration would not read and
/// there was no other Vault to reach, [`ErrorNoAccount`] when no account is named to act as,
/// and [`ErrorAccountUnknown`] when the one named is not there. What the exchange itself
/// fails with is routed on: `routeify` sends the [`ActionError`] the daemon call raises to
/// the renderer that knows it, so the failure is reported where every other action failure
/// is.
///
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "tool-handshake", routeify)]
pub fn tool_handshake(
    args: EntryToolHandshake,
    vault: &mut LazyRes<ResVault>,
    workspace: &mut LazyRes<ResWorkspace>,
    config: &mut LazyRes<ResWorkspaceConfig>,
    current: &mut LazyRes<ResCurrentAccount>,
) -> Next {
    // Picking cannot fail: a positional that is absent is `None`. Naming no Vault is how the
    // Workspace's own choice is reached for, so that is what is asked next.
    let place = match args.pick(&arg![Option<String>]).unwrap() {
        Some(place) => place,
        None => match default_vault(config.get_ref()) {
            Ok(Some(place)) => place,
            Ok(None) => return ErrorHandshakeArguments.into(),
            Err(next) => return next,
        },
    };

    let target = address_of(place, config.get_ref());

    let Some(name) = current.get_ref().name().map(str::to_string) else {
        return ErrorNoAccount.into();
    };

    let Some(account) = account_named(
        &name,
        workspace.get_ref().as_ref(),
        vault.get_ref().as_ref(),
    ) else {
        return ErrorAccountUnknown { name }.into();
    };

    // What the Workspace side holds and sends is who it is, so what is printed is the
    // daemon greeting the account this runs as.
    let input = account.name();

    // `?` here is `routeify`'s: an [`ActionError`] leaves through it rather than being said
    // again in the words of this command.
    //
    // [`ActionError`]: librorolala::protocol::ActionError
    let output = action_handshake(&account, target, input)?;

    ResultHandshake { output }.into()
}

/// Completes what `rola tool-handshake` can be given next.
///
/// What can be reached by name is what the Workspace has bound, so those are what is offered;
/// an address is not, since there is nothing here that knows which ones are worth offering.
#[completion(EntryToolHandshake)]
pub fn complete_tool_handshake(
    ctx: ShellContext,
    config: &mut LazyRes<ResWorkspaceConfig>,
) -> Suggest {
    if ctx.current_word.starts_with('-') {
        return suggest!();
    }

    let Some(config) = config.get_ref().config() else {
        return suggest!();
    };

    let mut names: Vec<String> = config.vaults().names().cloned().collect();
    names.sort();
    names.retain(|name| name.starts_with(&ctx.current_word));

    suggest! { names }
}

/// The Vault the Workspace reaches for, when nothing named one.
///
/// A Workspace that is not there has no choice to hand back, which is not an error: a daemon
/// can be reached without a Workspace at all. One whose configuration will not read is
/// another matter — there may well be a choice in it — so it is reported rather than passed
/// over, which is what the `Err` holds.
fn default_vault(config: &ResWorkspaceConfig) -> Result<Option<String>, Next> {
    match config {
        ResWorkspaceConfig::Read { config, .. } => {
            Ok(config.default_config().vault().map(str::to_string))
        }
        ResWorkspaceConfig::Absent => Ok(None),
        ResWorkspaceConfig::Unread { path, reason } => {
            Err(ErrorConfigUnreadable::new(path.clone(), reason.clone()).into())
        }
    }
}

/// The address to reach: what `place` names in the Workspace's Vaults, or `place` itself.
///
/// A name is the Workspace's own shorthand, so it is looked up in the configuration the
/// Workspace keeps beside it. A name that is not bound is taken to be an address already,
/// which is what lets a daemon be reached that no name was ever given.
fn address_of(place: String, config: &ResWorkspaceConfig) -> String {
    config
        .config()
        .and_then(|config| config.vaults().get(&place))
        .map_or(place, std::string::ToString::to_string)
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

/// Error: no Vault was named to reach.
#[derive(Grouped)]
pub struct ErrorHandshakeArguments;

#[renderer(buffer)]
pub fn render_error_handshake_arguments(_: ErrorHandshakeArguments, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tool_handshake.err_arguments").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_handshake.err_arguments_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_HANDSHAKE_ARGUMENT;
}

/// Error: no account was named to act as.
#[derive(Grouped)]
pub struct ErrorNoAccount;

#[renderer(buffer)]
pub fn render_error_no_account(_: ErrorNoAccount, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("tool_handshake.err_no_account").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("tool_handshake.err_no_account_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_HANDSHAKE_NO_ACCOUNT;
}

/// Error: the account named to act as is not there.
#[derive(Grouped)]
pub struct ErrorAccountUnknown {
    /// The name that is not an account.
    name: String,
}

#[renderer(buffer)]
pub fn render_error_account_unknown(error: ErrorAccountUnknown, ec: &mut ResExitCode) {
    r_eprintln!(
        "{}",
        err_line!(t!("tool_handshake.err_account_unknown", name = error.name).trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("tool_handshake.err_account_unknown_help").trim())
    );
    ec.exit_code = EC_ERR_TOOL_HANDSHAKE_NO_ACCOUNT;
}

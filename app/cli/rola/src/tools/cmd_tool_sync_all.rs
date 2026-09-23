//! The `rola tool sync-all` command: make the Workspace's store and the Vault's hold the same.
//!
//! It is [`tool handshake`](crate::tools::cmd_tool_handshake)'s reach — a Vault the Workspace has
//! bound, or an address — with the stores at both ends being what is spoken about: each side says
//! what it holds, and what only one of them holds crosses, so one run leaves both holding
//! everything either had. Because that is a change to both stores, it is asked for first.

// `#[chain]` copies the attributes of the function it is given onto the struct it generates,
// so a lint allowed on the handler below is reported as defined twice. The allow lives here,
// where it covers the one signature that needs it.
#![allow(clippy::trivially_copy_pass_by_ref)]

use librorolala::daemon::action_sync_all_async;
use librorolala::protocol::ActionError;
use mingling::{
    Grouped, LazyRes, ShellContext, Suggest, Wrap,
    confirm::YesConfirm,
    macros::{
        arg, buffer, chain, command, completion, help, metadata, r_eprintln, r_println, renderer,
        routeify, suggest,
    },
    metadata::Description,
    picker::EntryPicker,
    res::{ResConfirm, ResExitCode},
};
use rorolala_cli_setups::{ResCurrentRemoteVault, ResProgressSetting, ResWorkspace};
use rorolala_utils_cli_theme::{help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::exit_codes::{EC_CANCELLED, EC_HELP};
use crate::keys::account_named;
use crate::progress::Reporting;

#[help(buffer)]
pub fn help_tool_sync_all(_: EntryToolSyncAll, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tool_sync_all.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryToolSyncAll)]
pub fn desc_tool_sync_all() -> Description {
    t!("tool_sync_all.cmd_tool_sync_all_description")
        .to_string()
        .into()
}

/// Makes the stores at both ends hold the same keys.
///
/// `VAULT` names the Vault to reach, as [`tool handshake`](crate::tools::cmd_tool_handshake) reads
/// it: a name the Workspace has bound, or an ip and a port, with none naming the one the Workspace
/// reaches for by default. The exchange runs as the account the work acts as, the same one every
/// other command acts as.
///
/// What crosses is what only one end holds, in whichever direction that is: a full sync is not an
/// upload or a download, and one run leaves each store holding everything either of them had. A key
/// is the whole of what it names — a content kept as chunks is its manifest and the chunks the
/// manifest names — so a store that is given one keeps it the way the other store had it.
///
/// The run changes both stores, so it asks before it starts, and a run that is told no does nothing
/// and says so. `--confirm` answers that question in advance, which is what a run without a person
/// at the terminal should pass.
///
/// The run says what it is doing while it does it, down the channel `progress` names the end of:
/// each store is a bar that fills as its keys cross, and the key being carried is named beside it.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries it
/// out: the run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]), the Workspace
/// has no Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or as one no scope
/// holds ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), or the exchange itself failed
/// ([`ActionError`]).
///
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ActionError`]: librorolala::protocol::ActionError
// `ResConfirm` is a value small enough to copy, but it is the resource the protocol's injection
// hands in and not something this command owns: taking a copy would read a state of its own, so it
// is taken by reference, which is what the injection gives it. `ResProgressSetting` is read from
// where it is handed in for the same reason, and taking a copy of it here would be no more the
// answer the run gave.
#[command(node = "tool.sync-all")]
pub fn tool_sync_all(args: EntryToolSyncAll) -> StateToolSyncAll {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateToolSyncAll::from(named)
}

/// The state of making the two stores hold the same keys.
///
/// Which Vault is reached is the whole of what is said: naming none reaches for the one the
/// Workspace reaches for by default.
#[derive(Grouped, Wrap)]
pub struct StateToolSyncAll {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

// `ResConfirm` is a value small enough to copy, but it is the resource the protocol's injection
// hands in and not something this command owns: taking a copy would read a state of its own, so it
// is taken by reference, which is what the injection gives it. `ResProgressSetting` is read from
// where it is handed in for the same reason, and taking a copy of it here would be no more the
// answer the run gave.
#[chain(routeify)]
pub fn handle_tool_sync_all(
    state: StateToolSyncAll,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    confirm: &ResConfirm,
    progress: &ResProgressSetting,
) -> Next {
    // Everything below works through the Workspace, so it is asked once, here, and taken for granted
    // after: `?` is `routeify`'s, and a run that is inside no Workspace leaves through it. `held` is
    // the copy the exchange is spoken from, and the same one the action is handed.
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    // `?` here is `routeify`'s: a run with nothing to reach for leaves through it.
    let target = remote
        .get_ref()
        .vault_or_default(state.vault.unwrap_or_default())?;

    let name = current.get_ref().must_bind()?;
    let account = account_named(&name, Some(held), None)?;

    // Both stores are changed by this, so it is asked for rather than assumed. A run that is told no
    // has nothing to report and nothing to undo, and says so where nothing was done.
    if !confirm.ask::<YesConfirm>(&t!("tool_sync_all.confirm")) {
        return ResultSyncDeclined.into();
    }

    // What to sync is not named here: a full sync is everything either end holds, and the two ends
    // work that out between them. What the input carries is nothing, and the action reads none of it.
    //
    // The exchange is asynchronous and a command is not, so the two meet here: a runtime of this
    // run's own is what waits for it, and the run's progress is said to a reader on a thread of its
    // own, so that watching the exchange never slows it down. The reader is ended before anything
    // else is said, so the lines it left on the terminal are gone by the time there are results to
    // read.
    let reporting = Reporting::start(*progress);
    let runtime = tokio::runtime::Runtime::new().map_err(|error| ActionError::Io(error.into()))?;
    let outcome = runtime.block_on(action_sync_all_async(
        held,
        &account,
        target.to_string(),
        String::new(),
        reporting.progress(),
    ));
    reporting.finish();
    outcome?;

    ResultSynced.into()
}

/// Completes what `rola tool sync-all` can be given next.
///
/// The Vault is reached the way [`tool handshake`](crate::tools::cmd_tool_handshake) reaches it, so
/// what can be offered is the same: the names the Workspace has bound.
#[completion(EntryToolSyncAll)]
pub fn complete_tool_sync_all(
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

/// Result: the two stores were made to hold the same keys.
#[derive(Grouped)]
pub struct ResultSynced;

#[renderer(buffer)]
pub fn render_result_synced(_: ResultSynced) {
    r_println!("{}", t!("tool_sync_all.result_synced").trim());
}

/// Result: the run was asked to confirm itself and was not confirmed.
#[derive(Grouped)]
pub struct ResultSyncDeclined;

#[renderer(buffer)]
pub fn render_result_sync_declined(_: ResultSyncDeclined, ec: &mut ResExitCode) {
    r_eprintln!("{}", help_line!(t!("tool_sync_all.result_declined").trim()));
    ec.exit_code = EC_CANCELLED;
}

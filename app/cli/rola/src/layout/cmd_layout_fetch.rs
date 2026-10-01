//! The `rola layout fetch` command: bring a Vault's Layout here to read.
//!
//! A Vault holds one Layout, and it is the truth about what the work is made of: which path names
//! which `Uuid`, and who holds it. A Workspace cannot work in it — it is elsewhere — so what it
//! fetches is a copy, kept under the name the Vault was bound by and read rather than worked in.

use librorolala::daemon::action_fetch_layout;
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
use rorolala_cli_setups::{ResCurrentRemoteVault, ResOffline, ResWorkspace};
use rorolala_utils_cli_theme::trd;
use rorolala_utils_constants::VAULT_LAYOUT_NAME;
use rust_i18n::t;
use serde::Serialize;

use crate::Next;
use crate::account::ResCurrentAccount;
use crate::complete::{offer, positional, typing_flag, vault_names};
use crate::error::ErrorOffline;
use crate::exit_codes::EC_HELP;
use crate::keys::account_named;
use crate::layout::readonly_layout_dir;

#[help(buffer)]
pub fn help_layout_fetch(_: EntryLayoutFetch, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_layout_fetch.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryLayoutFetch)]
pub fn desc_layout_fetch() -> Description {
    t!("cmd_layout_fetch.description").to_string().into()
}

/// Completes what `rola layout fetch` can be given next.
///
/// What is named is a Vault the Workspace has bound, and never an address: the copy is kept under
/// the name, so a name is the whole of what the command can take.
#[completion(EntryLayoutFetch)]
pub fn complete_layout_fetch(
    ctx: ShellContext,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "fetch") != 0 {
        return suggest!();
    }

    offer(&ctx, vault_names(remote.get_ref()))
}

/// Brings a Vault's Layout here as a read-only copy
///
/// `VAULT` names the Vault to reach: a name the Workspace has
/// [bound](crate::vault::cmd_vault_bind), and not an address, since what is fetched is kept under
/// the Vault's name. Naming none reaches for the one the Workspace
/// [reaches for by default](crate::vault::cmd_vault_set_default).
///
/// The copy is written under `.rola/cache/readonly-layouts/<VAULT>/` and replaces whatever was
/// there. Nothing of the Workspace's own Layouts is touched, and nothing moves the other way.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries
/// it out: the run is not where the exchange is spoken from ([`ErrorShouldInWorkspace`]), the
/// Workspace has no Vault to hand it ([`ErrorRemoteVault`]), the run acts as no account or as one
/// no scope holds ([`ErrorNoAccount`], [`ErrorAccountUnknown`]), or the exchange itself failed
/// ([`ActionError`]).
///
/// [`ErrorShouldInWorkspace`]: crate::error::ErrorShouldInWorkspace
/// [`ErrorRemoteVault`]: crate::error::ErrorRemoteVault
/// [`ErrorNoAccount`]: crate::account::ErrorNoAccount
/// [`ErrorAccountUnknown`]: crate::keys::ErrorAccountUnknown
/// [`ActionError`]: librorolala::protocol::ActionError
#[command(node = "layout.fetch", entry = EntryLayoutFetch)]
pub fn layout_fetch(args: EntryLayoutFetch) -> StateLayoutFetch {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateLayoutFetch::from(named)
}

/// The state of fetching a Vault's Layout.
///
/// Which Vault is reached is the whole of what is said: naming none reaches for the one the
/// Workspace reaches for by default.
#[derive(Grouped, Wrap)]
pub struct StateLayoutFetch {
    /// The Vault to reach, or nothing when the Workspace's own choice is reached for.
    vault: Option<String>,
}

#[chain(routeify)]
pub fn handle_layout_fetch(
    state: StateLayoutFetch,
    workspace: &mut LazyRes<ResWorkspace>,
    remote: &mut LazyRes<ResCurrentRemoteVault>,
    current: &mut LazyRes<ResCurrentAccount>,
    offline: &ResOffline,
) -> Next {
    // This command is the fetch, so an offline run has nothing to do and nowhere to look.
    if **offline {
        return ErrorOffline.into();
    }

    // The copy is kept beside the Workspace and named after the Vault, so the two are needed
    // before anything is spoken: `check` is `routeify`'s, and a run outside a Workspace leaves
    // through it.
    workspace.get_ref().check()?;

    // UNWRAP: `check` above is exactly what a run without a Workspace fails with, and this run did
    // not fail, so there is a Workspace here to hand on.
    let held = workspace.get_ref().as_ref().unwrap();

    // What is fetched is kept under the Vault's name, so the name is what is resolved first and
    // the address only to dial it: `?` is `routeify`'s in both.
    let name = remote
        .get_ref()
        .name_or_default(state.vault.unwrap_or_default())?;
    let target = remote.get_ref().vault_or_default(name.clone())?;

    let account_name = current.get_ref().must_bind()?;
    let account = account_named(&account_name, Some(held), None)?;

    action_fetch_layout(held, &account, target.to_string(), name.clone())?;

    ResultLayoutFetched {
        path: readonly_layout_dir(held, &name, VAULT_LAYOUT_NAME)
            .display()
            .to_string(),
        vault: name,
    }
    .into()
}

/// Result: a Vault's Layout was fetched here.
#[derive(StructuralData, Serialize, Grouped)]
pub struct ResultLayoutFetched {
    /// The Vault it was fetched from.
    vault: String,
    /// Where the copy was written.
    path: String,
}

#[renderer(buffer)]
pub fn render_result_layout_fetched(result: ResultLayoutFetched) {
    r_println!(
        "{}",
        t!(
            "cmd_layout_fetch.result_fetched",
            vault = result.vault,
            path = result.path
        )
        .trim()
    );
}

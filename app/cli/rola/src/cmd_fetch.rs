//! The `rola fetch` command: the top-level name of `rola layout fetch`.
//!
//! Bringing a Vault's Layout here is the first thing a run does with a Vault, so it is reached by a
//! word of its own as well as under `layout`. Both are the same command: the same state is built
//! here and the same handler does the work, so what the two names do cannot drift apart.

use mingling::{
    LazyRes, ShellContext, Suggest,
    macros::{arg, buffer, command, completion, help, metadata, r_eprintln, suggest},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResCurrentRemoteVault;
use rorolala_utils_cli_theme::trd;
use rust_i18n::t;

use crate::complete::{offer, positional, typing_flag, vault_names};
use crate::exit_codes::EC_HELP;
use crate::layout::cmd_layout_fetch::StateLayoutFetch;

#[help(buffer)]
pub fn help_fetch(_: EntryFetch, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("cmd_fetch.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryFetch)]
pub fn desc_fetch() -> Description {
    t!("cmd_fetch.description").to_string().into()
}

/// Completes what `rola fetch` can be given next.
///
/// What is named is a Vault the Workspace has bound, and never an address: the copy is kept under
/// the name, so a name is the whole of what the command can take.
#[completion(EntryFetch)]
pub fn complete_fetch(ctx: ShellContext, remote: &mut LazyRes<ResCurrentRemoteVault>) -> Suggest {
    if typing_flag(&ctx) || positional(&ctx, "fetch") != 0 {
        return suggest!();
    }

    offer(&ctx, vault_names(remote.get_ref()))
}

/// Brings a Vault's Layout here as a read-only copy
///
/// The same command as [`rola layout fetch`](crate::layout::cmd_layout_fetch), reached by a shorter
/// name: `VAULT` names the Vault to reach, and naming none reaches for the one the Workspace
/// reaches for by default. The copy is written under `.rola/cache/readonly-layouts/<VAULT>/` and
/// replaces whatever was there.
///
/// # Errors
///
/// Every way this can fail is one the part that knows reports for itself, and `routeify` carries it
/// out: the run is not where the exchange is spoken from
/// ([`ErrorShouldInWorkspace`](crate::error::ErrorShouldInWorkspace)), the Workspace has no Vault to
/// hand it ([`ErrorRemoteVault`](crate::error::ErrorRemoteVault)), the run acts as no account or as
/// one no scope holds ([`ErrorNoAccount`](crate::account::ErrorNoAccount),
/// [`ErrorAccountUnknown`](crate::keys::ErrorAccountUnknown)), or the exchange itself failed
/// ([`ActionError`](librorolala::protocol::ActionError)).
#[command(node = "fetch", entry = EntryFetch)]
pub fn fetch(args: EntryFetch) -> StateLayoutFetch {
    // Picking cannot fail: a positional that is absent is `None`, and naming none is what lets the
    // Workspace's own choice be the one that is reached for.
    let named: Option<String> = args.pick(&arg![Option<String>]).unwrap();

    StateLayoutFetch::from(named)
}
